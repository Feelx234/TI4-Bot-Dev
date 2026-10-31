//! D4-2a: the agenda effects that write the table and ask nothing (`rules::agenda::table_effects`, run through `Resolve::of_facts`).

use ti4_engine_core::containers::{Deck, SmallList};
use ti4_engine_core::rules::agenda::effects::{Done, Resolve};
use ti4_engine_core::rules::agenda::table_effects::EffectFacts;
use ti4_engine_core::state::{Code, Outcome};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

fn facts(for_voters: &[u8], against_voters: &[u8]) -> EffectFacts {
    let seats = |v: &[u8]| v.iter().map(|s| Seat(*s)).collect::<SmallList<Seat, 8>>();
    EffectFacts {
        for_voters: seats(for_voters),
        against_voters: seats(against_voters),
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

fn table(n: usize) -> Scenario {
    Scenario::lobby(n)
}

fn run(sc: Scenario, agenda: Agenda, outcome: Outcome, f: &EffectFacts) -> Rig<Resolve> {
    let window = Resolve::of_facts(agenda, outcome, f).unwrap_or_else(|| panic!("an effect for {agenda:?} {outcome:?}"));
    let mut rig = Rig::from_parts(sc.state(), kernel_all(), window);
    rig.run();
    rig
}

fn tokens(rig: &Rig<Resolve>, seat: u8) -> (i16, i16, i16) {
    let e = rig.state.players().by_seat(Seat(seat)).unwrap().econ;
    (e.tactic_tokens, e.fleet_tokens, e.strategic_tokens)
}

fn goods(rig: &Rig<Resolve>, seat: u8) -> i16 {
    rig.state.players().by_seat(Seat(seat)).unwrap().econ.trade_goods
}

fn hand(rig: &Rig<Resolve>, seat: u8) -> usize {
    rig.state.players().by_seat(Seat(seat)).unwrap().cards.hand.as_slice().len()
}

const CARDS: [ActionCard; 2] = [ActionCard::Sabotage1, ActionCard::Spy];

#[test]
fn armed_forces_standardization_tops_the_elected_seat_up_and_never_down() {
    covers!("rules::agenda::table_effects::Table");
    let sc = table(3).tokens(1, 1, 1, 0).tokens(2, 5, 0, 2);
    let rig = run(sc.clone(), Agenda::ArmedForcesStandardization, Outcome::Seat(Seat(1)), &facts(&[], &[]));
    assert!(rig.is_done());
    assert_eq!(tokens(&rig, 1), (3, 3, 2));
    let high = run(sc, Agenda::ArmedForcesStandardization, Outcome::Seat(Seat(2)), &facts(&[], &[]));
    assert_eq!(tokens(&high, 2), (5, 3, 2), "a pool above the target stays");
}

#[test]
fn clandestine_operations_takes_two_tokens_from_the_fullest_pools_or_one_fleet_token() {
    covers!("rules::agenda::table_effects::Table");
    let sc = table(2).tokens(0, 3, 3, 2).tokens(1, 0, 0, 1);
    let both = run(sc.clone(), Agenda::ClandestineOperations, Outcome::For, &facts(&[], &[]));
    assert_eq!(tokens(&both, 0), (2, 2, 2), "the last of the fullest pools first: fleet, then tactic");
    assert_eq!(tokens(&both, 1), (0, 0, 0), "one strategy token, then nothing to take");
    let one = run(sc, Agenda::ClandestineOperations, Outcome::Against, &facts(&[], &[]));
    assert_eq!(tokens(&one, 0), (3, 2, 2));
    assert_eq!(tokens(&one, 1), (0, 0, 1), "no fleet token to spend");
}

#[test]
fn fleet_regulations_against_gives_each_seat_with_a_token_in_reinforcements_a_fleet_token() {
    covers!("rules::agenda::table_effects::Table");
    let sc = table(2).tokens(0, 3, 3, 2).tokens(1, 3, 3, 10);
    let rig = run(sc, Agenda::FleetRegulations, Outcome::Against, &facts(&[], &[]));
    assert_eq!(tokens(&rig, 0), (3, 4, 2));
    assert_eq!(tokens(&rig, 1), (3, 3, 10), "sixteen tokens already on the sheet: none left in the box");
}

#[test]
fn economic_equality_zeroes_every_pile_and_for_pays_five_each() {
    covers!("rules::agenda::table_effects::Table");
    let sc = table(3).goods(0, 7).goods(1, 2);
    let against = run(sc.clone(), Agenda::EconomicEquality, Outcome::Against, &facts(&[], &[]));
    assert_eq!((goods(&against, 0), goods(&against, 1), goods(&against, 2)), (0, 0, 0));
    let for_ = run(sc, Agenda::EconomicEquality, Outcome::For, &facts(&[], &[]));
    assert_eq!((goods(&for_, 0), goods(&for_, 1), goods(&for_, 2)), (5, 5, 5));
    assert!(!for_.state.pending().staged.is_empty(), "the gains are staged for the announcement");
}

#[test]
fn articles_of_war_against_pays_three_goods_to_each_for_voter() {
    covers!("rules::agenda::table_effects::Table");
    let rig = run(table(3), Agenda::ArticlesOfWar, Outcome::Against, &facts(&[0, 2], &[1]));
    assert_eq!((goods(&rig, 0), goods(&rig, 1), goods(&rig, 2)), (3, 0, 3));
    let for_ = run(table(3), Agenda::ArticlesOfWar, Outcome::For, &facts(&[0, 2], &[1]));
    assert_eq!((goods(&for_, 0), goods(&for_, 2)), (0, 0), "For does nothing");
}

#[test]
fn public_execution_empties_the_hand_of_the_elected_seat_and_passes_the_speaker_token_on() {
    covers!("rules::agenda::table_effects::Table");
    let sc = table(3).speaker(1).hand(1, &CARDS).hand(0, &CARDS);
    let window = Resolve::of_facts(Agenda::PublicExecution, Outcome::Seat(Seat(1)), &facts(&[], &[])).expect("an effect");
    assert_eq!(window.done(), Done::Deferred, "the string defers what follows");
    let rig = run(sc.clone(), Agenda::PublicExecution, Outcome::Seat(Seat(1)), &facts(&[], &[]));
    assert_eq!((hand(&rig, 1), hand(&rig, 0)), (0, 2));
    assert_eq!(rig.state.meta().speaker, Seat(2), "the token goes to the next seat clockwise");
    let not_speaker = run(sc, Agenda::PublicExecution, Outcome::Seat(Seat(0)), &facts(&[], &[]));
    assert_eq!(not_speaker.state.meta().speaker, Seat(1), "only the speaker passes the token");
}

#[test]
fn the_voters_hands_go_for_conventions_of_war_against_and_unconventional_measures_against() {
    covers!("rules::agenda::table_effects::Table");
    let sc = table(3).hand(0, &CARDS).hand(1, &CARDS).hand(2, &CARDS);
    let conventions = run(sc.clone(), Agenda::ConventionsOfWar, Outcome::Against, &facts(&[0], &[1]));
    assert_eq!((hand(&conventions, 0), hand(&conventions, 1), hand(&conventions, 2)), (2, 0, 2), "the Against voters");
    let measures = run(sc.clone(), Agenda::UnconventionalMeasures, Outcome::Against, &facts(&[0, 2], &[1]));
    assert_eq!((hand(&measures, 0), hand(&measures, 1), hand(&measures, 2)), (0, 2, 0), "the For voters");
    let for_ = run(sc, Agenda::ConventionsOfWar, Outcome::For, &facts(&[0], &[1]));
    assert_eq!((hand(&for_, 0), hand(&for_, 1)), (2, 2));
}

#[test]
fn unconventional_measures_for_draws_two_cards_for_each_for_voter() {
    covers!("rules::agenda::table_effects::Table");
    let mut sc = table(3).state();
    sc.decks_mut().action = Deck::new(vec![ActionCard::Spy, ActionCard::Sabotage1, ActionCard::Sabotage2, ActionCard::Sabotage3, ActionCard::Sabotage4]);
    let window = Resolve::of_facts(Agenda::UnconventionalMeasures, Outcome::For, &facts(&[0, 1], &[2])).expect("an effect");
    assert!(window.draws() && !window.rolls());
    let mut rig = Rig::from_parts(sc, kernel_all(), window);
    rig.run();
    assert_eq!((hand(&rig, 0), hand(&rig, 1), hand(&rig, 2)), (2, 2, 0));
    assert_eq!(rig.state.decks().action.cards().len(), 1);
    assert!(!rig.driver.window.unsupported());
}

#[test]
fn publicize_weapon_schematics_against_empties_the_hands_of_the_war_sun_owners() {
    covers!("rules::agenda::table_effects::Table");
    let sc = table(3).hand(0, &CARDS).hand(1, &CARDS).tech(0, Technology::WarSun).tech(1, Technology::PrototypeWarSunII);
    let rig = run(sc.hand(2, &CARDS), Agenda::PublicizeWeaponSchematics, Outcome::Against, &facts(&[], &[]));
    assert_eq!((hand(&rig, 0), hand(&rig, 1), hand(&rig, 2)), (0, 0, 2));
}

#[test]
fn shared_research_against_places_a_command_token_in_every_home_system() {
    covers!("rules::agenda::table_effects::Table");
    let rig = run(table(3), Agenda::SharedResearch, Outcome::Against, &facts(&[], &[]));
    for seat in 0..3u8 {
        let p = rig.state.players().by_seat(Seat(seat)).unwrap();
        let home = p.home_system.or(p.faction.data().home_system).expect("a home system");
        assert!(rig.state.board().sys(home).cmd.contains(Seat(seat)), "seat {seat}");
    }
    assert_eq!(rig.state.pending().staged.len(), 3, "one placement announced per seat");
}

#[test]
fn arms_reduction_for_keeps_two_dreadnoughts_and_four_cruisers_of_each_seat() {
    covers!("rules::agenda::table_effects::Table");
    let (a, b) = (plain_systems()[0], plain_systems()[1]);
    let sc = table(2)
        .map_from(&[a, b])
        .unit(a, None, Unit::DreadnoughtI, 0, 3)
        .unit(b, None, Unit::DreadnoughtI, 0, 1)
        .unit(a, None, Unit::CruiserI, 0, 6)
        .unit(a, None, Unit::DreadnoughtI, 1, 2)
        .unit(b, None, Unit::CarrierI, 0, 5);
    let rig = run(sc, Agenda::ArmsReduction, Outcome::For, &facts(&[], &[]));
    let count = |seat: u8, unit: Unit| -> usize {
        System::ALL.iter().map(|s| rig.state.board().sys(*s).units.as_slice().iter().filter(|p| p.kind() == unit && p.owner() == Seat(seat)).count()).sum()
    };
    assert_eq!((count(0, Unit::DreadnoughtI), count(0, Unit::CruiserI), count(0, Unit::CarrierI)), (2, 4, 5), "carriers are not reduced");
    assert_eq!(count(1, Unit::DreadnoughtI), 2, "two is within the limit");
}

#[test]
fn incentive_program_turns_up_the_first_facedown_objective_of_the_stage() {
    covers!("rules::agenda::table_effects::Table");
    use ti4_engine_core::query::objectives::stage_of;
    let ones: Vec<PublicObjective> = PublicObjective::ALL.into_iter().filter(|o| stage_of(*o) == Some(1)).take(2).collect();
    let twos: Vec<PublicObjective> = PublicObjective::ALL.into_iter().filter(|o| stage_of(*o) == Some(2)).take(1).collect();
    let mut s = table(2).state();
    s.decks_mut().objective = Deck::new(vec![twos[0], ones[0], ones[1]]);
    let window = Resolve::of_facts(Agenda::IncentiveProgram, Outcome::For, &facts(&[], &[])).expect("an effect");
    let mut rig = Rig::from_parts(s.clone(), kernel_all(), window);
    rig.run();
    assert_eq!(rig.state.objectives().revealed.len(), 1);
    assert_eq!(rig.state.decks().objective.cards(), &[twos[0], ones[1]]);
    let window = Resolve::of_facts(Agenda::IncentiveProgram, Outcome::Against, &facts(&[], &[])).expect("an effect");
    let mut two = Rig::from_parts(s, kernel_all(), window);
    two.run();
    assert_eq!(two.state.decks().objective.cards(), &[ones[0], ones[1]], "Against reveals a stage-two objective");
}

#[test]
fn compensated_disarmament_destroys_the_ground_forces_and_pays_their_controller_a_good_each() {
    covers!("rules::agenda::table_effects::Table");
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let sc = table(2).map_from(&[system]).control(planet, 0).unit(system, Some(planet), Unit::InfantryI, 0, 2).unit(system, Some(planet), Unit::InfantryI, 1, 1);
    let rig = run(sc, Agenda::CompensatedDisarmament, Outcome::Planet(planet), &facts(&[], &[]));
    assert!(rig.is_done() && !rig.driver.window.unsupported());
    assert!(rig.state.board().planet(planet).units.as_slice().is_empty(), "every ground force of every seat on the planet");
    assert_eq!(goods(&rig, 0), 3, "the controller gets one trade good for each unit destroyed");
    let _ = system_of(planet);
}
