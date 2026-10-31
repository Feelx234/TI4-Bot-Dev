//! Package W14: Greyfire Mutagen (`cd_registry::yin::timing::GREYFIRE`), the Yin's promissory note, as a rule of the core table: "At the start of a
//! ground combat against 2 or more ground forces that are not controlled by the Yin player: Replace 1 of your opponent's infantry with 1 infantry
//! from your reinforcements. Then, return this card to the Yin player."

use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::state::{EventKind, Key, NoteKey, Payload, Val};
use ti4_engine_core_factions::cd_registry::yin::timing::GREYFIRE;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;
use ti4_engine_core_tests::windows::EmitEvent;

const NOTE: NoteKey = NoteKey { note: PromissoryNote::GreyfireMutagen, faction: Faction::TheYinBrotherhood };

fn table() -> AbilityTable {
    let mut t = AbilityTable::default();
    ti4_engine_core_factions::cd_registry::register_abilities(&mut t);
    t
}

/// Seat 0 (Sol) holds the note and attacks seat 1 (Hacan) on the first planet of a plain system; seat 2 is the Yin. Seat 1 holds `ground` ground
/// forces there, `infantry` of them infantry.
fn table_rig(holder: Faction, opponent: Faction, ground: &[Unit]) -> (Rig<EmitEvent>, System, Planet) {
    let system = plain_systems()[0];
    let planet = system.data().planets[0];
    let mut sc = Scenario::new(&[holder, opponent, Faction::TheYinBrotherhood]).kernel(kernel_all()).map_from(&[system]).control(planet, 1);
    for unit in ground {
        sc = sc.unit(system, Some(planet), *unit, 1, 1);
    }
    let (mut state, kernel) = sc.build();
    state.cards_mut().promissory.edit().insert(NOTE, Seat(0));
    let payload = Payload::new()
        .with(Key::System, Val::System(system))
        .with(Key::Planet, Val::Planet(planet))
        .with(Key::Attacker, Val::Seat(Seat(0)))
        .with(Key::Defender, Val::Seat(Seat(1)));
    let rig = Rig::from_parts(state, kernel, EmitEvent::new(fx::event(EventKind::GroundCombatStarted, payload))).with_abilities(table());
    (rig, system, planet)
}

fn ground_of(rig: &Rig<EmitEvent>, planet: Planet, seat: u8) -> Vec<Unit> {
    rig.state.board().planet(planet).units.iter().filter(|p| p.owner() == Seat(seat)).map(|p| p.kind()).collect()
}

#[test]
fn greyfire_replaces_an_infantry_of_the_opponent_then_returns_the_note_to_the_yin() {
    covers!("cd_registry::yin::timing::GREYFIRE");
    let (mut rig, _, planet) = table_rig(Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan, &[Unit::InfantryI, Unit::InfantryI]);
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0), "the holder is asked whether to react: {}", rig.transcript());
    assert_eq!(GREYFIRE.0, 7004);
    let use_it = rig.opts().iter().position(|o| *o != Opt::Decline).expect("a use option");
    rig.pick(use_it);
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(ground_of(&rig, planet, 1), vec![Unit::InfantryI], "one of the opponent's two infantry went");
    assert_eq!(ground_of(&rig, planet, 0), vec![Unit::InfantryI], "the holder's infantry took its place");
    assert_eq!(rig.state.cards().promissory.get(&NOTE), Some(&Seat(2)), "the card is back with the Yin player");
}

#[test]
fn greyfire_declined_changes_nothing_and_the_note_stays() {
    covers!("cd_registry::yin::timing::GREYFIRE");
    let (mut rig, _, planet) = table_rig(Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan, &[Unit::InfantryI, Unit::InfantryI]);
    rig.run();
    rig.decline();
    assert!(rig.is_done());
    assert_eq!(ground_of(&rig, planet, 1).len(), 2);
    assert!(ground_of(&rig, planet, 0).is_empty());
    assert_eq!(rig.state.cards().promissory.get(&NOTE), Some(&Seat(0)));
}

#[test]
fn greyfire_needs_two_ground_forces_an_infantry_a_non_yin_opponent_and_a_non_yin_holder() {
    covers!("cd_registry::yin::timing::GREYFIRE");
    // One ground force alone is not "2 or more".
    let (mut rig, _, _) = table_rig(Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan, &[Unit::InfantryI]);
    rig.run();
    assert!(rig.is_done(), "a single ground force");
    // Two mechs and no infantry: nothing to replace.
    let (mut rig, _, _) = table_rig(Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan, &[Unit::FactionlessMech, Unit::FactionlessMech]);
    rig.run();
    assert!(rig.is_done(), "no infantry to replace");
    // The opponent is the Yin: the card is for the fights against the others.
    let (mut rig, _, _) = table_rig(Faction::TheFederationOfSol, Faction::TheYinBrotherhood, &[Unit::InfantryI, Unit::InfantryI]);
    rig.state.cards_mut().promissory.edit().insert(NOTE, Seat(0));
    rig.run();
    assert!(rig.is_done(), "against the Yin");
    // The note is not held.
    let (mut rig, _, _) = table_rig(Faction::TheFederationOfSol, Faction::TheEmiratesOfHacan, &[Unit::InfantryI, Unit::InfantryI]);
    rig.state.cards_mut().promissory.edit().insert(NOTE, Seat(2));
    rig.run();
    assert!(rig.is_done(), "the Yin holds its own note");
}
