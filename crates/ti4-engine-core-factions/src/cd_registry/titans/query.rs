//! titans: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `ground.forced_combat_planets` | `titans::forced_combat_planets` (Coalescence) | Q | `ground.forced_combat_planets` |
//! | `economy.action_card_draw_bonus` | `titans_leaders::slumberstate_draw_bonus` | Q | `economy.action_card_draw_bonus` (a sum) |
//! | `economy.pds_placement_alternative` | `titans::hecatoncheires_offer` | blocked | needs `action_cards::placed_unit_id` (upgrade, faction unit, `unit_form_override`) and `supply::allowed` typed: the supply owner |
//! | `economy.pds_placement_alternative_performed` | `titans::hecatoncheires_perform` | W | `offer.rs`, with the offer |
//! | `general.commander_unlocked` | `titans_leaders::commander_unlocked` (Tungstantus) | Q | `general.commander_unlocked` |
//! | `leader_action`, `use_leader`, `component_actions`, `perform_component`, `timing_abilities` | | W | B7 |

use ti4_engine_core::generated::{Breakthrough, Faction, Leader};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::hooks::ground::PlanetList;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::board;
use ti4_engine_core::state::CoreState;

use crate::common::{holds_breakthrough, is_faction, rec};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.general.leader_action.push(leader_action);
    t.ground.forced_combat_planets.push(forced_combat_planets);
    t.economy.action_card_draw_bonus.push(slumberstate_draw_bonus);
    t.general.commander_unlocked.push(commander_unlocked);
}

fn is_titans(s: &CoreState, who: Seat) -> bool {
    is_faction(s, who, Faction::TheTitansOfUl)
}

/// Coalescence (ground half): the planets of the active system where Awaken put the invader's units beside another
/// player's units this activation, and the invader still has units.
pub fn forced_combat_planets(s: &CoreState, _: &Kernel, invader: Seat, system: ti4_engine_core::generated::System) -> PlanetList {
    let mut out = PlanetList::default();
    if !is_titans(s, invader) {
        return out;
    }
    let now = s.meta().activation_seq;
    for (key, value) in s.marks().tail.iter() {
        let MarkKey::TitansCoalescence(planet) = *key else { continue };
        let Some([MarkVal::Atom(Atom::Num(seq)), MarkVal::Atom(Atom::System(at))]) = rec(value) else { continue };
        if *seq == now && *at == system && board::planet_units(s, planet).iter().any(|p| p.owner() == invader) {
            out.push(planet);
        }
    }
    out.as_mut_slice().sort_unstable();
    out
}

fn holds_slumberstate(s: &CoreState, who: Seat) -> bool {
    is_titans(s, who) && holds_breakthrough(s, who, Breakthrough::SlumberstateComputing)
}

/// Players sharing a coexisting planet with `who`: the controller and every coexister of each planet in coexistence on
/// which `who` is one of them.
fn coexisting_with(s: &CoreState, who: Seat) -> ti4_engine_core::SeatSet {
    let mut found = ti4_engine_core::SeatSet::EMPTY;
    for (system, _) in s.board().systems() {
        for planet in board::planets_of(s, system) {
            let mut here = board::coexisters(s, system, planet);
            if here.is_empty() {
                continue;
            }
            if let Some(holder) = board::control(s, planet) {
                here.insert(holder);
            }
            if here.contains(who) {
                for other in here.iter().filter(|other| *other != who) {
                    found.insert(other);
                }
            }
        }
    }
    found
}

/// "During the status phase, for each player you are coexisting with, you and that player each draw 1 additional
/// action card." For the holder, one per partner; for a partner, one per Slumberstate holder it coexists with.
pub fn slumberstate_draw_bonus(s: &CoreState, _: &Kernel, who: Seat, _requested: usize) -> usize {
    if s.meta().phase != ti4_engine_core::state::Phase::Status {
        return 0;
    }
    if holds_slumberstate(s, who) {
        return coexisting_with(s, who).iter().count();
    }
    s.players().iter().map(|p| p.seat).filter(|seat| holds_slumberstate(s, *seat)).filter(|seat| coexisting_with(s, *seat).contains(who)).count()
}

/// Tungstantus, unlock: 5 structures on the board.
pub fn commander_unlocked(s: &CoreState, _: &Kernel, who: Seat, leader: Leader) -> Option<bool> {
    (leader == Leader::Tungstantus).then(|| {
        let mine = |p: &ti4_engine_core::state::Piece| p.owner() == who && p.kind().data().structure;
        let count = s.board().systems().map(|(_, c)| c.units.as_slice().iter().filter(|p| mine(p)).count()).sum::<usize>()
            + s.board().planets().map(|(_, c)| c.units.as_slice().iter().filter(|p| mine(p)).count()).sum::<usize>();
        count >= 5
    })
}

// -- leader hooks (package F2) ----------------------------------------

/// Ul the Progenitor: Elysium is in play (controlled by somebody) and does not wear Geoform yet (`titans_leaders::elysium_available`).
pub fn leader_action(s: &CoreState, _: &Kernel, _: Seat, leader: Leader) -> Option<bool> {
    use ti4_engine_core::generated::{Attachment, Planet};
    (leader == Leader::UlTheProgenitor).then(|| board::control(s, Planet::Elysium).is_some() && !board::attachments(s, Planet::Elysium).as_slice().contains(&Attachment::Geoform))
}

