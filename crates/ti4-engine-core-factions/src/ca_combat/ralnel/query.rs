//! ralnel: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CA).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `movement.free_cargo` | `ralnel::free_cargo` | Q | `movement.rs` (A2) |
//! | `ralnel_silenced_in_space` | `ralnel::silenced_in_space` | Q | direct (`silenced`'s bridge reads it) |
//! | `ralnel_linkship_guns` | `ralnel::linkship_guns` | Q | direct; the string `may_fire` gate is the window's, the slot has none |
//! | `ralnel_flagship_among` | `ralnel::flagship_among` | Q | direct |
//! | `ralnel_alarum_on` | `ralnel::alarum_on` | Q | direct |
//! | `ralnel_listens_for_tactical_end` | `ralnel::listens_for_tactical_end` | Q | direct |
//! | `ralnel_cards_skimmer_takes` | `ralnel_cards::skimmer_takes` | W | `offer.rs` (writes a mark, takes the discard) |

use ti4_engine_core::generated::{Faction, Planet, System, Unit};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::hooks::direct::Pieces;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, Piece};

use crate::common::{flagship_has_text, is_faction};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    let d = &mut t.direct;
    d.ralnel_silenced_in_space.push(silenced_in_space);
    d.ralnel_linkship_guns.push(linkship_guns);
    d.ralnel_flagship_among.push(flagship_among);
    d.ralnel_alarum_on.push(alarum_on);
    d.ralnel_listens_for_tactical_end.push(listens_for_tactical_end);
}

/// Whether `piece` is a structure of a Ral Nel player.
fn ralnel_structure(s: &CoreState, piece: Piece) -> bool {
    is_faction(s, piece.owner(), Faction::TheRalNelConsortium) && piece.kind().data().structure
}

/// "While your structures are in the space area, they cannot use their unit abilities."
fn silenced_in_space(s: &CoreState, _: &Kernel, piece: Piece) -> bool {
    ralnel_structure(s, piece)
}

/// The SPACE CANNON shots the Linkships in `system` add: each Linkship I uses one of its owner's structures in the
/// space area (each structure once); each Linkship II uses the best of them again.
fn linkship_guns(s: &CoreState, _: &Kernel, system: System) -> Pieces {
    let units = s.board().sys(system).units.as_slice();
    let mut owners: Vec<Seat> = units.iter().map(|p| p.owner()).collect();
    owners.sort_unstable();
    owners.dedup();
    let mut guns = Pieces::default();
    for owner in owners {
        if !is_faction(s, owner, Faction::TheRalNelConsortium) {
            continue;
        }
        let mut structures: Vec<(u8, Piece)> = units
            .iter()
            .filter(|p| p.owner() == owner && p.kind().data().structure)
            .filter_map(|p| p.kind().data().space_cannon.filter(|r| r.hits_on > 0).map(|r| (r.hits_on, *p)))
            .collect();
        if structures.is_empty() {
            continue;
        }
        structures.sort_by_key(|(hits, _)| *hits);
        let count = |kind: Unit| units.iter().filter(|p| p.owner() == owner && p.kind() == kind).count();
        for (_, structure) in structures.iter().take(count(Unit::LinkshipI)) {
            guns.push(*structure);
        }
        if let Some((_, best)) = structures.first() {
            for _ in 0..count(Unit::LinkshipII) {
                guns.push(*best);
            }
        }
    }
    guns
}

/// Whether a Ral Nel flagship (or a Nekro flagship lent its text) is among the `arrived` ship types of a retreat.
fn flagship_among(s: &CoreState, who: Seat, arrived: &[Unit]) -> bool {
    arrived.iter().any(|unit| flagship_has_text(s, who, *unit, Unit::LastDispatch))
}

/// Whether a Ral Nel Alarum stands on `planet` in `system`.
fn alarum_on(s: &CoreState, system: System, planet: Planet) -> bool {
    board::home_system(s, planet) == Some(system)
        && board::planet_units(s, planet).iter().any(|p| p.kind() == Unit::Alarum && is_faction(s, p.owner(), Faction::TheRalNelConsortium))
}

/// Whether Miniaturization could matter now: a Ral Nel seat with a space-area structure in a system where it
/// controls a planet.
fn listens_for_tactical_end(s: &CoreState, _: &Kernel) -> bool {
    s.players().iter().any(|p| {
        if p.faction == Faction::TheRalNelConsortium {
            ti4_engine_core::unsupported::hit(ti4_engine_core::unsupported::Disabled::RalNel);
        }
        p.faction == Faction::TheRalNelConsortium
            && s.board().systems().any(|(system, cell)| {
                cell.units.as_slice().iter().any(|u| u.owner() == p.seat && u.kind().data().structure)
                    && board::controls_a_planet_in(s, system, p.seat)
            })
    })
}

#[cfg(test)]
mod unsupported_tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::unsupported::{Policy, with_policy};

    fn panics_with(name: &str, f: impl FnOnce()) -> bool {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        std::panic::set_hook(prev);
        r.err().is_some_and(|e| {
            e.downcast_ref::<String>().is_some_and(|t| t.contains(name))
                || e.downcast_ref::<&str>().is_some_and(|t| t.contains(name))
        })
    }

    /// XF-3 family: the Ral Nel Consortium is switched off.
    #[test]
    fn the_ral_nel_listener_panics_for_a_ral_nel_seat() {
        let (s, k) = table(&[Faction::TheArborec, Faction::TheRalNelConsortium]);
        let _ = with_policy(Policy::Off, || listens_for_tactical_end(&s, &k));
        assert!(panics_with("RalNel", || with_policy(Policy::Panic, || { listens_for_tactical_end(&s, &k); })));
        let (plain, k) = table(&[Faction::TheArborec, Faction::TheEmiratesOfHacan]);
        assert!(!with_policy(Policy::Panic, || listens_for_tactical_end(&plain, &k)));
    }
}
