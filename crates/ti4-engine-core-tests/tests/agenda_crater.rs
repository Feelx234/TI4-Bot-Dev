//! Ixthian Artifact, For (rules version 36, alpha report fb_1793193205, string-engine bug S6 in `docs/STRING_ENGINE_BUGS.md`): the speaker rolls one die. 6-10:
//! each player may research 2 technologies. 1-5: destroy all units in Mecatol Rex's system, and each player with units in the systems adjacent to it destroys 3
//! of their own units in each of those systems (their choice). The string effect destroyed every ship without Sustain Damage everywhere and deferred the 6-10.

use ti4_engine_core::choice::agenda::{Ask as AAsk, Opt as AOpt};
use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt};
use ti4_engine_core::hex::Hex;
use ti4_engine_core::query::tech::UnitRoster;
use ti4_engine_core::rules::agenda::effects::{Done, Resolve};
use ti4_engine_core::rules::agenda::table_effects::EffectFacts;
use ti4_engine_core::state::{Code, Outcome};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

const MECATOL: System = System::MecatolRex18;

fn facts() -> EffectFacts {
    EffectFacts {
        for_voters: ti4_engine_core::containers::SmallList::new(),
        against_voters: ti4_engine_core::containers::SmallList::new(),
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
        roster: Some(Box::leak(Box::new(UnitRoster::default()))),
        redistribution: Code(4),
    }
}

/// Mecatol Rex at the centre, two systems beside it (`near`) and one two steps away (`far`).
fn board() -> (Scenario, [System; 3]) {
    let plain = plain_systems();
    let (a, b, far) = (plain[0], plain[1], plain[2]);
    let sc = Scenario::lobby(4).map_hexes(&[(MECATOL, Hex::new(0, 0)), (a, Hex::new(1, 0)), (b, Hex::new(0, 1)), (far, Hex::new(2, 0))]).speaker(2);
    (sc, [a, b, far])
}

fn start(sc: Scenario, seed: u64) -> Rig<Resolve> {
    let window = Resolve::of_facts(Agenda::IxthianArtifact, Outcome::For, &facts()).expect("an effect");
    let mut rig = Rig::from_parts(sc.state(), kernel_all(), window).seeded(seed);
    rig.run();
    rig
}

/// The first seed whose die falls in the range.
fn seed_where(sc: &dyn Fn() -> Scenario, want: impl Fn(u32) -> bool) -> u64 {
    (0..200).find(|seed| want(start(sc(), *seed).driver.window.faces()[0])).expect("a seed for the range")
}

fn units(rig: &Rig<Resolve>, system: System, seat: u8) -> usize {
    let on_planets: usize = system.data().planets.iter().map(|p| rig.state.board().planet(*p).units.iter().filter(|u| u.owner() == Seat(seat)).count()).sum();
    rig.state.board().sys(system).units.iter().filter(|u| u.owner() == Seat(seat)).count() + on_planets
}

fn crater_scenario() -> Scenario {
    let (sc, [a, b, far]) = board();
    let rex = Planet::MecatolRex;
    sc
        // Mecatol Rex: a dreadnought (it sustains damage, and is destroyed all the same), a carrier, infantry and a PDS, two owners.
        .unit(MECATOL, None, Unit::DreadnoughtI, 0, 1)
        .unit(MECATOL, None, Unit::CarrierI, 1, 1)
        .unit(MECATOL, Some(rex), Unit::InfantryI, 0, 2)
        .unit(MECATOL, Some(rex), Unit::PDSI, 1, 1)
        // Beside it: seat 0 has two units (all go), seat 1 has five different units (it chooses three), seat 3 has four identical carriers (three go).
        .unit(a, None, Unit::CruiserI, 0, 1)
        .unit(a, None, Unit::FighterI, 0, 1)
        .unit(a, None, Unit::CruiserI, 1, 1)
        .unit(a, None, Unit::DestroyerI, 1, 1)
        .unit(a, None, Unit::FighterI, 1, 2)
        .unit(a, None, Unit::DreadnoughtI, 1, 1)
        .unit(b, None, Unit::CarrierI, 3, 4)
        // Two steps away: untouched.
        .unit(far, None, Unit::CarrierI, 1, 2)
}

#[test]
fn a_roll_of_one_to_five_destroys_everything_in_the_mecatol_rex_system_whoever_owns_it() {
    covers!("rules::agenda::effects::Resolve");
    let seed = seed_where(&crater_scenario, |face| face <= 5);
    let rig = start(crater_scenario(), seed);
    for seat in 0..4 {
        assert_eq!(units(&rig, MECATOL, seat), 0, "seat {seat}: nothing is left in the Mecatol Rex system, ships (the dreadnought sustains damage), ground forces and structures");
    }
    let [_, _, far] = board().1;
    assert_eq!(units(&rig, far, 1), 2, "a system two steps away is not touched: the string destroyed every ship without Sustain Damage everywhere");
}

#[test]
fn each_player_beside_mecatol_rex_destroys_three_units_of_their_choice_in_each_system() {
    covers!("rules::agenda::effects::Resolve");
    let [a, b, _] = board().1;
    let seed = seed_where(&crater_scenario, |face| face <= 5);
    let mut rig = start(crater_scenario(), seed);
    // Seat 0 holds two units in `a`: all go without a question. Seat 1 holds six pieces (five kinds at one place each): it is asked, three times.
    assert_eq!(rig.asked().actor, Seat(1), "the first player with more than three units: seat 0 lost its two without being asked");
    assert_eq!(rig.asked().ask, Ask::Agenda(AAsk::Crater { system: a, left: 3 }));
    assert_eq!(units(&rig, a, 0), 0);
    let kinds: Vec<Unit> = rig.opts().iter().map(|o| match o { Opt::Agenda(AOpt::Unit { piece, .. }) => piece.kind(), other => panic!("{other:?}") }).collect();
    assert_eq!(kinds.len(), 4, "a cruiser, a destroyer, a fighter and a dreadnought: each kind once, the two fighters are one option: {kinds:?}");
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.pick_where(|o| matches!(o, Opt::Agenda(AOpt::Unit { piece, .. }) if piece.kind() == Unit::DestroyerI));
    assert_eq!(rig.asked().ask, Ask::Agenda(AAsk::Crater { system: a, left: 2 }));
    rig.pick_where(|o| matches!(o, Opt::Agenda(AOpt::Unit { piece, .. }) if piece.kind() == Unit::FighterI));
    assert_eq!(rig.asked().ask, Ask::Agenda(AAsk::Crater { system: a, left: 1 }));
    // The destroyer is gone from the offer; one fighter is left.
    assert!(rig.opts().iter().all(|o| !matches!(o, Opt::Agenda(AOpt::Unit { piece, .. }) if piece.kind() == Unit::DestroyerI)));
    rig.pick_where(|o| matches!(o, Opt::Agenda(AOpt::Unit { piece, .. }) if piece.kind() == Unit::FighterI));
    // Seat 3 in `b` held four carriers: three went without a question (they are all alike). The player was not asked.
    assert!(rig.is_done(), "{}", rig.transcript());
    let left: Vec<Unit> = rig.state.board().sys(a).units.iter().map(|u| u.kind()).collect();
    assert_eq!(left.len(), 2);
    assert!(left.contains(&Unit::CruiserI) && left.contains(&Unit::DreadnoughtI), "the player kept what it chose to keep: {left:?}");
    assert_eq!(units(&rig, b, 3), 1, "four identical carriers: three destroyed, one left");
    assert_eq!(rig.driver.window.done(), Done::Resolved);
}

#[test]
fn a_player_with_three_units_or_fewer_in_a_system_beside_mecatol_rex_loses_them_all() {
    covers!("rules::agenda::effects::Resolve");
    let [a, ..] = board().1;
    let sc = || board().0.unit(a, None, Unit::CruiserI, 0, 1).unit(a, None, Unit::DestroyerI, 0, 1).unit(a, None, Unit::FighterI, 0, 1);
    let seed = seed_where(&sc, |face| face <= 5);
    let rig = start(sc(), seed);
    assert!(rig.is_done(), "no question: three units go");
    assert_eq!(units(&rig, a, 0), 0);
}

#[test]
fn the_ground_forces_and_structures_beside_mecatol_rex_count_as_units_too() {
    covers!("rules::agenda::effects::Resolve");
    let [a, ..] = board().1;
    let planet = a.data().planets[0];
    let sc = || board().0.control(planet, 0).unit(a, Some(planet), Unit::InfantryI, 0, 2).unit(a, Some(planet), Unit::PDSI, 0, 1).unit(a, None, Unit::CarrierI, 0, 1).unit(a, None, Unit::FighterI, 0, 1);
    let seed = seed_where(&sc, |face| face <= 5);
    let mut rig = start(sc(), seed);
    assert_eq!(rig.asked().ask, Ask::Agenda(AAsk::Crater { system: a, left: 3 }), "five units, two kinds on the planet and two in space");
    let on_planet = rig.opts().iter().filter(|o| matches!(o, Opt::Agenda(AOpt::Unit { planet: Some(_), .. }))).count();
    assert_eq!(on_planet, 2, "the infantry and the PDS stand on the planet");
    rig.pick_where(|o| matches!(o, Opt::Agenda(AOpt::Unit { piece, .. }) if piece.kind() == Unit::PDSI));
    rig.pick_where(|o| matches!(o, Opt::Agenda(AOpt::Unit { piece, .. }) if piece.kind() == Unit::CarrierI));
    rig.pick_where(|o| matches!(o, Opt::Agenda(AOpt::Unit { piece, .. }) if piece.kind() == Unit::FighterI));
    assert!(rig.is_done());
    assert_eq!(rig.state.board().planet(planet).units.len(), 2, "both infantry stay");
    assert!(rig.state.board().sys(a).units.is_empty());
}

#[test]
fn a_roll_of_six_to_ten_lets_each_player_research_two_technologies_in_speaker_order() {
    covers!("rules::agenda::effects::Resolve");
    let seed = seed_where(&|| board().0, |face| face >= 6);
    let mut rig = start(board().0, seed);
    assert_eq!(rig.driver.window.done(), Done::Resolved, "the research is the core's, no longer deferred");
    let owned = |rig: &Rig<Resolve>, seat: u8| rig.state.players().by_seat(Seat(seat)).unwrap().tech.owned.len();
    let before: Vec<usize> = (0..4).map(|s| owned(&rig, s)).collect();
    // The speaker (seat 2) is asked first, then clockwise: 3, 0, 1.
    let mut order = Vec::new();
    // Seat 2 researches one technology, then declines the second; seat 3 researches two; seats 0 and 1 decline at once.
    let script: &[(u8, Option<usize>)] = &[(2, Some(0)), (2, None), (3, Some(0)), (3, Some(0)), (0, None), (1, None)];
    for (seat, take) in script {
        assert_eq!(rig.asked().actor, Seat(*seat), "{}", rig.transcript());
        assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::ResearchTechnology { decline: true, .. })), "{:?}", rig.asked().ask);
        assert_eq!(rig.asked().ctx.origin, ti4_engine_core::choice::Origin::Agenda(Agenda::IxthianArtifact), "the agenda is the source of the question");
        order.push(rig.asked().actor.0);
        match take {
            Some(i) => {
                assert!(matches!(rig.opts()[*i], Opt::Strategy(SOpt::Research(_))));
                rig.pick(*i);
            }
            None => {
                rig.decline();
            }
        }
    }
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(order, [2, 2, 3, 3, 0, 1]);
    let after: Vec<usize> = (0..4).map(|s| owned(&rig, s)).collect();
    assert_eq!(after.iter().zip(&before).map(|(a, b)| a - b).collect::<Vec<_>>(), [0, 0, 1, 2], "seat 2 took one, seat 3 two, the others none");
}

#[test]
fn nothing_happens_on_the_other_outcome_and_the_die_is_rolled_once() {
    covers!("rules::agenda::effects::Resolve");
    let w = Resolve::of_facts(Agenda::IxthianArtifact, Outcome::Against, &facts()).expect("an effect");
    assert!(!w.rolls());
    let rig = start(board().0, 3);
    assert_eq!(rig.driver.window.faces().len(), 1);
}
