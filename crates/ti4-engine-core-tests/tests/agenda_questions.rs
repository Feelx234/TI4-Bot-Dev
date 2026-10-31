//! D4 lane A2: the agenda effects that ask a question (`rules::agenda::effects::questions`): Seed of an Empire (the speaker breaks a tie) and
//! Homeland Defense Act, Against (every player with several PDS picks the one to destroy), against the agenda texts.

use ti4_engine_core::choice::agenda::{Ask as AAsk, Opt as AOpt};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::rules::agenda::effects::Resolve;
use ti4_engine_core::rules::agenda::table_effects::EffectFacts;
use ti4_engine_core::state::{Code, Outcome};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn facts() -> EffectFacts {
    EffectFacts {
        for_voters: SmallList::new(),
        against_voters: SmallList::new(),
        mahact_held: [0; 8],
        scope: ALL_SOURCES,
        default_scope: ALL_SOURCES,
        source: Code(1),
        pool: Code(2),
        has_galaxy: true,
        defense_act: Code(3),
        hooked: 0,
        drop_unofficial: false,
        blanked: false,
        roster: None,
        redistribution: Code(4),
    }
}

fn start(sc: Scenario, agenda: Agenda, outcome: Outcome) -> Rig<Resolve> {
    let window = Resolve::of_facts(agenda, outcome, &facts()).unwrap_or_else(|| panic!("an effect for {agenda:?} {outcome:?}"));
    let mut rig = Rig::from_parts(sc.state(), kernel_all(), window);
    rig.run();
    rig
}

fn points(rig: &Rig<Resolve>) -> Vec<i16> {
    (0..4).map(|i| rig.state.players().by_seat(Seat(i)).map_or(-1, |p| p.econ.victory_points)).collect()
}

#[test]
fn seed_of_an_empire_for_gives_a_point_to_the_player_with_the_most_and_against_to_the_one_with_the_fewest() {
    covers!("rules::agenda::effects::Resolve");
    let build = |vp: &[i16]| {
        let mut s = Scenario::lobby(4).state();
        for (i, v) in vp.iter().enumerate() {
            s.players_mut().by_seat_mut(Seat(i as u8)).unwrap().econ.victory_points = *v;
        }
        s
    };
    let run = |s: CoreState, outcome: Outcome| {
        let window = Resolve::of_facts(Agenda::SeedOfAnEmpire, outcome, &facts()).expect("an effect");
        let mut rig = Rig::from_parts(s, kernel_all(), window);
        rig.run();
        rig
    };
    let rig = run(build(&[2, 5, 3, 1]), Outcome::For);
    assert!(rig.is_done());
    assert_eq!(points(&rig), vec![2, 6, 3, 1], "For: the most points gains one");
    let rig = run(build(&[2, 5, 3, 1]), Outcome::Against);
    assert_eq!(points(&rig), vec![2, 5, 3, 2], "Against: the fewest gains one");
    // A leader on ten does not go above the target.
    let rig = run(build(&[10, 5, 3, 1]), Outcome::For);
    assert_eq!(points(&rig)[0], 10, "no more than ten points");
}

#[test]
fn a_tie_for_the_most_or_fewest_points_is_broken_by_the_speaker_among_the_tied() {
    covers!("rules::agenda::effects::Resolve");
    let mut s = Scenario::lobby(4).speaker(2).state();
    for (i, v) in [4i16, 4, 1, 4].iter().enumerate() {
        s.players_mut().by_seat_mut(Seat(i as u8)).unwrap().econ.victory_points = *v;
    }
    let window = Resolve::of_facts(Agenda::SeedOfAnEmpire, Outcome::For, &facts()).unwrap();
    let mut rig = Rig::from_parts(s, kernel_all(), window);
    rig.run();
    assert_eq!(rig.asked().actor, Seat(2), "the speaker names the player");
    assert_eq!(rig.asked().ask, Ask::Agenda(AAsk::NameTied));
    assert_eq!(rig.opts(), vec![Opt::Agenda(AOpt::Elect(Seat(0))), Opt::Agenda(AOpt::Elect(Seat(1))), Opt::Agenda(AOpt::Elect(Seat(3)))], "only the tied players, in seat order");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(Opt::Agenda(AOpt::Elect(Seat(2)))));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.pick_opt(Opt::Agenda(AOpt::Elect(Seat(3))));
    assert!(rig.is_done());
    assert_eq!(points(&rig), vec![4, 4, 1, 5]);
    // Against: the lowest is alone.
    let mut s = Scenario::lobby(3).state();
    for (i, v) in [3i16, 1, 3].iter().enumerate() {
        s.players_mut().by_seat_mut(Seat(i as u8)).unwrap().econ.victory_points = *v;
    }
    let window = Resolve::of_facts(Agenda::SeedOfAnEmpire, Outcome::Against, &facts()).unwrap();
    let mut rig = Rig::from_parts(s, kernel_all(), window);
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().econ.victory_points, 2);
}

fn pds_scenario() -> (Scenario, [Planet; 3]) {
    let sys = plain_systems();
    let ps = [sys[0].data().planets[0], sys[1].data().planets[0], sys[2].data().planets[0]];
    let sc = Scenario::lobby(3)
        .map_from(&[sys[0], sys[1], sys[2]])
        .control(ps[0], 0)
        .control(ps[1], 0)
        .control(ps[2], 1)
        .unit(sys[0], Some(ps[0]), Unit::PDSI, 0, 1)
        .unit(sys[1], Some(ps[1]), Unit::PDSII, 0, 1)
        .unit(sys[2], Some(ps[2]), Unit::PDSI, 1, 1);
    (sc, ps)
}

#[test]
fn homeland_defense_act_against_destroys_a_players_only_pds_and_asks_a_player_with_several() {
    covers!("rules::agenda::effects::Resolve");
    let (sc, ps) = pds_scenario();
    let mut rig = start(sc, Agenda::HomelandDefenseAct, Outcome::Against);
    // Seat 0 holds two PDS: it is asked first; the others are not asked.
    assert_eq!(rig.asked().actor, Seat(0));
    assert_eq!(rig.asked().ask, Ask::Agenda(AAsk::Structure));
    assert_eq!(rig.opts().len(), 2);
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    let before = rig.state.board().planet(ps[2]).units.len();
    let idx = rig.opts().iter().position(|o| matches!(o, Opt::Agenda(AOpt::Structure { planet, .. }) if *planet == ps[1])).expect("the second planet");
    rig.pick(idx);
    assert!(rig.is_done());
    assert!(rig.state.board().planet(ps[1]).units.is_empty(), "the chosen PDS is destroyed");
    assert_eq!(rig.state.board().planet(ps[0]).units.len(), 1, "the other stays");
    assert!(rig.state.board().planet(ps[2]).units.is_empty() && before == 1, "seat 1 lost its only PDS without being asked");
}

#[test]
fn homeland_defense_act_for_changes_nothing_and_a_player_without_a_pds_loses_nothing() {
    covers!("rules::agenda::effects::Resolve");
    let (sc, ps) = pds_scenario();
    let rig = start(sc, Agenda::HomelandDefenseAct, Outcome::For);
    assert!(rig.is_done());
    assert_eq!(rig.state.board().planet(ps[0]).units.len(), 1);
    let sys = plain_systems();
    let none = Scenario::lobby(2).map_from(&[sys[0]]).unit(sys[0], Some(sys[0].data().planets[0]), Unit::InfantryI, 0, 2);
    let rig = start(none, Agenda::HomelandDefenseAct, Outcome::Against);
    assert!(rig.is_done());
    assert_eq!(rig.state.board().planet(sys[0].data().planets[0]).units.len(), 2, "infantry are not PDS");
}
