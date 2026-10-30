//! The faction component actions that the core owns (package W11, lent-bodies lane 5): each is a function over the state or a
//! [`crate::kernel::stage::Window`] in its own file section. The listing of the actions and the dispatch (`perform_component`)
//! are the driver's (D1b); the adapter calls the bodies from the string dispatch and compares.

use crate::generated::{System, Technology, Unit};
use crate::ids::Seat;
use crate::kernel::{fx, fx_strategy};
use crate::query::{board, fleet};
use crate::state::CoreState;
use crate::tables::Scope;

/// Lazax Gate Folding (Winnu): "ACTION: Exhaust this card to place 1 infantry from your reinforcements on Mecatol Rex" (when the
/// seat controls it). No question: the planet is the first the seat controls in the Mecatol system (`mecatol`), the unit is the
/// form the seat places (`None`: it places nothing, or a faction rule forbids it). Returns whether the action resolved: the card is
/// ready, the seat controls a planet there and the box holds an infantry for it; the card is exhausted even when the unit then
/// cannot be placed (the string exhausts first), in which case the action did not resolve.
pub fn lazax_gate_folding(core: &mut CoreState, scope: Scope, who: Seat, mecatol: System, form: Option<Unit>) -> bool {
    let ready = core.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(Technology::LazaxGateFolding) && !p.tech.exhausted.contains(Technology::LazaxGateFolding));
    if !ready {
        return false;
    }
    let Some(planet) = board::controlled_planets(core, who).as_slice().iter().find(|(system, _)| *system == mecatol).map(|(_, planet)| *planet) else {
        return false;
    };
    let Some(unit) = form else { return false };
    if fleet::remaining_all(core, scope, who, unit) <= 0 {
        return false;
    }
    fx_strategy::exhaust_technology(core, who, Technology::LazaxGateFolding);
    fx::place_unit(core, who, mecatol, Some(planet), unit);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{Faction, Planet, UnitBaseType};
    use crate::state::{Player, Players};

    fn table() -> CoreState {
        let mut s = CoreState::all_loaded();
        let list = (0..2u8).map(|i| Player::new(Seat(i), Faction::from_index(usize::from(i)).unwrap())).collect();
        s.players = Players::from_vec(list);
        s
    }

    fn infantry() -> Unit {
        Unit::ALL.iter().copied().find(|u| u.data().base_type == UnitBaseType::Infantry && u.data().faction.is_none()).expect("infantry")
    }

    /// Two systems with a planet each, in system order.
    fn two() -> [(System, Planet); 2] {
        let mut found: Vec<(System, Planet)> = Vec::new();
        for planet in Planet::ALL {
            if let Some(system) = planet.data().system
                && !found.iter().any(|(s, _)| *s == system)
            {
                found.push((system, planet));
            }
        }
        found.sort_unstable();
        [found[0], found[1]]
    }

    fn ready(s: &mut CoreState) {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::LazaxGateFolding);
    }

    fn standing(s: &CoreState, planet: Planet) -> usize {
        s.board().planet(planet).units.as_slice().iter().filter(|p| p.owner() == Seat(0)).count()
    }

    #[test]
    fn lazax_gate_folding_exhausts_the_card_and_places_the_infantry_on_the_planet_held_in_the_mecatol_system() {
        let [(mecatol, planet), (other, elsewhere)] = two();
        let mut s = table();
        ready(&mut s);
        for p in [planet, elsewhere] {
            s.board_mut().planet_mut(p).control = Some(Seat(0));
        }
        assert!(lazax_gate_folding(&mut s, Scope(u32::MAX), Seat(0), mecatol, Some(infantry())));
        assert_eq!((standing(&s, planet), standing(&s, elsewhere)), (1, 0), "on the planet of that system only");
        assert!(s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::LazaxGateFolding));
        assert!(!lazax_gate_folding(&mut s, Scope(u32::MAX), Seat(0), mecatol, Some(infantry())), "exhausted");
        assert_eq!(standing(&s, planet), 1);
        assert_ne!(other, mecatol);
    }

    #[test]
    fn lazax_gate_folding_changes_nothing_without_the_card_the_planet_or_a_unit_to_place() {
        let [(mecatol, planet), (other, elsewhere)] = two();
        let run = |own: bool, held: Option<Planet>, form: Option<Unit>| {
            let mut s = table();
            if own {
                ready(&mut s);
            }
            if let Some(p) = held {
                s.board_mut().planet_mut(p).control = Some(Seat(0));
            }
            let before = s.clone();
            let done = lazax_gate_folding(&mut s, Scope(u32::MAX), Seat(0), mecatol, form);
            assert!(!done);
            assert_eq!(s, before, "nothing written");
        };
        run(false, Some(planet), Some(infantry()));
        run(true, None, Some(infantry()));
        run(true, Some(elsewhere), Some(infantry()));
        run(true, Some(planet), None);
        assert_ne!(other, mecatol);
    }
}
