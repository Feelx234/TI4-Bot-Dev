//! Silent gaps found by the hook census (`silent_census.rs`): factions whose rule is registered on a seam only, while the combat rules read the legacy slot only.
//!
//! 1. The Nomad's Quantum Manipulator and The Cavalry give SUSTAIN DAMAGE through the seam
//! `grants_sustain` (the module registers nothing else), but the combat rules read only the legacy slot `combat.grants_sustain`, which no module fills in
//! the native kernel: in a native game the mech never sustained a hit in space. (The module's own tests call its function directly.)
//! 2. Mentak's Moll Terminus ("other players' ground forces on this planet cannot use SUSTAIN DAMAGE") registers `seam.may_sustain`; the ground fight read
//! `ground.may_sustain` only.

use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::rules::combat::{sustains_in_space, Rd};
use ti4_engine_core::state::Piece;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

#[test]
fn the_quantum_manipulator_sustains_a_hit_in_space_in_the_native_kernel() {
    covers!("rules::combat::sustains_in_space");
    let at = plain_systems()[0];
    let (mut s, k) = Scenario::new(&[Faction::TheNomad, Faction::TheArborec]).map_from(&[at]).unit(at, None, Unit::CruiserI, 1, 1).build();
    // A mech is not a ship on a planet: it stands in the space area as cargo.
    s.board_mut().sys_mut(at).units.push(Piece::new(Unit::QuantumManipulator, Seat(0)));
    let mech = Piece::new(Unit::QuantumManipulator, Seat(0));
    assert!(sustains_in_space(Rd::new(&s, &k), at, Seat(0), mech, true), "while in a space area during combat");
    assert!(!sustains_in_space(Rd::new(&s, &k), at, Seat(0), mech, false), "a SPACE CANNON OFFENSE hit is not a combat hit");
    // Another seat's mech of the same type is no Nomad's, and a cruiser of the Arborec has no sustain.
    let other = Piece::new(Unit::CruiserI, Seat(1));
    assert!(!sustains_in_space(Rd::new(&s, &k), at, Seat(1), other, true));
}

#[test]
fn the_cavalry_gives_the_chosen_ship_type_sustain_while_it_is_undamaged() {
    covers!("rules::combat::sustains_in_space");
    let at = plain_systems()[0];
    let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheNomad]).map_from(&[at]).unit(at, None, Unit::CruiserI, 0, 1).unit(at, None, Unit::CruiserI, 1, 1).build();
    let cruiser = Piece::new(Unit::CruiserI, Seat(0));
    assert!(!sustains_in_space(Rd::new(&s, &k), at, Seat(0), cruiser, true), "no note: no sustain");
    let note = ti4_engine_core::state::NoteKey { note: PromissoryNote::TheCavalry, faction: Faction::TheNomad };
    s.cards_mut().promissory.edit().insert(note, Seat(0));
    s.cards_mut().promissory_faceup.edit().insert(note, ());
    s.marks_mut().tail.edit().insert(MarkKey::NomadCavalry(Seat(0)), MarkVal::Rec(vec![MarkVal::Atom(Atom::System(at)), MarkVal::Atom(Atom::Unit(Unit::CruiserI))]));
    assert!(sustains_in_space(Rd::new(&s, &k), at, Seat(0), cruiser, true), "the note was played on this ship type here");
}

#[test]
fn moll_terminus_takes_sustain_damage_from_the_other_seats_ground_forces_on_its_planet_in_the_native_kernel() {
    covers!("rules::invasion::can_sustain_here");
    use ti4_engine_core::rules::invasion::can_sustain_here;
    let system = plain_systems().into_iter().find(|s| s.data().planets.len() >= 2).expect("a two-planet tile");
    let (here, elsewhere) = (system.data().planets[0], system.data().planets[1]);
    // A ground force with SUSTAIN DAMAGE of the Arborec (seat 1).
    let mech = Unit::ALL.iter().copied().find(|u| u.data().sustain_damage && u.data().base_type == UnitBaseType::Mech && !u.data().ship).expect("a mech with SUSTAIN DAMAGE");
    let piece = Piece::new(mech, Seat(1));
    let build = |terminus: bool| {
        let mut sc = Scenario::new(&[Faction::TheMentakCoalition, Faction::TheArborec]).map_from(&[system]).unit(system, Some(here), Unit::InfantryI, 1, 1).unit(system, Some(elsewhere), Unit::InfantryI, 1, 1);
        if terminus {
            sc = sc.unit(system, Some(here), Unit::MollTerminus, 0, 1);
        }
        let (mut s, k) = sc.build();
        s.board_mut().planet_mut(here).units.push(piece);
        s.board_mut().planet_mut(elsewhere).units.push(piece);
        (s, k)
    };
    let (s, k) = build(false);
    assert!(can_sustain_here(Rd::new(&s, &k), piece, system, here), "without the mech the Arborec mech sustains");
    let (s, k) = build(true);
    assert!(!can_sustain_here(Rd::new(&s, &k), piece, system, here), "on the planet of Moll Terminus it cannot");
    assert!(can_sustain_here(Rd::new(&s, &k), piece, system, elsewhere), "on another planet it can");
}
