//! The faction component actions that the core owns (package W11, lent-bodies lane 5): each is a function over the state or a
//! [`crate::kernel::stage::Window`] in its own file section. The listing of the actions and the dispatch (`perform_component`)
//! are the driver's (D1b); the adapter calls the bodies from the string dispatch and compares.

use crate::generated::{Planet, System, Technology, Unit};
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

/// ???_ERROR_ERROR_??? (Nekro Virus), the ACTION that repairs: "Exhaust this card to repair all of your damaged units." No question: every
/// unit of the seat, in the space areas and on the planets, is repaired and the card is exhausted. `false` (nothing written) when the seat
/// does not own the card; the offer (a ready card and a damaged unit) is the caller's.
pub fn c4r_repair(core: &mut CoreState, who: Seat) -> bool {
    if !fx_strategy::exhaust_technology(core, who, Technology::ERRORERROR) {
        return false;
    }
    let damaged = |list: &crate::state::UnitList| list.as_slice().iter().any(|p| p.owner() == who && p.sustained());
    let repair = |list: &mut crate::state::UnitList| {
        for piece in list.as_mut_slice() {
            if piece.owner() == who {
                *piece = piece.with_sustained(false);
            }
        }
    };
    let systems: Vec<System> = core.board().systems().filter(|(_, c)| damaged(&c.units)).map(|(s, _)| s).collect();
    let planets: Vec<crate::generated::Planet> = core.board().planets().filter(|(_, c)| damaged(&c.units)).map(|(p, _)| p).collect();
    for system in systems {
        repair(&mut core.board_mut().sys_mut(system).units);
    }
    for planet in planets {
        repair(&mut core.board_mut().planet_mut(planet).units);
    }
    true
}

/// Terraform (the Titans promissory note, a component action of its holder): "Attach this card to a non-home planet you control other
/// than Mecatol Rex: +1 resource, +1 influence, and all planet traits" once, anywhere in the game; the card is then in play, no longer a
/// note in hand. No question here (the planet is the option the caller chose): every condition is read again, and nothing is written
/// unless all hold. `home` says the planet is somebody's home planet (the seated homes and the printed homeworlds: a string read).
pub fn terraform(core: &mut CoreState, who: Seat, planet: Planet, home: bool) -> bool {
    use crate::generated::{Attachment, PromissoryNote};
    use crate::state::{NoteKey, PlanetFlags};
    let Some(own) = core.players().by_seat(who).map(|p| p.faction) else { return false };
    let Some(note) = core
        .cards()
        .promissory
        .iter()
        .find(|(key, holder)| **holder == who && key.note == PromissoryNote::Terraform && key.faction != own)
        .map(|(key, _)| *key)
    else {
        return false;
    };
    let attached = core.board().planets().any(|(_, c)| c.attach.as_slice().contains(&Attachment::Terraform));
    let controlled = board::controlled_planets(core, who).as_slice().iter().any(|(_, p)| *p == planet);
    if attached || !controlled || home || matches!(planet, Planet::MecatolRex | Planet::MecatolRexTe) {
        return false;
    }
    let cell = core.board_mut().planet_mut(planet);
    cell.attach.push(Attachment::Terraform);
    cell.flags.set(PlanetFlags::ATTACH_ENTRY, true);
    let note: NoteKey = note;
    core.cards_mut().promissory_faceup.edit().insert(note, ());
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

    #[test]
    fn the_nekro_repair_repairs_every_unit_of_the_seat_in_space_and_on_planets_and_exhausts_the_card() {
        let [(system, planet), (other, elsewhere)] = two();
        let mut s = table();
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::ERRORERROR);
        let ship = Unit::ALL.iter().copied().find(|u| u.data().base_type == UnitBaseType::Dreadnought && u.data().faction.is_none()).expect("a dreadnought");
        let damaged = |seat: Seat, unit: Unit| crate::state::Piece::new(unit, seat).with_sustained(true);
        s.board_mut().sys_mut(system).units.push(damaged(Seat(0), ship));
        s.board_mut().sys_mut(other).units.push(damaged(Seat(1), ship));
        s.board_mut().planet_mut(planet).units.push(damaged(Seat(0), ship));
        s.board_mut().planet_mut(elsewhere).units.push(damaged(Seat(0), ship));
        s.board_mut().planet_mut(elsewhere).units.push(damaged(Seat(1), ship));
        assert!(c4r_repair(&mut s, Seat(0)));
        let sustained = |s: &CoreState, seat: Seat| -> usize {
            let mut all = s.board().systems().map(|(_, c)| c.units.as_slice().to_vec()).collect::<Vec<_>>();
            all.extend(s.board().planets().map(|(_, c)| c.units.as_slice().to_vec()));
            all.iter().flatten().filter(|p| p.owner() == seat && p.sustained()).count()
        };
        assert_eq!((sustained(&s, Seat(0)), sustained(&s, Seat(1))), (0, 2), "only the seat's own units are repaired");
        assert!(s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::ERRORERROR));
    }

    #[test]
    fn the_nekro_repair_writes_nothing_for_a_seat_without_the_card() {
        let [(system, _), _] = two();
        let mut s = table();
        let ship = Unit::ALL.iter().copied().find(|u| u.data().base_type == UnitBaseType::Dreadnought && u.data().faction.is_none()).expect("a dreadnought");
        s.board_mut().sys_mut(system).units.push(crate::state::Piece::new(ship, Seat(0)).with_sustained(true));
        let before = s.clone();
        assert!(!c4r_repair(&mut s, Seat(0)));
        assert_eq!(s, before);
    }

    #[test]
    fn terraform_attaches_the_card_once_to_a_controlled_non_home_planet_and_turns_the_note_faceup() {
        use crate::generated::{Attachment, PromissoryNote};
        use crate::state::NoteKey;
        let [(_, planet), (_, other)] = two();
        let note = NoteKey { note: PromissoryNote::Terraform, faction: Faction::from_index(1).unwrap() };
        let setup = || {
            let mut s = table();
            s.cards_mut().promissory.edit().insert(note, Seat(0));
            for p in [planet, other] {
                s.board_mut().planet_mut(p).control = Some(Seat(0));
            }
            s
        };
        let refused = |mut s: CoreState, p: Planet, home: bool| {
            let before = s.clone();
            assert!(!terraform(&mut s, Seat(0), p, home));
            assert_eq!(s, before, "nothing written");
        };
        let mut s = setup();
        assert!(terraform(&mut s, Seat(0), planet, false));
        assert!(s.board().planet(planet).attach.as_slice().contains(&Attachment::Terraform));
        assert!(s.cards().promissory_faceup.get(&note).is_some(), "the card is in play");
        assert!(!terraform(&mut s, Seat(0), other, false), "once in the game, on any planet");
        // The conditions, one at a time.
        refused(setup(), planet, true);
        refused(setup(), Planet::MecatolRex, false);
        let mut not_mine = setup();
        not_mine.board_mut().planet_mut(planet).control = Some(Seat(1));
        refused(not_mine, planet, false);
        let mut no_note = setup();
        no_note.cards_mut().promissory.edit().insert(note, Seat(1));
        refused(no_note, planet, false);
        let mut own_note = setup();
        own_note.cards_mut().promissory.edit().remove(&note);
        own_note.cards_mut().promissory.edit().insert(NoteKey { note: PromissoryNote::Terraform, faction: Faction::from_index(0).unwrap() }, Seat(0));
        refused(own_note, planet, false);
        let mut elsewhere = setup();
        elsewhere.board_mut().planet_mut(other).attach.push(Attachment::Terraform);
        refused(elsewhere, planet, false);
    }
}
