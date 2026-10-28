//! deepwrought: tier Q: pure hooks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CB). Owned by CB only.
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | `direct.deepwrought_oceans` | `oceans` | Q | `direct` (a `concat`) |
//! | `direct.deepwrought_spendable_oceans` | `spendable_oceans` | Q | `direct` (a `concat`; the seam `planet_spend` is B3's) |
//! | `direct.deepwrought_research_team_open` | `research_team_open` | Q | `direct` (a `call`) |
//! | `direct.deepwrought_research_team_defender_open` | `research_team_defender_open` | Q | `direct` |
//! | `waived_prerequisites` | `waived_prerequisites` | Q | `general.waived_prerequisites` (Carrina's mark) |
//! | `direct.deepwrought_note_coexistence_began`, `begin_coexisting` | | O | `observe.rs` |
//! | `direct.deepwrought_research_open`, `research_settle`, `research_counts` | | W | `offer.rs` (the Carrina window, B-research) |
//! | `component_actions`, `perform_component`, leaders, `timing_abilities` | | W | B7 / A5 |
//! | movement `own_unit_passage` | | Q | `movement.rs` (A2, done) |

use ti4_engine_core::generated::{Ability, Planet, System, Technology};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::hooks::direct::Planets;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::board;
use ti4_engine_core::state::CoreState;

use crate::common::{mark, player};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.direct.deepwrought_oceans.push(oceans);
    t.direct.deepwrought_spendable_oceans.push(spendable_oceans);
    t.direct.deepwrought_research_team_open.push(research_team_open);
    t.direct.deepwrought_research_team_defender_open.push(research_team_defender_open);
    t.general.waived_prerequisites.push(waived_prerequisites);
}

/// Ocean cards `who` holds in the state, lowest id first, before the Oceanbound cap.
fn stored_oceans(s: &CoreState, who: Seat) -> Vec<Planet> {
    s.marks()
        .tail
        .iter()
        .filter_map(|(key, value)| match (key, value) {
            (MarkKey::DeepwroughtOcean(planet), MarkVal::Atom(Atom::Seat(holder))) if *holder == who => Some(*planet),
            _ => None,
        })
        .collect()
}

/// How many planets contain `who`'s coexisting units: the player is coexisting there and still has a unit on it.
fn coexisting_planets(s: &CoreState, who: Seat) -> usize {
    s.board().planets().filter(|(_, cell)| cell.coexist.contains(who) && cell.units.as_slice().iter().any(|u| u.owner() == who)).count()
}

/// The ocean cards `who` has in play: what they hold, cut to Oceanbound's cap.
pub fn oceans(s: &CoreState, who: Seat) -> Planets {
    let mut held = stored_oceans(s, who);
    held.truncate(coexisting_planets(s, who));
    held.into_iter().collect()
}

/// The holder's readied ocean cards, spendable like any planet.
pub fn spendable_oceans(s: &CoreState, who: Seat) -> Planets {
    oceans(s, who).as_slice().iter().copied().filter(|p| !board::exhausted(s, *p)).collect()
}

fn has_research_team(s: &CoreState, who: Seat) -> bool {
    player(s, who).is_some_and(|p| p.faction.data().abilities.contains(&Ability::ResearchTeam))
}

/// Whether Research Team's choice is open for `invader`'s ground forces committed to `planet`: the ability,
/// another player controlling the planet, and not already coexisting there.
pub fn research_team_open(s: &CoreState, _: &Kernel, invader: Seat, system: System, planet: Planet) -> bool {
    has_research_team(s, invader)
        && !board::is_coexisting(s, system, planet, invader)
        && board::control(s, planet).is_some_and(|holder| holder != invader)
}

/// Whether Research Team's choice is open for `defender`, whose units stand on `planet`.
pub fn research_team_defender_open(s: &CoreState, _: &Kernel, defender: Seat, invader: Seat, system: System, planet: Planet) -> bool {
    defender != invader
        && has_research_team(s, defender)
        && !board::is_coexisting(s, system, planet, defender)
        && board::planet_units(s, planet).iter().any(|u| u.owner() == defender)
}

/// Doctor Carrina's waiver: one prerequisite ignored while the researcher's mark is up.
pub fn waived_prerequisites(s: &CoreState, _: &Kernel, who: Seat, _: Technology) -> usize {
    usize::from(mark(s, MarkKey::DeepwroughtCarrina(who)).is_some())
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::cb_economy::testkit::table;
    use ti4_engine_core::state::Piece;
    use ti4_engine_core::keys::{Atom, MarkVal};

    const D: Faction = Faction::TheDeepwroughtScholarate;
    const SOL: Faction = Faction::TheFederationOfSol;

    fn coexist(s: &mut CoreState, who: Seat, planet: Planet) {
        let cell = s.board_mut().planet_mut(planet);
        cell.coexist.insert(who);
        cell.units.push(Piece::new(Unit::InfantryI, who));
    }

    fn hold(s: &mut CoreState, who: Seat, ocean: Planet) {
        s.marks_mut().tail.edit().insert(MarkKey::DeepwroughtOcean(ocean), MarkVal::Atom(Atom::Seat(who)));
    }

    use ti4_engine_core::generated::Unit;
    use ti4_engine_core::generated::Faction;

    #[test]
    fn oceanbound_cuts_the_cards_to_the_planets_with_coexisting_units() {
        let (mut s, _) = table(&[D, SOL]);
        hold(&mut s, Seat(0), Planet::LostFleet);
        hold(&mut s, Seat(0), Planet::CoralReef);
        hold(&mut s, Seat(1), Planet::IceShelf);
        assert!(oceans(&s, Seat(0)).is_empty(), "no coexisting planet: no card in play");
        coexist(&mut s, Seat(0), Planet::Abaddon);
        assert_eq!(oceans(&s, Seat(0)).as_slice(), &[Planet::CoralReef], "lowest id first, cut to the cap of 1");
        coexist(&mut s, Seat(0), Planet::Abyz);
        assert_eq!(oceans(&s, Seat(0)).as_slice(), &[Planet::CoralReef, Planet::LostFleet]);
        s.board_mut().planet_mut(Planet::CoralReef).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true);
        assert_eq!(spendable_oceans(&s, Seat(0)).as_slice(), &[Planet::LostFleet], "an exhausted card is not spendable");
    }

    #[test]
    fn research_team_works_on_offence_and_defence() {
        let (mut s, k) = table(&[D, SOL]);
        let sys = ti4_engine_core::ids::system_at(30);
        s.board_mut().planet_mut(Planet::Abaddon).control = Some(Seat(1));
        assert!(research_team_open(&s, &k, Seat(0), sys, Planet::Abaddon));
        assert!(!research_team_open(&s, &k, Seat(1), sys, Planet::Abaddon), "Sol has no Research Team");
        s.board_mut().planet_mut(Planet::Abaddon).control = Some(Seat(0));
        assert!(!research_team_open(&s, &k, Seat(0), sys, Planet::Abaddon), "your own planet");
        s.board_mut().planet_mut(Planet::Abaddon).units.push(Piece::new(Unit::InfantryI, Seat(0)));
        assert!(research_team_defender_open(&s, &k, Seat(0), Seat(1), sys, Planet::Abaddon));
        assert!(!research_team_defender_open(&s, &k, Seat(0), Seat(0), sys, Planet::Abaddon), "not against yourself");
    }

    #[test]
    fn carrinas_waiver_counts_one_while_the_mark_is_up() {
        let (mut s, k) = table(&[D, SOL]);
        assert_eq!(waived_prerequisites(&s, &k, Seat(0), Technology::ALL[0]), 0);
        hold_carrina(&mut s);
        assert_eq!(waived_prerequisites(&s, &k, Seat(0), Technology::ALL[0]), 1);
        assert_eq!(waived_prerequisites(&s, &k, Seat(1), Technology::ALL[0]), 0);
    }

    fn hold_carrina(s: &mut CoreState) {
        s.marks_mut().tail.edit().insert(MarkKey::DeepwroughtCarrina(Seat(0)), MarkVal::Atom(Atom::Seat(Seat(1))));
    }
}
