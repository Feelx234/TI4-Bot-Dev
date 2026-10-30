//! D4-2a: `rules::production::window::ProductionWindow` (Choosing, Paying, Placing, Stop) from the facts the adapter would hand it.

use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt, Spot};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::rules::production::window::{ProductionWindow, Stage};
use ti4_engine_core::state::PlanetFlags;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::first_planet;

fn window(limit: i32) -> ProductionWindow {
    let (system, _) = first_planet();
    let mut buildable = [None; 9];
    buildable[0] = Some(Unit::FighterI);
    buildable[1] = Some(Unit::InfantryI);
    buildable[2] = Some(Unit::CarrierI);
    ProductionWindow {
        who: Seat(0),
        system,
        stage: Stage::Choosing,
        limit,
        remaining: limit,
        free_capacity: 0,
        small_exemptions: 0,
        credit: 0,
        discount_remaining: 0,
        fleet_limit: 8,
        armada: 0,
        free_this_use: false,
        flagship_free: false,
        only_unit: None,
        max_unit_cost: None,
        buildable,
        produced: SmallList::new(),
        purchased: false,
        placement_timing_done: false,
        agent_rules: false,
        agent_spent: false,
        emit_timing: false,
    }
}


/// Pay with `pay`, place at `spot`, stop at any other question, until the window is done.
fn finish(rig: &mut Rig<ProductionWindow>, pay: fn(&Opt) -> bool, spot: Spot) {
    let mut guard = 0;
    while !rig.is_done() && guard < 12 {
        guard += 1;
        let paying = matches!(rig.asked().ask, Ask::Production(PAsk::Pay(_)));
        let placing = matches!(rig.asked().ask, Ask::Production(PAsk::Place(_)));
        if paying {
            rig.pick_where(pay);
        } else if placing {
            rig.pick_opt(Opt::Production(POpt::Place { spot }));
        } else {
            rig.pick_opt(Opt::Production(POpt::Stop));
        }
    }
}

fn scenario(goods: i16) -> Scenario {
    let (system, planet) = first_planet();
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[system]).control(planet, 0).unit(system, Some(planet), Unit::SpaceDockI, 0, 1).goods(0, goods).tokens(0, 3, 3, 2)
}

fn rig(limit: i32, goods: i16) -> Rig<ProductionWindow> {
    Rig::new(scenario(goods), window(limit))
}

#[test]
fn the_first_question_offers_every_affordable_build_and_stopping() {
    covers!("rules::production::window::ProductionWindow");
    let mut rig = rig(3, 6);
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0));
    let Ask::Production(PAsk::Choose(ask)) = &rig.asked().ask else { panic!("the choose question: {:?}", rig.asked().ask) };
    assert_eq!((ask.limit, ask.remaining), (3, 3));
    let opts = rig.opts();
    assert!(opts.contains(&Opt::Production(POpt::Stop)), "stopping is always an option");
    assert!(opts.iter().any(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::FighterI, .. }))));
    assert!(opts.iter().any(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::CarrierI, .. }))));
    rig.assert_options_sound();
}

#[test]
fn stopping_ends_the_use_without_spending_anything() {
    covers!("rules::production::window::ProductionWindow");
    let mut rig = rig(3, 6);
    rig.run().pick_opt(Opt::Production(POpt::Stop));
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 6);
    assert!(rig.driver.window.produced.as_slice().is_empty());
}

#[test]
fn a_paid_build_spends_a_trade_good_and_places_the_unit_with_the_capacity_it_uses() {
    covers!("rules::production::window::ProductionWindow");
    let mut rig = rig(3, 6);
    rig.run();
    // the carrier costs 3: pay with trade goods, then place it in space
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::CarrierI, .. })));
    finish(&mut rig, |o| matches!(o, Opt::Production(POpt::TradeGood { .. })), Spot::Space);
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 3, "three goods paid");
    assert_eq!(rig.driver.window.produced.as_slice(), &[(Unit::CarrierI, Spot::Space)]);
    assert_eq!(rig.driver.window.remaining, 2, "one unit of capacity used");
}

#[test]
fn a_ground_force_goes_to_the_planet_with_the_dock_and_a_planet_exhausted_to_pay_stays_exhausted() {
    covers!("rules::production::window::ProductionWindow");
    let (_, planet) = first_planet();
    let mut rig = rig(3, 0);
    rig.run();
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::InfantryI, .. })));
    finish(&mut rig, |o| matches!(o, Opt::Production(POpt::Exhaust { .. })), Spot::Planet(planet));
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(rig.driver.window.produced.as_slice(), &[(Unit::InfantryI, Spot::Planet(planet)), (Unit::InfantryI, Spot::Planet(planet))], "two infantry for one resource");
    assert!(rig.state.board().planet(planet).flags.has(PlanetFlags::EXHAUSTED));
}

#[test]
fn a_window_with_no_capacity_left_or_nothing_buildable_asks_nothing() {
    covers!("rules::production::window::ProductionWindow");
    let mut spent = rig(0, 6);
    spent.run();
    assert!(spent.is_done());
    let mut none = Rig::new(scenario(6), ProductionWindow { buildable: [None; 9], ..window(3) });
    none.run();
    assert!(none.is_done());
    let mut broke = rig(3, 0);
    broke.state.board_mut().planet_mut(first_planet().1).flags.set(PlanetFlags::EXHAUSTED, true);
    broke.run();
    assert!(broke.is_done(), "nothing to pay with, nothing affordable");
}

#[test]
fn a_production_window_survives_every_answer_policy_with_its_properties() {
    covers!("rules::production::window::ProductionWindow");
    smoke_quiet(|| rig(3, 6));
    smoke_quiet(|| rig(2, 1));
}
