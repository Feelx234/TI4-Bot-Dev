//! Lent-bodies lane 9: `rules::invasion::fight::CommittedFight` (the ground combats of one committed planet, to their end) and
//! `rules::invasion::yin_hero::YinHero` (Quantum Dissemination).

use ti4_engine_core::rules::invasion::fight::{CommittedFight, FightFacts};
use ti4_engine_core::rules::invasion::roll::NAMED;
use ti4_engine_core::rules::invasion::yin_hero::{YinFacts, YinHero};
use ti4_engine_core::state::{Code, EventKind, Feat};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::first_planet;

fn facts(system: System, planet: Planet) -> FightFacts {
    FightFacts {
        invader: Seat(0),
        system,
        planet,
        order: vec![Seat(0), Seat(1), Seat(2)],
        announce: true,
        copied: false,
        numbered: true,
        can_pay: [false, false],
        pay_env: None,
        dunlain: Code(1),
        kind: Code(2),
        names: (0..NAMED).map(|i| Code(10 + i as u16)).collect(),
        unit_codes: vec![(Unit::InfantryI, Code(3)), (Unit::Eidolon, Code(4))],
        cause: Code(5),
        harrow_cause: Code(6),
        step_kind: Code(7),
        harrow: false,
        x89: vec![],
        dacxive: vec![],
        notes: vec![],
    }
}

fn arena(mine: usize, theirs: usize) -> (Scenario, System, Planet) {
    let (system, planet) = first_planet();
    let sc = Scenario::new(&[Faction::TheYinBrotherhood, Faction::TheArborec, Faction::TheEmbersOfMuaat])
        .map_from(&[system])
        .unit(system, Some(planet), Unit::InfantryI, 0, mine)
        .unit(system, Some(planet), Unit::InfantryI, 1, theirs);
    (sc, system, planet)
}

fn fight(mine: usize, theirs: usize, seed: u64) -> Rig<CommittedFight> {
    let (sc, system, planet) = arena(mine, theirs);
    Rig::new(sc, CommittedFight::new(facts(system, planet))).seeded(seed)
}

fn left(rig: &Rig<impl Window>, planet: Planet, seat: u8) -> usize {
    rig.state.board().planet(planet).units.as_slice().iter().filter(|p| p.owner() == Seat(seat)).count()
}

#[test]
fn a_planet_is_fought_to_the_end_with_the_events_of_the_combat() {
    covers!("rules::invasion::fight::CommittedFight");
    let (_, planet) = first_planet();
    for seed in 0..6 {
        let mut rig = fight(3, 3, seed);
        rig.run();
        while !rig.is_done() {
            rig.pick(0);
        }
        assert!(!rig.driver.window.unsupported, "{}", rig.transcript());
        assert!(left(&rig, planet, 0) == 0 || left(&rig, planet, 1) == 0, "seed {seed}: one side is gone");
        let count = |kind: EventKind| rig.driver.emitted.iter().filter(|e| e.kind == kind).count();
        assert_eq!((count(EventKind::GroundCombatStarted), count(EventKind::GroundCombatEnded)), (1, 1), "seed {seed}");
        assert_eq!(rig.driver.window.fought, vec![Seat(1)]);
    }
}

#[test]
fn a_win_earns_the_feats_against_the_leader_and_a_note_holder_against_one_occurrence() {
    covers!("rules::invasion::fight::CommittedFight");
    let (sc, system, planet) = arena(5, 1);
    let mut f = facts(system, planet);
    f.notes = vec![(Seat(0), Seat(1))];
    // Everyone has 0 points: the loser ties for the most.
    let mut rig = Rig::new(sc, CommittedFight::new(f)).seeded(1);
    rig.run();
    while !rig.is_done() {
        rig.pick(0);
    }
    let seq = rig.state.meta().feat_occurrence_seq;
    let feats = rig.state.players().by_seat(Seat(0)).unwrap().misc.event_feats.as_slice().to_vec();
    assert!(feats.contains(&(Feat::WonAgainstThePointsLeader, seq)) && feats.contains(&(Feat::WonAgainstANoteHolder, seq)), "{feats:?} at {seq}");
    assert_eq!(rig.driver.window.scoring, vec![seq], "the string opens a scoring window for the occurrence");
    assert_eq!(seq, 2, "the window the fight lives in begins one occurrence, the combat a second");
}

#[test]
fn every_rival_is_fought_in_the_order_of_the_player_ids_while_the_invader_stands() {
    covers!("rules::invasion::fight::CommittedFight");
    let (sc, system, planet) = arena(8, 1);
    let sc = sc.unit(system, Some(planet), Unit::InfantryI, 2, 1);
    let mut f = facts(system, planet);
    f.order = vec![Seat(0), Seat(2), Seat(1)];
    let mut rig = Rig::new(sc, CommittedFight::new(f)).seeded(4);
    rig.run();
    while !rig.is_done() {
        rig.pick(0);
    }
    assert_eq!(rig.driver.window.fought, vec![Seat(2), Seat(1)], "the seat that comes first in the order is fought first");
}

#[test]
fn nothing_is_fought_without_a_rival_or_without_the_invader() {
    covers!("rules::invasion::fight::CommittedFight");
    for (mine, theirs) in [(3, 0), (0, 3)] {
        let mut rig = fight(mine, theirs, 1);
        rig.run();
        assert!(rig.is_done() && rig.driver.window.fought.is_empty());
        assert_eq!(rig.state.meta().feat_occurrence_seq, 1, "only the window's own occurrence");
    }
}

fn yin(mine_there: usize, theirs: usize, stock: u8, seed: u64) -> (Rig<YinHero>, System, Planet) {
    let (sc, system, planet) = arena(mine_there, theirs);
    let facts = YinFacts { spots: vec![(system, planet)], stock, infantry: Unit::InfantryI, fight: facts(system, planet), minister: Code(20) };
    (Rig::new(sc, YinHero::new(Seat(0), facts)).seeded(seed), system, planet)
}

#[test]
fn infantry_is_committed_one_question_at_a_time_landed_fought_for_and_the_planet_taken() {
    covers!("rules::invasion::yin_hero::YinHero");
    let (mut rig, system, planet) = yin(0, 0, 2, 3);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Leaders(ti4_engine_core::choice::leaders::Ask::YinCommit { n: 0, stock: 2 })));
    assert_eq!(rig.opts().last(), Some(&Opt::Decline));
    rig.assert_options_sound();
    let spot = Opt::Leaders(ti4_engine_core::choice::leaders::Opt::YinSpot(system, planet));
    rig.pick_opt(spot);
    assert!(matches!(rig.asked().ask, Ask::Leaders(ti4_engine_core::choice::leaders::Ask::YinCommit { n: 1, stock: 2 })));
    rig.pick_opt(spot);
    assert!(rig.is_done() && rig.driver.window.used, "{}", rig.transcript());
    assert_eq!(left(&rig, planet, 0), 2, "both landed");
    assert_eq!(rig.state.board().planet(planet).control, Some(Seat(0)), "nobody stood in the way: the planet is taken");
    assert!(rig.state.board().planet(planet).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED), "taken exhausted");
}

#[test]
fn a_decline_stops_the_asking_and_declining_at_once_changes_nothing() {
    covers!("rules::invasion::yin_hero::YinHero");
    let (mut rig, system, planet) = yin(0, 0, 3, 1);
    rig.run();
    rig.pick_opt(Opt::Leaders(ti4_engine_core::choice::leaders::Opt::YinSpot(system, planet)));
    rig.decline();
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(left(&rig, planet, 0), 1);
    let (mut rig, _, _) = yin(0, 0, 3, 1);
    rig.run();
    rig.decline();
    assert!(rig.is_done() && !rig.driver.window.used);
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

#[test]
fn no_infantry_or_no_spot_means_the_card_is_not_used() {
    covers!("rules::invasion::yin_hero::YinHero");
    let (mut rig, _, _) = yin(0, 0, 0, 1);
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.used);
    let (sc, system, planet) = arena(0, 0);
    let facts = YinFacts { spots: vec![], stock: 3, infantry: Unit::InfantryI, fight: facts(system, planet), minister: Code(20) };
    let mut rig = Rig::new(sc, YinHero::new(Seat(0), facts));
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.used);
}

#[test]
fn a_defended_planet_is_fought_and_taken_only_by_the_side_that_stands() {
    covers!("rules::invasion::yin_hero::YinHero");
    let mut taken = (0, 0);
    for seed in 0..8 {
        let (mut rig, system, planet) = yin(0, 2, 3, seed);
        rig.run();
        let spot = Opt::Leaders(ti4_engine_core::choice::leaders::Opt::YinSpot(system, planet));
        rig.pick_opt(spot);
        rig.pick_opt(spot);
        rig.pick_opt(spot);
        while !rig.is_done() {
            rig.pick(0);
        }
        assert!(rig.driver.window.used && !rig.driver.window.unsupported, "{}", rig.transcript());
        let (mine, theirs) = (left(&rig, planet, 0), left(&rig, planet, 1));
        assert!(mine == 0 || theirs == 0, "seed {seed}");
        let held = rig.state.board().planet(planet).control == Some(Seat(0));
        assert_eq!(held, mine > 0, "seed {seed}: control follows the ground force that stands");
        if held { taken.0 += 1 } else { taken.1 += 1 }
    }
    assert!(taken.0 > 0, "some seed is won by the hero");
}

#[test]
fn the_end_of_a_combat_names_the_winner_the_rival_and_whether_control_changed() {
    covers!("rules::invasion::fight::CommittedFight");
    let (_, planet) = first_planet();
    let (mut won, mut lost) = (0, 0);
    for seed in 0..12 {
        let (sc, system, planet2) = arena(2, 2);
        assert_eq!(planet, planet2);
        let mut rig = Rig::new(sc, CommittedFight::new(facts(system, planet))).seeded(seed);
        rig.run();
        while !rig.is_done() {
            rig.pick(0);
        }
        let ended = rig.driver.emitted.iter().find(|e| e.kind == EventKind::GroundCombatEnded).expect("the end is announced");
        let get = |key: ti4_engine_core::state::Key| ended.payload.get(key);
        let mine = left(&rig, planet, 0);
        assert_eq!(get(ti4_engine_core::state::Key::Defender), Some(ti4_engine_core::state::Val::Seat(Seat(1))), "the rival is named whoever wins");
        let winner = get(ti4_engine_core::state::Key::Winner);
        if mine > 0 {
            won += 1;
            assert_eq!(winner, Some(ti4_engine_core::state::Val::Seat(Seat(0))));
            assert_eq!(get(ti4_engine_core::state::Key::ControlChanged), Some(ti4_engine_core::state::Val::Flag(true)), "the invader took an uncontrolled planet");
        } else {
            lost += 1;
            assert_eq!(get(ti4_engine_core::state::Key::ControlChanged), Some(ti4_engine_core::state::Val::Flag(false)));
            if left(&rig, planet, 1) > 0 {
                assert_eq!(winner, Some(ti4_engine_core::state::Val::Seat(Seat(1))));
            } else {
                assert_eq!(winner, None, "nobody stands: no winner is named");
            }
        }
    }
    assert!(won > 0 && lost > 0, "the seeds give both outcomes ({won}, {lost})");
}

#[test]
fn a_spot_that_is_not_offered_is_refused_and_a_hero_that_loses_does_not_take_the_planet() {
    covers!("rules::invasion::yin_hero::YinHero");
    let (mut rig, _, _) = yin(0, 0, 3, 1);
    rig.run();
    rig.expect_illegal(Pick::Opt(Opt::Leaders(ti4_engine_core::choice::leaders::Opt::YinSpot(there(), Planet::ALL[0]))));
    let mut lost = 0;
    for seed in 0..10 {
        let (mut rig, system, planet) = yin(0, 6, 1, seed);
        rig.run();
        rig.pick_opt(Opt::Leaders(ti4_engine_core::choice::leaders::Opt::YinSpot(system, planet)));
        while !rig.is_done() {
            rig.pick(0);
        }
        if left(&rig, planet, 0) == 0 {
            lost += 1;
            assert_eq!(rig.state.board().planet(planet).control, None, "seed {seed}: a lost planet is not taken");
            assert!(rig.driver.window.used, "the card is spent all the same");
        }
    }
    assert!(lost > 0);
}

fn there() -> System {
    ti4_engine_core_tests::scenario::plain_systems()[5]
}

// -- Package W13: the effects the committed fight and the hero used to leave to the string ---------------------

fn run_out(rig: &mut Rig<CommittedFight>) {
    rig.run();
    while !rig.is_done() {
        rig.pick(0);
    }
}

/// Dacxive Animators: after a ground combat the holder won, 1 infantry from the reinforcements stands on the planet; nobody wins a double
/// wipe-out, and a seat without the technology gets nothing.
#[test]
fn dacxive_animators_places_an_infantry_for_the_winner_that_holds_it() {
    covers!("rules::invasion::fight::CommittedFight");
    let (_, planet) = first_planet();
    let mut placed = 0;
    for seed in 0..10 {
        let (sc, system, planet2) = arena(3, 1);
        let mut f = facts(system, planet2);
        f.dacxive = vec![(Seat(0), Unit::InfantryI), (Seat(1), Unit::InfantryI)];
        let mut with = Rig::new(sc, CommittedFight::new(f)).seeded(seed);
        run_out(&mut with);
        let (sc, system, planet2) = arena(3, 1);
        let mut without = Rig::new(sc, CommittedFight::new(facts(system, planet2))).seeded(seed);
        run_out(&mut without);
        let (a, b) = (left(&with, planet, 0) + left(&with, planet, 1), left(&without, planet, 0) + left(&without, planet, 1));
        assert!(a == b || a == b + 1, "seed {seed}: at most the one infantry ({a} against {b})");
        if left(&without, planet, 0) + left(&without, planet, 1) > 0 {
            assert_eq!(a, b + 1, "seed {seed}: a combat with a winner places the infantry");
            placed += 1;
        } else {
            assert_eq!(a, b, "seed {seed}: a double wipe-out has no winner");
        }
    }
    assert!(placed > 0);
    // Only the holder: the other seat wins and gets nothing.
    let (sc, system, planet2) = arena(1, 6);
    let mut f = facts(system, planet2);
    f.dacxive = vec![(Seat(0), Unit::InfantryI)];
    let mut rig = Rig::new(sc, CommittedFight::new(f)).seeded(2);
    run_out(&mut rig);
    assert_eq!(left(&rig, planet, 0), 0);
}

/// X-89 Bacterial Weapon: the hits of the holder's ground combat roll are doubled.
#[test]
fn x89_doubles_the_hits_of_the_holder_in_a_committed_fight() {
    covers!("rules::invasion::fight::CommittedFight");
    let (_, planet) = first_planet();
    let mut doubled = 0;
    for seed in 0..12 {
        let (sc, system, planet2) = arena(6, 6);
        let mut f = facts(system, planet2);
        f.x89 = vec![Seat(0)];
        let mut with = Rig::new(sc, CommittedFight::new(f)).seeded(seed);
        run_out(&mut with);
        let (sc, system, planet2) = arena(6, 6);
        let mut without = Rig::new(sc, CommittedFight::new(facts(system, planet2))).seeded(seed);
        run_out(&mut without);
        if left(&with, planet, 1) != left(&without, planet, 1) || left(&with, planet, 0) != left(&without, planet, 0) {
            doubled += 1;
        }
        assert!(left(&with, planet, 0) == 0 || left(&with, planet, 1) == 0);
    }
    assert!(doubled > 0, "the doubled hits change some fight");
}

/// The Minister of Exploration pays a trade good for each planet the hero takes, and the Crown of Emphidia changes hands when the planet
/// is in its owner's home system.
#[test]
fn the_control_gained_pays_the_minister_of_exploration_and_moves_the_crown_of_emphidia() {
    covers!("rules::invasion::yin_hero::YinHero");
    use ti4_engine_core::generated::Agenda;
    use ti4_engine_core::keys::Outcome;
    let take = |rig: &mut Rig<YinHero>, system: System, planet: Planet| {
        rig.run();
        let spot = Opt::Leaders(ti4_engine_core::choice::leaders::Opt::YinSpot(system, planet));
        rig.pick_opt(spot);
        rig.decline();
        assert!(rig.is_done() && rig.driver.window.used, "{}", rig.transcript());
    };
    let (mut rig, system, planet) = yin(0, 0, 2, 3);
    rig.state.agenda_mut().laws.insert(Agenda::MinisterOfExploration, Outcome::Seat(Seat(0)));
    let goods = rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods;
    take(&mut rig, system, planet);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, goods + 1, "a trade good for the planet taken");
    // Held by another seat: nothing.
    let (mut rig, system, planet) = yin(0, 0, 2, 3);
    rig.state.agenda_mut().laws.insert(Agenda::MinisterOfExploration, Outcome::Seat(Seat(1)));
    let goods = rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods;
    take(&mut rig, system, planet);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, goods);
    // The Crown of Emphidia: its owner's home system is the one taken.
    let (mut rig, system, planet) = yin(0, 0, 2, 3);
    rig.state.players_mut().by_seat_mut(Seat(1)).unwrap().home_system = Some(system);
    rig.state.agenda_mut().laws.insert(Agenda::TheCrownOfEmphidia, Outcome::Seat(Seat(1)));
    let (mine, theirs) = (rig.state.players().by_seat(Seat(0)).unwrap().econ.victory_points, rig.state.players().by_seat(Seat(1)).unwrap().econ.victory_points);
    take(&mut rig, system, planet);
    assert_eq!(rig.state.agenda().laws.get(&Agenda::TheCrownOfEmphidia), Some(&Outcome::Seat(Seat(0))), "the card changed hands");
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.victory_points, mine + 1);
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().econ.victory_points, theirs - 1);
    // Another system: it stays.
    let (mut rig, system, planet) = yin(0, 0, 2, 3);
    rig.state.agenda_mut().laws.insert(Agenda::TheCrownOfEmphidia, Outcome::Seat(Seat(1)));
    take(&mut rig, system, planet);
    assert_eq!(rig.state.agenda().laws.get(&Agenda::TheCrownOfEmphidia), Some(&Outcome::Seat(Seat(1))));
}
