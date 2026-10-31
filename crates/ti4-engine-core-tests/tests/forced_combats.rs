//! W12: `rules::invasion::forced::ForcedCombats`, the combats Titans' Coalescence forces at the end of the commitment and Slumberstate
//! Computing's coexist question.

use ti4_engine_core::choice::invasion::{Ask as IAsk, Opt as IOpt};
use ti4_engine_core::generated::{Breakthrough, Planet};
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::rules::invasion::forced::ForcedCombats;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

/// A system with one planet.
fn one() -> (System, Planet) {
    plain_systems()
        .into_iter()
        .find_map(|s| (s.data().planets.len() == 1).then(|| (s, s.data().planets[0])))
        .expect("a one-planet system")
}

/// Seat 0 (Titans) and seat 1 both have infantry on the planet; seat 1 controls it; Awaken put seat 0's units there this activation.
fn scene(breakthrough: bool, rival: bool) -> (CoreState, Kernel, System, Planet) {
    let (system, planet) = one();
    let mut sc = Scenario::new(&[Faction::TheTitansOfUl, Faction::TheArborec]).map_from(&[system]).control(planet, 1).unit(system, Some(planet), Unit::InfantryI, 0, 1);
    if rival {
        sc = sc.unit(system, Some(planet), Unit::InfantryI, 1, 2);
    }
    let (mut state, kernel) = sc.build();
    if breakthrough {
        state.players_mut().by_seat_mut(Seat(0)).unwrap().cards.breakthrough = Some(Breakthrough::SlumberstateComputing);
    }
    let now = state.meta().activation_seq;
    state
        .marks_mut()
        .tail
        .edit()
        .insert(MarkKey::TitansCoalescence(planet), MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(now)), MarkVal::Atom(Atom::System(system))]));
    (state, kernel, system, planet)
}

fn table(breakthrough: bool, rival: bool) -> (Rig<ForcedCombats>, System, Planet) {
    let (state, kernel, system, planet) = scene(breakthrough, rival);
    (Rig::from_parts(state, kernel, ForcedCombats::new(Seat(0), system, Vec::new())), system, planet)
}

#[test]
fn coalescence_adds_the_planet_without_a_question_when_the_invader_holds_no_breakthrough() {
    covers!("rules::invasion::forced::ForcedCombats");
    let (mut rig, _, planet) = table(false, true);
    rig.run();
    assert!(rig.is_done(), "no Slumberstate, no question");
    assert_eq!(rig.driver.window.added, vec![planet]);
}

#[test]
fn slumberstate_asks_fight_or_coexist_and_coexisting_adds_nothing_and_writes_the_coexistence() {
    covers!("rules::invasion::forced::ForcedCombats");
    let (mut rig, system, planet) = table(true, true);
    rig.run();
    assert_eq!((rig.asked().actor, rig.asked().ask), (Seat(0), Ask::Invasion(IAsk::CoexistInstead { system, planet, holder: Seat(1) })));
    assert_eq!(rig.opts(), vec![Opt::Invasion(IOpt::CoexistFight), Opt::Invasion(IOpt::CoexistInstead)]);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Invasion(IOpt::CoexistInstead));
    assert!(rig.is_done());
    assert!(rig.driver.window.added.is_empty(), "the units coexist: no combat");
    assert!(rig.state.board().planet(planet).coexist.contains(Seat(0)), "the invader coexists");
    assert_eq!(rig.state.board().planet(planet).control, Some(Seat(1)), "the controller keeps the planet");
}

#[test]
fn fighting_adds_the_planet_and_a_planet_nobody_else_holds_asks_nothing() {
    covers!("rules::invasion::forced::ForcedCombats");
    let (mut rig, _, planet) = table(true, true);
    rig.run();
    rig.pick_opt(Opt::Invasion(IOpt::CoexistFight));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.added, vec![planet]);
    assert!(!rig.state.board().planet(planet).coexist.contains(Seat(0)));
    // No rival ground force there: the combat is not a combat, nothing to coexist with; the planet is added at once.
    let (mut rig, _, planet) = table(true, false);
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.added, vec![planet]);
}

#[test]
fn units_committed_elsewhere_skip_the_question_and_a_planet_already_committed_is_not_added_twice() {
    covers!("rules::invasion::forced::ForcedCombats");
    let (state, kernel, system, planet) = scene(true, true);
    // "If you commit no other units": a commitment that holds another planet asks nothing, and adds the forced planet.
    let other = plain_systems().into_iter().flat_map(|s| s.data().planets.iter().copied().collect::<Vec<_>>()).find(|p| *p != planet).unwrap();
    let mut rig = Rig::from_parts(state, kernel, ForcedCombats::new(Seat(0), system, vec![other]));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.added, vec![planet]);
    // The forced planet is committed already: nothing is added and nothing asked.
    let (state, kernel, ..) = scene(true, true);
    let mut rig = Rig::from_parts(state, kernel, ForcedCombats::new(Seat(0), system, vec![planet]));
    rig.run();
    assert!(rig.is_done());
    assert!(rig.driver.window.added.is_empty());
}
