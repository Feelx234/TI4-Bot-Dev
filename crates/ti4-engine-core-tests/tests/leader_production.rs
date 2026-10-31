//! W10 (lane 7): `rules::leader_production`, the leaders that produce as the child of their own window: Letani Miasmiala (Arborec hero)
//! and Umbat (Muaat agent). The child is a production by an ability (`Spawn::Produce`, `ProductionWindow::ability`).

use ti4_engine_core::choice::leaders::{Ask as LAsk, Opt as LOpt};
use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt, Spot};
use ti4_engine_core::generated::{Leader, Planet};
use ti4_engine_core::kernel::children::ProduceFacts;
use ti4_engine_core::rules::leader_production::{ArborecHero, Umbat};
use ti4_engine_core::state::LeaderStatus;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

/// Two planets in two different systems, in system order.
fn two() -> [(System, Planet); 2] {
    let mut found: Vec<(System, Planet)> = Vec::new();
    for s in plain_systems() {
        if let Some(p) = s.data().planets.first()
            && s.data().planets.len() == 1
        {
            found.push((s, *p));
        }
        if found.len() == 2 {
            break;
        }
    }
    found.sort_unstable();
    [found[0], found[1]]
}

fn facts(who: u8, system: System, limit: i32, cap: Option<i32>) -> ProduceFacts {
    let mut buildable = [None; 9];
    buildable[0] = Some(Unit::FighterI);
    buildable[1] = Some(Unit::InfantryI);
    buildable[2] = Some(Unit::CarrierI);
    ProduceFacts { who: Seat(who), system, limit, max_unit_cost: cap, small_exemptions: 0, discount: 0, fleet_limit: 8, armada: 0, flagship_free: false, buildable }
}

fn hero_table(goods: i16) -> (Scenario, [(System, Planet); 2]) {
    let [(a, pa), (b, pb)] = two();
    let sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat])
        .map_from(&[a, b])
        .control(pa, 0)
        .control(pb, 0)
        .unit(a, Some(pa), Unit::InfantryI, 0, 1)
        .unit(b, Some(pb), Unit::InfantryI, 0, 1)
        .goods(0, goods)
        .tokens(0, 3, 3, 2);
    (sc, [(a, pa), (b, pb)])
}

fn hero(sc: Scenario, systems: &[System]) -> Rig<ArborecHero> {
    Rig::new(sc, ArborecHero::new(Seat(0), systems.iter().map(|s| facts(0, *s, 1000, None)).collect()))
}

fn build_infantry_pay_with_goods(rig: &mut Rig<ArborecHero>) {
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::InfantryI, .. })));
    if matches!(rig.asked().ask, Ask::Production(PAsk::Pay(_))) {
        rig.pick_where(|o| matches!(o, Opt::Production(POpt::TradeGood { .. })));
    }
}

#[test]
fn the_hero_opens_a_use_in_each_system_with_a_ground_force_in_system_order() {
    covers!("rules::leader_production::ArborecHero");
    let (sc, [(a, pa), (b, pb)]) = hero_table(6);
    let mut rig = hero(sc, &[a, b]);
    rig.run();
    let Ask::Production(PAsk::Choose(first)) = &rig.asked().ask else { panic!("the production question: {:?}", rig.asked().ask) };
    assert_eq!((first.system, first.limit, first.ability, rig.asked().actor), (a, 1000, true, Seat(0)), "the first system, by an ability, with no real limit");
    rig.assert_options_sound();
    build_infantry_pay_with_goods(&mut rig);
    // The same system is offered again until the seat stops; then the next system's use opens.
    rig.pick_opt(Opt::Production(POpt::Stop));
    let Ask::Production(PAsk::Choose(second)) = &rig.asked().ask else { panic!("the second use: {:?}", rig.asked().ask) };
    assert_eq!(second.system, b);
    build_infantry_pay_with_goods(&mut rig);
    rig.pick_opt(Opt::Production(POpt::Stop));
    assert!(rig.is_done(), "{}", rig.transcript());
    assert!(rig.driver.window.used());
    let made: Vec<(System, Vec<(Unit, Option<Planet>)>)> = rig.driver.window.made.iter().map(|(s, m)| (*s, m.as_slice().to_vec())).collect();
    assert_eq!(made, vec![(a, vec![(Unit::InfantryI, Some(pa)), (Unit::InfantryI, Some(pa))]), (b, vec![(Unit::InfantryI, Some(pb)), (Unit::InfantryI, Some(pb))])], "two infantry for each resource, on the planet");
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 4, "one good for each use");
}

#[test]
fn the_hero_passes_over_a_system_where_nothing_can_be_built_and_a_seat_without_ground_forces_is_not_used() {
    covers!("rules::leader_production::ArborecHero");
    // No purse: no unit is affordable, so no question and nothing made.
    let (sc, [(a, _), (b, _)]) = hero_table(0);
    let mut broke = hero(sc, &[a, b]);
    broke.state.board_mut().planet_mut(two()[0].1).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true);
    broke.state.board_mut().planet_mut(two()[1].1).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true);
    broke.run();
    assert!(broke.is_done());
    assert!(!broke.driver.window.used());
    // The second seat has no ground force: the hero of seat 1 finds no system.
    let (sc, [(a, _), _]) = hero_table(6);
    let mut none = Rig::new(sc, ArborecHero::new(Seat(1), vec![facts(1, a, 1000, None)]));
    none.run();
    assert!(none.is_done());
    assert!(!none.driver.window.used());
    assert_eq!(delta(none.start_state(), &none.state), "", "nothing was touched");
}

#[test]
fn the_systems_of_the_hero_are_those_with_a_ground_force_in_space_or_on_a_planet_once_each_in_system_order() {
    covers!("rules::leader_production::ArborecHero");
    let [(a, pa), (b, pb)] = two();
    let (sc, _) = hero_table(6);
    let (mut state, _) = sc.build();
    // A second planet of a system that already has one with a ground force does not list it twice.
    let planets = a.data().planets;
    for p in planets {
        ti4_engine_core::kernel::fx::place_unit(&mut state, Seat(0), a, Some(*p), Unit::InfantryI);
    }
    // A ground force in the space area of the later system, one on a planet of the earlier: system order, not the order of finding.
    ti4_engine_core::kernel::fx::place_unit(&mut state, Seat(0), b, None, Unit::InfantryI);
    assert!(a < b);
    assert_eq!(ArborecHero::ground_force_systems(&state, Seat(0)), vec![a, b]);
    assert_eq!(ArborecHero::ground_force_systems(&state, Seat(1)), Vec::<System>::new(), "the other seat has none");
    // A ship is no ground force.
    let (sc, _) = hero_table(6);
    let (mut state, _) = sc.build();
    state.board_mut().sys_mut(a).units.clear();
    for planet in [pa, pb] {
        state.board_mut().planet_mut(planet).units.clear();
    }
    ti4_engine_core::kernel::fx::place_unit(&mut state, Seat(0), a, None, Unit::CarrierI);
    assert_eq!(ArborecHero::ground_force_systems(&state, Seat(0)), Vec::<System>::new());
}

#[test]
fn the_hero_is_unsupported_for_a_system_the_adapter_gave_no_facts_for() {
    covers!("rules::leader_production::ArborecHero");
    let (sc, [(a, _), _]) = hero_table(6);
    let mut rig = hero(sc, &[a]);
    rig.run();
    // The first system has facts and asks; stopping it leaves the second system, which has none.
    rig.pick_opt(Opt::Production(POpt::Stop));
    assert!(rig.is_done());
    assert!(rig.driver.window.unsupported, "no facts for the second system");
}

#[test]
fn the_hero_places_a_ship_in_space_and_ground_forces_on_a_planet_it_controls() {
    covers!("rules::leader_production::ArborecHero");
    let (sc, [(a, pa), (b, _)]) = hero_table(6);
    let mut rig = hero(sc, &[a, b]);
    rig.run();
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::FighterI, .. })));
    if matches!(rig.asked().ask, Ask::Production(PAsk::Pay(_))) {
        rig.pick_where(|o| matches!(o, Opt::Production(POpt::TradeGood { .. })));
    }
    if matches!(rig.asked().ask, Ask::Production(PAsk::Place(_))) {
        rig.pick_opt(Opt::Production(POpt::Place { spot: Spot::Space }));
    }
    rig.pick_opt(Opt::Production(POpt::Stop));
    let made: Vec<(Unit, Option<Planet>)> = rig.driver.window.made.first().map(|(_, m)| m.as_slice().to_vec()).unwrap_or_default();
    assert_eq!(made, vec![(Unit::FighterI, None), (Unit::FighterI, None)], "two fighters for one resource, in the space area");
    assert!(rig.state.board().sys(a).units.as_slice().iter().filter(|p| p.kind() == Unit::FighterI).count() == 2);
    assert_eq!(rig.state.board().planet(pa).units.as_slice().len(), 1, "the infantry that was there");
}

// -- Umbat ----------------------------------------------------------------------------------------

/// A Muaat seat with a readied agent and a war sun in one system, an Arborec seat with a flagship... (a war sun) in another.
fn umbat_table(goods: [i16; 2]) -> (Scenario, [(System, Planet); 2]) {
    let [(a, pa), (b, pb)] = two();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[a, b])
        .control(pa, 0)
        .control(pb, 1)
        .unit(a, None, Unit::WarSun, 0, 1)
        .unit(b, None, Unit::WarSun, 1, 1)
        .goods(0, goods[0])
        .goods(1, goods[1])
        .tokens(0, 3, 3, 2)
        .tokens(1, 3, 3, 2);
    (sc, [(a, pa), (b, pb)])
}

fn umbat_rig(sc: Scenario, facts: Vec<ProduceFacts>, status: LeaderStatus) -> Rig<Umbat> {
    let (mut state, kernel) = sc.build();
    state.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::Umbat, status);
    Rig::from_parts(state, kernel, Umbat::new(Seat(0), facts))
}

fn both_facts(a: System, b: System) -> Vec<ProduceFacts> {
    vec![facts(0, a, 2, Some(4)), facts(1, b, 2, Some(4))]
}

#[test]
fn umbat_offers_every_player_that_can_produce_in_seat_order_and_the_chosen_one_produces_up_to_two_cheap_units() {
    covers!("rules::leader_production::Umbat");
    let (sc, [(a, _), (b, pb)]) = umbat_table([6, 6]);
    let mut rig = umbat_rig(sc, both_facts(a, b), LeaderStatus::Readied);
    rig.run();
    assert_eq!((rig.asked().actor, rig.asked().ask), (Seat(0), Ask::Leaders(LAsk::UmbatPlayer)));
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::Player(Seat(0))), Opt::Leaders(LOpt::Player(Seat(1))), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Leaders(LOpt::Player(Seat(1))));
    // One system: no question about it; the other player produces, with a limit of 2 and a ceiling of 4.
    let Ask::Production(PAsk::Choose(ask)) = &rig.asked().ask else { panic!("the production: {:?}", rig.asked().ask) };
    assert_eq!((rig.asked().actor, ask.system, ask.limit, ask.max_unit_cost), (Seat(1), b, 2, Some(4)));
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::InfantryI, .. })));
    if matches!(rig.asked().ask, Ask::Production(PAsk::Pay(_))) {
        rig.pick_where(|o| matches!(o, Opt::Production(POpt::TradeGood { .. })));
    }
    // Two infantry used the limit of 2: the use is over without a stop.
    assert!(rig.is_done(), "{}", rig.transcript());
    assert!(rig.driver.window.used, "the agent is spent");
    assert_eq!(rig.driver.window.made.as_ref().map(|(s, m)| (*s, m.as_slice().to_vec())), Some((b, vec![(Unit::InfantryI, Some(pb)), (Unit::InfantryI, Some(pb))])));
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().econ.trade_goods, 5, "the chosen player pays");
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 6);
}

#[test]
fn umbat_declined_spends_nothing_and_a_player_who_cannot_produce_is_not_offered() {
    covers!("rules::leader_production::Umbat");
    let (sc, [(a, _), (b, pb)]) = umbat_table([6, 0]);
    let mut rig = umbat_rig(sc.clone(), both_facts(a, b), LeaderStatus::Readied);
    rig.state.board_mut().planet_mut(pb).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true);
    let before = rig.state.clone();
    rig.run();
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::Player(Seat(0))), Opt::Decline], "the other player has nothing to pay with");
    rig.pick_opt(Opt::Decline);
    assert!(rig.is_done());
    assert!(!rig.driver.window.used, "a decline spends nothing");
    assert_eq!(delta(&before, &rig.state), "");
}

#[test]
fn umbat_asks_the_chosen_player_for_the_system_when_there_are_two_and_a_decline_still_spends_the_agent() {
    covers!("rules::leader_production::Umbat");
    let [(a, pa), (b, _)] = two();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[a, b])
        .control(pa, 0)
        .unit(a, None, Unit::WarSun, 0, 1)
        .unit(b, None, Unit::WarSun, 0, 1)
        .goods(0, 6)
        .tokens(0, 3, 3, 2);
    let facts = vec![facts(0, a, 2, Some(4)), facts(0, b, 2, Some(4))];
    let mut rig = umbat_rig(sc, facts, LeaderStatus::Readied);
    rig.run();
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::Player(Seat(0))), Opt::Decline]);
    rig.pick_opt(Opt::Leaders(LOpt::Player(Seat(0))));
    assert_eq!((rig.asked().actor, rig.asked().ask), (Seat(0), Ask::Leaders(LAsk::UmbatSystem)));
    assert_eq!(rig.opts().len(), 3, "a system with a war sun, the other, a decline");
    rig.assert_options_sound();
    rig.pick_opt(Opt::Decline);
    assert!(rig.is_done());
    assert!(rig.driver.window.used, "the agent is spent");
    assert!(rig.driver.window.made.is_none(), "nothing was produced");
}

#[test]
fn umbat_needs_the_agent_readied_and_a_muaat_seat() {
    covers!("rules::leader_production::Umbat");
    let (sc, [(a, _), (b, _)]) = umbat_table([6, 6]);
    for status in [LeaderStatus::Exhausted, LeaderStatus::Locked] {
        let mut rig = umbat_rig(sc.clone(), both_facts(a, b), status);
        rig.run();
        assert!(rig.is_done(), "no question for an agent that is not ready");
        assert!(!rig.driver.window.used);
    }
    let mut other = Rig::new(sc.clone(), Umbat::new(Seat(1), both_facts(a, b)));
    other.run();
    assert!(other.is_done(), "the other seat does not play Muaat");
    // Another faction holding a readied card of that name is not the Muaat agent.
    let (mut state, kernel) = sc.build();
    state.players_mut().by_seat_mut(Seat(1)).unwrap().leaders.status.insert(Leader::Umbat, LeaderStatus::Readied);
    let mut foreign = Rig::from_parts(state, kernel, Umbat::new(Seat(1), both_facts(a, b)));
    foreign.run();
    assert!(foreign.is_done(), "no question for a seat that does not play Muaat");
    assert!(!foreign.driver.window.used);
}

#[test]
fn the_use_a_player_opens_is_given_that_players_facts_even_when_two_players_stand_in_one_system() {
    covers!("rules::leader_production::Umbat");
    let [(a, pa), _] = two();
    // Both seats have a war sun in the same system; each has its own facts (the second a limit of one).
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[a])
        .control(pa, 1)
        .unit(a, None, Unit::WarSun, 0, 1)
        .unit(a, None, Unit::TheInferno, 1, 1)
        .goods(0, 6)
        .goods(1, 6)
        .tokens(0, 3, 3, 2)
        .tokens(1, 3, 3, 2);
    let facts = vec![facts(0, a, 2, Some(4)), ProduceFacts { limit: 1, ..facts(1, a, 2, Some(4)) }];
    let mut rig = umbat_rig(sc, facts, LeaderStatus::Readied);
    rig.run();
    rig.pick_opt(Opt::Leaders(LOpt::Player(Seat(1))));
    let Ask::Production(PAsk::Choose(ask)) = &rig.asked().ask else { panic!("the production: {:?}", rig.asked().ask) };
    assert_eq!((rig.asked().actor, ask.limit), (Seat(1), 1), "the facts of the chosen player");
}

#[test]
fn umbat_counts_a_flagship_as_well_as_a_war_sun() {
    covers!("rules::leader_production::Umbat");
    let [(a, pa), (b, pb)] = two();
    // The other seat has only a flagship in its system.
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[a, b])
        .control(pa, 0)
        .control(pb, 1)
        .unit(a, None, Unit::WarSun, 0, 1)
        .unit(b, None, Unit::TheInferno, 1, 1)
        .goods(0, 6)
        .goods(1, 6)
        .tokens(0, 3, 3, 2)
        .tokens(1, 3, 3, 2);
    let mut rig = umbat_rig(sc, both_facts(a, b), LeaderStatus::Readied);
    rig.run();
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::Player(Seat(0))), Opt::Leaders(LOpt::Player(Seat(1))), Opt::Decline], "the flagship's seat is offered");
    // A carrier alone is neither.
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[a, b])
        .control(pa, 0)
        .control(pb, 1)
        .unit(a, None, Unit::WarSun, 0, 1)
        .unit(b, None, Unit::CarrierI, 1, 1)
        .goods(0, 6)
        .goods(1, 6)
        .tokens(0, 3, 3, 2)
        .tokens(1, 3, 3, 2);
    let mut rig = umbat_rig(sc, both_facts(a, b), LeaderStatus::Readied);
    rig.run();
    assert_eq!(rig.opts(), vec![Opt::Leaders(LOpt::Player(Seat(0))), Opt::Decline]);
}

#[test]
fn the_leader_production_windows_survive_every_answer_policy_with_their_properties() {
    covers!("rules::leader_production::ArborecHero");
    smoke_quiet(|| {
        let (sc, [(a, _), (b, _)]) = hero_table(6);
        hero(sc, &[a, b])
    });
    smoke_quiet(|| {
        let (sc, [(a, _), (b, _)]) = umbat_table([6, 6]);
        umbat_rig(sc, both_facts(a, b), LeaderStatus::Readied)
    });
}
