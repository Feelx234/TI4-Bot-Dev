//! The display facts of a core question that only the state knows (`Choice.details`), computed from the `CoreState` alone: the
//! string builds them from its position (`tokens::with_pool_details`, ...). Display only: invisible to the rules and to every
//! hash, so the oracle's choice comparison (`details` included) is what guards them.

use ti4_engine_core::choice::reaction::{Ask as RAsk, Relation};
use ti4_engine_core::choice::strategy::Ask as SAsk;
use ti4_engine_core::choice::{Ask, Choice as CoreChoice};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::{fx, Kernel};
use ti4_engine_core::state::{CoreState, EventKind};

use crate::choice::Choice;

/// Complete the rendered `rendered` of `choice`, asked in `core`.
pub fn complete(core: &CoreState, kernel: &Kernel, choice: &CoreChoice, rendered: &mut Choice) {
    match choice.ask {
        Ask::Tactical(_) => crate::tactical_live::complete(core, kernel, choice, rendered),
        Ask::Combat(_) => crate::combat_live::complete(core, kernel, choice, rendered),
        Ask::Strategy(SAsk::GainPool { left }) => pool_details(core, choice.actor, "gain", Some(usize::from(left)), rendered),
        // 81.5: the whole sheet is rearranged (`tokens::with_pool_details(.., "redistribute", None)`).
        Ask::Strategy(SAsk::StatusRedistribute { .. }) => pool_details(core, choice.actor, "redistribute", None, rendered),
        // Warfare's redistributions: the whole sheet is rearranged.
        Ask::Strategy(SAsk::WarfareRedistribute { .. } | SAsk::TeWarfareRedistribute { .. } | SAsk::TeWarfareRedistributeAfter { .. }) => {
            pool_details(core, choice.actor, "redistribute", None, rendered);
        }
        Ask::Reaction(RAsk::Window { event: EventKind::SystemActivated, relation: Relation::After }) => harness_preview(core, kernel, choice, rendered),
        _ => {}
    }
    super::turn::complete(core, choice, rendered);
    super::agenda::complete(core, choice, rendered);
    super::strategic::complete(core, kernel, choice, rendered);
    super::explore::complete(core, choice, rendered);
}

/// `tokens::with_pool_details(.., mode, to_place)`: the pools as they stand, the tokens in reinforcements, Letnev's Armada, then the tokens
/// still to place (a gain) or the total held (a redistribution).
pub(super) fn pool_details(core: &CoreState, actor: Seat, mode: &str, to_place: Option<usize>, rendered: &mut Choice) {
    let Some(p) = core.players().by_seat(actor) else { return };
    let d = &mut rendered.details;
    d.insert("kind".to_owned(), "command_tokens".into());
    d.insert("mode".to_owned(), mode.into());
    d.insert(
        "pools".to_owned(),
        serde_json::json!({ "tactic": p.econ.tactic_tokens, "fleet": p.econ.fleet_tokens, "strategic": p.econ.strategic_tokens }),
    );
    d.insert("reinforcements".to_owned(), fx::tokens_in_reinforcements(core, actor).into());
    if p.faction == ti4_engine_core::generated::Faction::TheBaronyOfLetnev {
        d.insert("fleet_supply_bonus".to_owned(), 2.into());
    }
    match (mode, to_place) {
        ("gain" | "buy", Some(count)) => {
            d.insert("tokens_to_place".to_owned(), count.into());
        }
        ("redistribute", _) => {
            d.insert("total".to_owned(), (p.econ.tactic_tokens + p.econ.fleet_tokens + p.econ.strategic_tokens).into());
        }
        _ => {}
    }
}

/// `reaction_preview::preview_for`'s "Harness Energy": the window around a system's activation offers the card to the activator of an anomaly, and
/// says how many commodities it would replenish (display only; the offer itself is the core's `reaction_slot`).
fn harness_preview(core: &CoreState, k: &ti4_engine_core::kernel::Kernel, choice: &CoreChoice, rendered: &mut Choice) {
    use ti4_engine_core::generated::ActionCard;
    let Some(player) = core.players().by_seat(choice.actor) else { return };
    let (Some(system), true) = (core.meta().active_system, core.meta().active == Some(choice.actor)) else { return };
    if !player.cards.hand.as_slice().contains(&ActionCard::HarnessEnergy) {
        return;
    }
    let rifts = k.hooks.movement.gravity_rifts(core);
    if !ti4_engine_core::rules::action_cards::system_is_anomaly(ti4_engine_core::query::reach::scope_of(k), system, rifts.as_slice()) {
        return;
    }
    let Some(option) = rendered.options.iter().find(|o| o.id.starts_with("reaction:")).map(|o| o.id.clone()) else { return };
    let limit = ti4_engine_core::rules::strategy::commodity_limit(player.faction);
    let held = i32::try_from(player.econ.commodities).unwrap_or(i32::MAX);
    let Some(context) = rendered.context.as_mut() else { return };
    context.previews.push(crate::decision_context::EffectPreview {
        option,
        card: "harness".to_owned(),
        effect: "replenish_commodities".to_owned(),
        seats: Vec::new(),
        system: Some(ti4_model::id::SystemId::new(crate::generated::IdStr::as_str(system))),
        planets: Vec::new(),
        units: Vec::new(),
        amount: Some(i64::from((limit - held).max(0))),
        of: Some(i64::from(limit)),
        refs: Vec::new(),
        fizzles: None,
    });
}

/// `reaction_preview::preview_for`'s "Infiltrate": the window around the gain of a planet offers the card to the gainer when a rival's PDS or space dock
/// stands on it, and names what would be replaced (display only; the offer is the core's `reaction_slot`, the swaps are `infiltrate_swaps`).
pub fn infiltrate_preview(r: &crate::render::Render, core: &CoreState, k: &ti4_engine_core::kernel::Kernel, plays: Option<&ti4_engine_core::kernel::ability::PlaysProvider>, choice: &CoreChoice, rendered: &mut Choice) {
    use ti4_engine_core::generated::ActionCard;
    let Ask::Reaction(RAsk::Window { event: EventKind::PlanetControlGained, .. }) = choice.ask else { return };
    let Some(player) = core.players().by_seat(choice.actor) else { return };
    if !player.cards.hand.as_slice().contains(&ActionCard::Infiltrate) || core.meta().rules_version < ti4_model::state::RULES_INFILTRATE_FIX {
        return;
    }
    let Some((system, planet, gainer, _)) = core.transient().combat.last_control_gained else { return };
    if gainer != choice.actor {
        return;
    }
    let Some(option) = rendered.options.iter().find(|o| o.id.starts_with("reaction:")).map(|o| o.id.clone()) else { return };
    let scope = ti4_engine_core::query::reach::scope_of(k);
    let forbidden = |unit| k.hooks.economy.effect_placement_forbidden(core, k, choice.actor, unit);
    // The forms the seat would place (the swaps' replacement), as the play facts hand them to the card.
    let own = plays
        .and_then(|p| (p.0)(core, k))
        .and_then(|facts| facts.seats.iter().find(|s| s.who == choice.actor).map(|s| (s.more.pds.0, s.more.spacedock.0)))
        .unwrap_or((None, None));
    let swaps = ti4_engine_core::rules::action_cards::infiltrate_swaps(core, scope, choice.actor, planet, own, &forbidden);
    let mut counts: std::collections::BTreeMap<(ti4_model::id::PlayerId, String), u32> = std::collections::BTreeMap::new();
    for (piece, _) in swaps.as_slice() {
        let Some(owner) = r.seats.get(usize::from(piece.owner().0)) else { continue };
        *counts.entry((ti4_model::id::PlayerId::new(owner.clone()), crate::generated::IdStr::as_str(piece.kind()).to_owned())).or_insert(0) += 1;
    }
    let units: Vec<_> = counts.into_iter().map(|((owner, unit_type), count)| crate::decision_context::TriggerUnits { owner, unit_type, count }).collect();
    let Some(context) = rendered.context.as_mut() else { return };
    context.previews.push(crate::decision_context::EffectPreview {
        option,
        card: "infiltrate".to_owned(),
        effect: "replace_structures".to_owned(),
        seats: Vec::new(),
        system: Some(ti4_model::id::SystemId::new(crate::generated::IdStr::as_str(system))),
        planets: vec![ti4_model::id::PlanetId::new(crate::generated::IdStr::as_str(planet))],
        fizzles: units.is_empty().then(|| "nothing_there".to_owned()),
        units,
        amount: None,
        of: None,
        refs: Vec::new(),
    });
}
