//! naaz: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//!
//! | point | string function | status |
//! |---|---|---|
//! | the Eidolon Maximum is repaired and Supercharge is offered at the start of a ground combat round | `naaz::ground_combat_round_started` (`repair_voltron`, `offer_supercharge`) | ported (package W1/W2): `ground.ground_combat_round_started`, called by the `BeginGroundRound` window; the question is `Ask::Supercharge` |
//! | the same at the start of a space combat round | `naaz::space_combat_round_started` | ported (package W2): `general.space_combat_round_started`, called by the `BeginRound` window (which also flips the ground-form dual-form units to their ship form in round 1; the machine adds them to the fleet of the start) |
//! | Garv and Gunn, Hesh and Prit, Dart and Tai, Visz El Vir synergy asks | `component_actions`, `perform_component`, `leader_action`, `use_leader`, `timing_abilities`, `commander` | B7 (leaders, component actions, timing abilities) |
//! | Absolute Synergy flip and the survivor choice | `flip_synergy`, `survivor_choices` | B7 + the events window |

use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt, Subtype as CSub};
use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Planet, System, Technology, Unit};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::{fx_strategy, Cx};
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Word};

use super::query::is_naaz;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.ground.ground_combat_round_started.push(ground_round_started);
    t.general.space_combat_round_started.push(space_round_started);
}

/// `naaz::space_combat_round_started`: the Eidolon Maximum of the active system is repaired ("repair it at the start of every combat
/// round": in the space area, and on the planets of the system while its owner has a ship in space), then the Supercharge offer.
pub fn space_round_started(cx: &mut Cx<'_>, who: Seat) {
    if is_naaz(cx.state, who)
        && let Some(system) = cx.state.meta().active_system
    {
        let scope = ti4_engine_core::tables::Scope(u32::from(cx.kernel.sources.0));
        let ship = |kind: Unit| scope.unit(kind).is_some_and(|d| d.ship);
        let participates = cx.state.board().sys(system).units.iter().any(|p| p.owner() == who && ship(p.kind()));
        if participates {
            let planets: Vec<Planet> = ti4_engine_core::query::board::planets_of(cx.state, system).collect();
            for planet in planets {
                repair_maximum(cx.state.board_mut().planet_mut(planet).units.as_mut_slice(), who);
            }
        }
        repair_maximum(cx.state.board_mut().sys_mut(system).units.as_mut_slice(), who);
    }
    offer_supercharge(cx, who, Word::Space);
}

fn repair_maximum(pieces: &mut [ti4_engine_core::state::Piece], who: Seat) {
    for piece in pieces {
        if piece.owner() == who && piece.kind() == Unit::EidolonMaximum {
            *piece = piece.with_sustained(false);
        }
    }
}

/// `naaz::ground_combat_round_started`: "repair it at the start of every combat round" (the Eidolon Maximum on the planet), then the
/// Supercharge offer of every participant (the string asks for each of them; only a holder of the technology is asked).
pub fn ground_round_started(cx: &mut Cx<'_>, who: Seat, _system: System, planet: Planet) {
    if is_naaz(cx.state, who) {
        repair_maximum(cx.state.board_mut().planet_mut(planet).units.as_mut_slice(), who);
    }
    offer_supercharge(cx, who, Word::Ground);
}

/// `naaz::offer_supercharge`: "At the start of a combat round, you may exhaust this card to apply +1 to the results of all of your units'
/// combat rolls during this combat round." The mark names the kind of combat and the round sequence, so it lapses by itself.
fn offer_supercharge(cx: &mut Cx<'_>, who: Seat, word: Word) {
    let key = MarkKey::NaazSupercharge(who);
    if std::env::var_os("TI4_W_DEBUG").is_some() {
        eprintln!("W-DEBUG offer_supercharge who {who:?} seq {} mark {:?} owned {:?} exhausted {:?}", cx.state.meta().combat_round_seq, cx.state.marks().tail.get(&key), cx.state.players().by_seat(who).map(|p| p.tech.owned.contains(Technology::Supercharge)), cx.state.players().by_seat(who).map(|p| p.tech.exhausted.contains(Technology::Supercharge)));
    }
    cx.state.marks_mut().tail.edit().remove(&key);
    let ready = cx.state.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(Technology::Supercharge) && !p.tech.exhausted.contains(Technology::Supercharge));
    if !ready {
        return;
    }
    let choice = Choice {
        actor: who,
        ask: Ask::Combat(CAsk::Supercharge),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Technology(Technology::Supercharge)),
            subtype: Subtype::Combat(CSub::Supercharge),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: [Opt::Combat(COpt::UseSupercharge), Opt::Decline].into_iter().collect(),
    };
    if cx.ask(&choice) != Pick::Opt(Opt::Combat(COpt::UseSupercharge)) {
        return;
    }
    let seq = cx.state.meta().combat_round_seq;
    cx.state.marks_mut().tail.edit().insert(key, MarkVal::Rec(vec![MarkVal::Atom(Atom::Word(word)), MarkVal::Atom(Atom::Num(seq))]));
    fx_strategy::exhaust_technology(cx.state, who, Technology::Supercharge);
}
