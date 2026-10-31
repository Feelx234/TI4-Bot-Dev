//! The display facts of a core question that only the state knows (`Choice.details`), computed from the `CoreState` alone: the
//! string builds them from its position (`tokens::with_pool_details`, ...). Display only: invisible to the rules and to every
//! hash, so the oracle's choice comparison (`details` included) is what guards them.

use ti4_engine_core::choice::reaction::{Ask as RAsk, Relation};
use ti4_engine_core::choice::strategy::Ask as SAsk;
use ti4_engine_core::choice::{Ask, Choice as CoreChoice};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::state::{CoreState, EventKind};

use crate::choice::Choice;

/// Complete the rendered `rendered` of `choice`, asked in `core`.
pub fn complete(core: &CoreState, k: &ti4_engine_core::kernel::Kernel, choice: &CoreChoice, rendered: &mut Choice) {
    match choice.ask {
        Ask::Tactical(_) => crate::tactical_live::complete(core, k, choice, rendered),
        Ask::Combat(_) => crate::combat_live::complete(core, k, choice, rendered),
        Ask::Strategy(SAsk::GainPool { left }) => pool_details(core, choice, "gain", Some(usize::from(left)), rendered),
        // 81.5: the whole sheet is rearranged (`tokens::with_pool_details(.., "redistribute", None)`).
        Ask::Strategy(SAsk::StatusRedistribute { .. }) => pool_details(core, choice, "redistribute", None, rendered),
        Ask::Reaction(RAsk::Window { event: EventKind::SystemActivated, relation: Relation::After }) => harness_preview(core, k, choice, rendered),
        _ => {}
    }
    super::turn::complete(core, choice, rendered);
    super::agenda::complete(core, choice, rendered);
}

/// `tokens::with_pool_details(.., mode, to_place)`: the pools as they stand, the tokens in reinforcements, Letnev's Armada, then the tokens
/// still to place (a gain) or the total held (a redistribution).
fn pool_details(core: &CoreState, choice: &CoreChoice, mode: &str, to_place: Option<usize>, rendered: &mut Choice) {
    let Some(p) = core.players().by_seat(choice.actor) else { return };
    let d = &mut rendered.details;
    d.insert("kind".to_owned(), "command_tokens".into());
    d.insert("mode".to_owned(), mode.into());
    d.insert(
        "pools".to_owned(),
        serde_json::json!({ "tactic": p.econ.tactic_tokens, "fleet": p.econ.fleet_tokens, "strategic": p.econ.strategic_tokens }),
    );
    d.insert("reinforcements".to_owned(), fx::tokens_in_reinforcements(core, choice.actor).into());
    if p.faction == ti4_engine_core::generated::Faction::TheBaronyOfLetnev {
        d.insert("fleet_supply_bonus".to_owned(), 2.into());
    }
    match (mode, to_place) {
        ("gain", Some(count)) => {
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
