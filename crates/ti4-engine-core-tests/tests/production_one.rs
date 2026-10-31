//! W12 (lane 7): `rules::production::one::ProduceOne`, one unit outside a use of PRODUCTION (Chaos Mapping, Freelancers, the single units
//! of the Minister of Industry before its window).

use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt, Spot};
use ti4_engine_core::generated::{Planet, UnitBaseType};
use ti4_engine_core::rules::production::one::{OneFacts, ProduceOne, Why, sort};
use ti4_engine_core::rules::production::Cx;
use ti4_engine_core::state::PlanetFlags;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{first_planet, plain_systems};

fn buildable() -> [Option<Unit>; 9] {
    let mut b = [None; 9];
    b[0] = Some(Unit::FighterI);
    b[1] = Some(Unit::InfantryI);
    b[2] = Some(Unit::CarrierI);
    b[3] = Some(Unit::DestroyerI);
    b
}

fn facts(limit: i32) -> OneFacts {
    OneFacts { buildable: buildable(), fleet_limit: limit, name: None, alias: None }
}

fn scenario(goods: i16) -> Scenario {
    let (system, planet) = first_planet();
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[system]).control(planet, 0).unit(system, Some(planet), Unit::SpaceDockI, 0, 1).goods(0, goods).tokens(0, 3, 3, 2)
}

fn rig(goods: i16, limit: i32) -> Rig<ProduceOne> {
    let (system, _) = first_planet();
    Rig::new(scenario(goods), ProduceOne::new(Seat(0), system, facts(limit)))
}

fn pay_with_goods(rig: &mut Rig<ProduceOne>) {
    while !rig.is_done() && matches!(rig.asked().ask, Ask::Production(PAsk::Pay(_))) {
        rig.pick_where(|o| matches!(o, Opt::Production(POpt::TradeGood { .. })));
    }
}

/// The planet of the dock exhausted: only the trade goods pay.
fn goods_only(goods: i16, limit: i32) -> Rig<ProduceOne> {
    let mut r = rig(goods, limit);
    r.state.board_mut().planet_mut(first_planet().1).flags.set(PlanetFlags::EXHAUSTED, true);
    r
}

#[test]
fn the_units_the_seat_can_afford_and_place_are_offered_with_their_cost_and_a_decline() {
    covers!("rules::production::one::ProduceOne");
    let mut rig = goods_only(2, 8);
    rig.run();
    let Ask::Production(PAsk::One(ask)) = rig.asked().ask else { panic!("the one-unit question: {:?}", rig.asked().ask) };
    assert_eq!((ask.who, ask.system, ask.fleet_limit), (Seat(0), first_planet().0, 8));
    let costs: Vec<(Unit, i32)> = rig.opts().iter().filter_map(|o| if let Opt::Production(POpt::One { unit, shown }) = o { Some((*unit, shown.worth)) } else { None }).collect();
    assert_eq!(costs, vec![(Unit::FighterI, 1), (Unit::InfantryI, 1), (Unit::DestroyerI, 1)], "the carrier costs 3 and two goods cannot pay it");
    assert_eq!(rig.opts().last(), Some(&Opt::Decline));
    rig.assert_options_sound();
}

#[test]
fn a_chosen_unit_is_paid_for_and_placed_and_the_window_ends() {
    covers!("rules::production::one::ProduceOne");
    let mut rig = rig(6, 8);
    rig.run();
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::One { unit: Unit::CarrierI, .. })));
    pay_with_goods(&mut rig);
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(rig.driver.window.made, Some((Unit::CarrierI, Spot::Space)));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 3, "three resources");
    assert!(!rig.driver.window.unsupported);
    let (system, _) = first_planet();
    assert_eq!(rig.state.board().sys(system).units.as_slice().iter().filter(|p| p.kind() == Unit::CarrierI).count(), 1);
}

#[test]
fn a_decline_pays_nothing_and_places_nothing() {
    covers!("rules::production::one::ProduceOne");
    let mut rig = rig(6, 8);
    rig.run().decline();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.made, None);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 6);
}

#[test]
fn nothing_affordable_asks_nothing() {
    covers!("rules::production::one::ProduceOne");
    let mut broke = rig(0, 8);
    broke.state.board_mut().planet_mut(first_planet().1).flags.set(PlanetFlags::EXHAUSTED, true);
    broke.run();
    assert!(broke.is_done());
    assert_eq!(broke.driver.window.made, None);
}

fn why_of(rig: &Rig<ProduceOne>, unit: Unit, limit: i32) -> Option<Why> {
    let (system, _) = first_planet();
    let cx = Cx::new(&rig.state, &rig.kernel, ti4_engine_core::tables::Scope(u32::from(rig.kernel.sources.0)), false);
    sort(&cx, Seat(0), system, &buildable(), limit).not_offered.as_slice().iter().find(|(u, _)| *u == unit).map(|(_, why)| *why)
}

#[test]
fn a_unit_that_is_not_offered_says_why_the_first_reason_that_applies() {
    covers!("rules::production::one::ProduceOne");
    let (system, planet) = first_planet();
    // Too dear.
    let r = goods_only(2, 8);
    assert_eq!(why_of(&r, Unit::CarrierI, 8), Some(Why::Costs { cost: 3, can_pay: 2 }));
    assert_eq!(why_of(&r, Unit::FighterI, 8), None);
    // No producer for a ground force: the dock is gone; a ship needs none.
    let mut r = rig(6, 8);
    r.state.board_mut().planet_mut(planet).units = Default::default();
    assert_eq!(why_of(&r, Unit::InfantryI, 8), Some(Why::NoPlace));
    assert_eq!(why_of(&r, Unit::FighterI, 8), None);
    // The fleet supply of the system is full: a limit of one ship, one stands.
    let mut r = rig(6, 1);
    ti4_engine_core::kernel::fx::place_unit(&mut r.state, Seat(0), system, None, Unit::CarrierI);
    assert_eq!(why_of(&r, Unit::DestroyerI, 1), Some(Why::FleetFull));
    assert_eq!(why_of(&r, Unit::FighterI, 1), None, "a fighter is no ship against the pool");
    // The box is empty: all eight destroyers stand.
    let mut r = rig(6, 99);
    for _ in 0..8 {
        ti4_engine_core::kernel::fx::place_unit(&mut r.state, Seat(0), system, None, Unit::DestroyerI);
    }
    assert_eq!(why_of(&r, Unit::DestroyerI, 99), Some(Why::Empty));
    // The first reason wins: dear and not placeable is dear.
    let mut r = goods_only(0, 8);
    r.state.board_mut().planet_mut(planet).units = Default::default();
    assert_eq!(why_of(&r, Unit::InfantryI, 8), Some(Why::Costs { cost: 1, can_pay: 0 }));
}

#[test]
fn a_ground_force_with_two_docks_asks_where() {
    covers!("rules::production::one::ProduceOne");
    let system = plain_systems().into_iter().find(|s| s.data().planets.len() >= 2).expect("a system with two planets");
    let [pa, pb]: [Planet; 2] = [system.data().planets[0], system.data().planets[1]];
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
        .map_from(&[system])
        .control(pa, 0)
        .control(pb, 0)
        .unit(system, Some(pa), Unit::SpaceDockI, 0, 1)
        .unit(system, Some(pb), Unit::SpaceDockI, 0, 1)
        .goods(0, 4)
        .tokens(0, 3, 3, 2);
    let mut rig = Rig::new(sc, ProduceOne::new(Seat(0), system, facts(8)));
    rig.run();
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::One { unit: Unit::InfantryI, .. })));
    pay_with_goods(&mut rig);
    let Ask::Production(PAsk::OnePlace { unit, system: asked }) = rig.asked().ask else { panic!("the placement question: {:?}", rig.asked().ask) };
    assert_eq!((unit, asked), (Unit::InfantryI, system));
    assert_eq!(rig.opts(), vec![Opt::Production(POpt::Place { spot: Spot::Planet(pa) }), Opt::Production(POpt::Place { spot: Spot::Planet(pb) })]);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Production(POpt::Place { spot: Spot::Planet(pb) }));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.made, Some((Unit::InfantryI, Spot::Planet(pb))));
    assert_eq!(rig.state.board().planet(pb).units.as_slice().iter().filter(|p| p.kind() == Unit::InfantryI).count(), 1);
    assert_eq!(rig.state.board().planet(pa).units.as_slice().iter().filter(|p| p.kind() == Unit::InfantryI).count(), 0);
}

#[test]
fn a_unit_over_the_capacity_of_its_system_ends_the_window_unsupported() {
    covers!("rules::production::one::ProduceOne");
    let (system, _) = first_planet();
    let fighter_after = |preplaced: usize, carrier: bool| {
        let mut r = rig(6, 8);
        for _ in 0..preplaced {
            ti4_engine_core::kernel::fx::place_unit(&mut r.state, Seat(0), system, None, Unit::FighterI);
        }
        if carrier {
            ti4_engine_core::kernel::fx::place_unit(&mut r.state, Seat(0), system, None, Unit::CarrierI);
        }
        r.run();
        r.pick_where(|o| matches!(o, Opt::Production(POpt::One { unit: Unit::FighterI, .. })));
        pay_with_goods(&mut r);
        r
    };
    // The space dock supports three fighters: the fourth has nothing to ride in (the string asks which units go; the window cannot).
    let over = fighter_after(3, false);
    assert!(over.is_done());
    assert!(over.driver.window.unsupported, "the fourth fighter stands over the capacity");
    // The dock holds the first; a carrier holds more.
    let fits = fighter_after(3, true);
    assert!(fits.is_done());
    assert!(!fits.driver.window.unsupported, "a carrier holds it");
    let dock = fighter_after(0, false);
    assert!(!dock.driver.window.unsupported, "the dock supports the first");
    assert_eq!(dock.driver.window.made, Some((Unit::FighterI, Spot::Space)));
}

#[test]
fn the_permission_to_pay_with_influence_is_the_positions() {
    covers!("rules::production::one::ProduceOne");
    // No goods and no ready planet with resources: with Freelancers' permission an influence face pays.
    let (system, planet) = first_planet();
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[system]).control(planet, 0).unit(system, Some(planet), Unit::SpaceDockI, 0, 1).goods(0, 0).tokens(0, 3, 3, 2);
    let (state, kernel) = sc.build();
    let resources = i32::from(planet.data().resources);
    let influence = i32::from(planet.data().influence);
    let mut plain = Rig::from_parts(state.clone(), kernel.clone(), ProduceOne::new(Seat(0), system, facts(8)));
    let mut allowed_state = state;
    allowed_state.meta_mut().influence_pays_for_units.insert(Seat(0));
    let mut allowed = Rig::from_parts(allowed_state, kernel, ProduceOne::new(Seat(0), system, facts(8)));
    plain.run();
    allowed.run();
    let offered = |r: &Rig<ProduceOne>| r.opts().iter().filter(|o| matches!(o, Opt::Production(POpt::One { .. }))).count();
    if influence > resources {
        assert!(offered(&allowed) > offered(&plain), "influence pays more than resources here: {resources} against {influence}");
    } else {
        assert!(offered(&allowed) >= offered(&plain));
    }
    // Never more than the unit types the seat may build.
    assert!(offered(&allowed) <= buildable().iter().flatten().count());
    let _ = UnitBaseType::Fighter;
}

#[test]
fn the_one_unit_window_survives_every_answer_policy_with_its_properties() {
    covers!("rules::production::one::ProduceOne");
    smoke_quiet(|| rig(6, 8));
    smoke_quiet(|| rig(2, 1));
}

// -- Chaos Mapping ----------------------------------------------------------------------------------

use ti4_engine_core::choice::technology::Opt as TOpt;
use ti4_engine_core::generated::Technology;
use ti4_engine_core::rules::technology_windows::{ChaosFacts, ChaosMapping, TurnStart};
use ti4_engine_core::state::Code;

fn chaos_scenario(own: bool) -> Scenario {
    let sc = scenario(6);
    if own { sc.tech(0, Technology::ChaosMapping) } else { sc }
}

fn chaos(own: bool) -> Rig<ChaosMapping> {
    let (system, _) = first_planet();
    let facts = ChaosFacts { systems: vec![system], one: facts(8) };
    Rig::new(chaos_scenario(own), ChaosMapping::new(Seat(0), facts))
}

#[test]
fn chaos_mapping_asks_the_system_then_runs_one_unit_as_a_child() {
    covers!("rules::technology_windows::ChaosMapping");
    let (system, _) = first_planet();
    let mut rig = chaos(true);
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Technology(ti4_engine_core::choice::technology::Ask::ChaosMapping));
    assert_eq!(rig.opts(), vec![Opt::Technology(TOpt::System(system)), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick_opt(Opt::Technology(TOpt::System(system)));
    assert!(matches!(rig.asked().ask, Ask::Production(PAsk::One(_))), "the one-unit question of the child: {:?}", rig.asked().ask);
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::One { unit: Unit::DestroyerI, .. })));
    while !rig.is_done() && matches!(rig.asked().ask, Ask::Production(PAsk::Pay(_))) {
        rig.pick_where(|o| matches!(o, Opt::Production(POpt::TradeGood { .. })));
    }
    assert!(rig.is_done(), "{}", rig.transcript());
    assert!(!rig.driver.window.unsupported);
    assert_eq!(rig.state.board().sys(system).units.as_slice().iter().filter(|p| p.kind() == Unit::DestroyerI).count(), 1);
}

#[test]
fn chaos_mapping_declined_or_without_the_card_or_a_system_does_nothing() {
    covers!("rules::technology_windows::ChaosMapping");
    let mut declined = chaos(true);
    declined.run().decline();
    assert!(declined.is_done());
    assert_eq!(delta(declined.start_state(), &declined.state), "");
    let mut without = chaos(false);
    without.run();
    assert!(without.is_done(), "no question without the card");
    let (system, _) = first_planet();
    let _ = system;
    let mut nowhere = Rig::new(chaos_scenario(true), ChaosMapping::new(Seat(0), ChaosFacts { systems: Vec::new(), one: facts(8) }));
    nowhere.run();
    assert!(nowhere.is_done(), "no system can produce");
}

#[test]
fn chaos_mapping_reports_a_unit_that_stands_over_its_system_as_unsupported() {
    covers!("rules::technology_windows::ChaosMapping");
    let (system, _) = first_planet();
    let mut rig = chaos(true);
    for _ in 0..3 {
        ti4_engine_core::kernel::fx::place_unit(&mut rig.state, Seat(0), system, None, Unit::FighterI);
    }
    rig.run().pick_opt(Opt::Technology(TOpt::System(system)));
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::One { unit: Unit::FighterI, .. })));
    while !rig.is_done() && matches!(rig.asked().ask, Ask::Production(PAsk::Pay(_))) {
        rig.pick_where(|o| matches!(o, Opt::Production(POpt::TradeGood { .. })));
    }
    assert!(rig.is_done());
    assert!(rig.driver.window.unsupported, "the fourth fighter has nothing to ride in");
}

#[test]
fn the_start_of_a_turn_runs_chaos_mapping_after_the_other_windows_when_given_its_facts() {
    covers!("rules::technology_windows::ChaosMapping");
    let (system, _) = first_planet();
    let window = TurnStart::new(Seat(0), Code(0)).with_chaos_mapping(ChaosFacts { systems: vec![system], one: facts(8) });
    let mut rig = Rig::new(chaos_scenario(true), window);
    rig.run();
    assert_eq!(rig.asked().ask, Ask::Technology(ti4_engine_core::choice::technology::Ask::ChaosMapping), "nothing before it applies");
    rig.decline();
    assert!(rig.is_done());
    let mut plain = Rig::new(chaos_scenario(true), TurnStart::new(Seat(0), Code(0)));
    plain.run();
    assert!(plain.is_done(), "without the facts the window ends as before");
    assert!(!plain.driver.window.unsupported());
}
