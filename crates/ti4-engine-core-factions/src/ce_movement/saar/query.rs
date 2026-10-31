//! saar: tier Q: pure hooks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CE). Owned by CE only.
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | `economy.production_destinations` | `commander_docks` | Q | seam `production_destinations` (a P3 add) |
//! | `strategy.scores_without_home` | `nomadic` | Q | `strategy.scores_without_home` |
//! | movement `cannot_activate`, `move_bonus` | | Q | `movement.rs` (A2, done) |
//! | `timing_abilities`, `mapped_component_actions`, `perform_component`, `commander_unlocked`, `leader_action`, `use_leader` | | W | B7 (leaders) |

use ti4_engine_core::generated::{Faction, Leader, System, UnitBaseType};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::hooks::economy::Destinations;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::kernel::seam::{DestSite, Pass, Q, Source};
use ti4_engine_core::state::CoreState;

use super::super::readers::has_commander_ability;
use crate::common::{is_faction, scope};

const SOURCE: Source = Source::Faction(Faction::TheClanOfSaar);

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.general.commander_unlocked.push(commander_unlocked);
    t.general.leader_action.push(leader_action);
    t.seam.production_destinations.push(SOURCE, Pass::Add, |q: Q<'_>, site: &DestSite, out| {
        out.extend_from_slice(&commander_docks(q.s, q.k, site.who, site.system, site.base));
    });
    t.strategy.scores_without_home.push(nomadic);
}

/// "You can score objectives even if you do not control the planets in your home system."
pub fn nomadic(s: &CoreState, who: Seat) -> bool {
    is_faction(s, who, Faction::TheClanOfSaar)
}

/// Rowl Sarring: "Your space docks are treated as if they were in the systems you produce in" (fighters and infantry
/// can be placed at any of the holder's docks that are not blockaded): the docks in board order, the space area for
/// fighters, the dock's planet for infantry. Own unlocked commander, a faceup Alliance, or a Yin grant.
pub fn commander_docks(s: &CoreState, k: &Kernel, who: Seat, _producing: System, base: UnitBaseType) -> Destinations {
    let mut out = Destinations::new();
    if !matches!(base, UnitBaseType::Fighter | UnitBaseType::Infantry) || !has_commander_ability(s, who, Leader::RowlSarring) {
        return out;
    }
    let scope = scope(k);
    let is_ship = |kind| scope.unit(kind).is_some_and(|d| d.ship);
    let is_dock = |kind| scope.unit(kind).is_some_and(|d| d.base_type == UnitBaseType::Spacedock);
    let blockaded = |system: System| {
        let units = s.board().sys(system).units.as_slice();
        units.iter().any(|u| u.owner() != who && is_ship(u.kind())) && !units.iter().any(|u| u.owner() == who && is_ship(u.kind()))
    };
    for (system, cell) in s.board().systems() {
        let docks_in_space = cell.units.as_slice().iter().filter(|u| u.owner() == who && is_dock(u.kind())).count();
        // The string walks the system's space area first, then its planets in planet-id order.
        let mut found: Vec<(System, Option<ti4_engine_core::generated::Planet>)> = Vec::new();
        for _ in 0..docks_in_space {
            found.push((system, None));
        }
        for planet in ti4_engine_core::query::board::planets_of(s, system) {
            let docks = s.board().planet(planet).units.as_slice().iter().filter(|u| u.owner() == who && is_dock(u.kind())).count();
            for _ in 0..docks {
                found.push((system, if base == UnitBaseType::Infantry { Some(planet) } else { None }));
            }
        }
        if !found.is_empty() && !blockaded(system) {
            for f in found {
                out.push(f);
            }
        }
    }
    out
}

// -- leader hooks (package F2) ----------------------------------------

/// Rowl Sarring, unlock: "Have 3 space docks on the game board." A Floating Factory is a space dock (same base type).
pub fn commander_unlocked(s: &CoreState, _: &Kernel, who: Seat, leader: Leader) -> Option<bool> {
    (leader == Leader::RowlSarring).then(|| ti4_engine_core::query::leader_reads::count_pieces(s, who, |p| p.kind().data().base_type == UnitBaseType::Spacedock) >= 3)
}

/// Gurno Aggero: a space dock stands and the seat holds the card (any adjacent system is a legal choice) (`saar::leader_action`).
pub fn leader_action(s: &CoreState, _: &Kernel, who: Seat, leader: Leader) -> Option<bool> {
    (leader == Leader::GurnoAggero).then(|| {
        ti4_engine_core::query::leader_reads::count_pieces(s, who, |p| p.kind().data().base_type == UnitBaseType::Spacedock) > 0
            && ti4_engine_core::query::leaders::status(s, who, leader).is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ce_movement::testkit::table;
    use ti4_engine_core::generated::Unit;
    use ti4_engine_core::state::{LeaderStatus, Piece};

    const SAAR: Faction = Faction::TheClanOfSaar;
    const SOL: Faction = Faction::TheFederationOfSol;

    #[test]
    fn nomadic_is_the_saar_seat_alone() {
        let (s, _) = table(&[SAAR, SOL]);
        assert!(nomadic(&s, Seat(0)));
        assert!(!nomadic(&s, Seat(1)));
    }

    #[test]
    fn the_unlocked_commander_opens_unblockaded_docks_for_fighters_and_infantry() {
        let (mut s, k) = table(&[SAAR, SOL]);
        let sys = ti4_engine_core::ids::system_at(30);
        s.board_mut().sys_mut(sys).units.push(Piece::new(Unit::SpaceDockI, Seat(0)));
        let producing = ti4_engine_core::ids::system_at(31);
        assert!(commander_docks(&s, &k, Seat(0), producing, UnitBaseType::Fighter).is_empty(), "commander still locked");
        s.players_mut().edit(0).leaders.status.insert(Leader::RowlSarring, LeaderStatus::Unlocked);
        assert_eq!(commander_docks(&s, &k, Seat(0), producing, UnitBaseType::Fighter).as_slice(), &[(sys, None)]);
        assert!(commander_docks(&s, &k, Seat(0), producing, UnitBaseType::Mech).is_empty(), "fighters and infantry only");
        s.board_mut().sys_mut(sys).units.push(Piece::new(Unit::DestroyerI, Seat(1)));
        assert!(commander_docks(&s, &k, Seat(0), producing, UnitBaseType::Fighter).is_empty(), "blockaded");
        s.board_mut().sys_mut(sys).units.push(Piece::new(Unit::CruiserI, Seat(0)));
        assert_eq!(commander_docks(&s, &k, Seat(0), producing, UnitBaseType::Fighter).len(), 1, "a ship of the holder lifts the blockade");
    }

    #[test]
    fn registering_fills_the_gather_and_the_legacy_slot() {
        let mut t = HookTable::new();
        register(&mut t);
        assert_eq!(t.seam.production_destinations.len(), 1);
        assert_eq!(t.registered(), 2);
    }
}
