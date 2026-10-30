//! D4 lane A2: more of `ProductionWindow` (LRR 76 PRODUCTION): several builds in one use, credit from overpaying, discounts, the free use, the
//! Winnu agent (Berekar Berekon), the unit filters of a use, and the stages that end the use unasked.

use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt, Spot};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::Leader;
use ti4_engine_core::rules::production::window::{ProductionWindow, Stage};
use ti4_engine_core::state::LeaderStatus;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::first_planet;

fn window(limit: i32) -> ProductionWindow {
    let (system, _) = first_planet();
    let mut buildable = [None; 9];
    buildable[0] = Some(Unit::FighterI);
    buildable[1] = Some(Unit::InfantryI);
    buildable[2] = Some(Unit::CarrierI);
    buildable[3] = Some(Unit::DreadnoughtI);
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

fn scenario(goods: i16) -> Scenario {
    let (system, planet) = first_planet();
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[system]).control(planet, 0).unit(system, Some(planet), Unit::SpaceDockI, 0, 1).goods(0, goods).tokens(0, 3, 3, 2)
}

fn p(o: POpt) -> Opt {
    Opt::Production(o)
}

fn builds(rig: &Rig<ProductionWindow>) -> Vec<Unit> {
    rig.opts().iter().filter_map(|o| if let Opt::Production(POpt::Build { unit, .. }) = o { Some(*unit) } else { None }).collect()
}

fn pay_with_goods(rig: &mut Rig<ProductionWindow>) {
    while !rig.is_done() && matches!(rig.asked().ask, Ask::Production(PAsk::Pay(_))) {
        rig.pick_where(|o| matches!(o, Opt::Production(POpt::TradeGood { .. })));
    }
}

fn goods(rig: &Rig<ProductionWindow>) -> i16 {
    rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods
}

#[test]
fn a_use_goes_on_until_its_limit_is_spent_and_each_unit_takes_one_place() {
    covers!("rules::production::window::ProductionWindow");
    let mut rig = Rig::new(scenario(9), window(2));
    rig.run();
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::CarrierI, .. })));
    pay_with_goods(&mut rig);
    // The only place for a ship is the space area: it is placed without a question.
    assert_eq!(rig.driver.window.remaining, 1);
    assert!(!rig.is_done(), "one place of the limit is left");
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::DreadnoughtI, .. })));
    pay_with_goods(&mut rig);
    assert!(rig.is_done(), "the limit is spent: no further question");
    assert_eq!(goods(&rig), 9 - 3 - 4, "carrier 3, dreadnought 4");
    assert_eq!(rig.driver.window.produced.as_slice(), &[(Unit::CarrierI, Spot::Space), (Unit::DreadnoughtI, Spot::Space)]);
    let ships = rig.state.board().sys(first_planet().0).units.iter().filter(|u| u.owner() == Seat(0)).count();
    assert_eq!(ships, 2, "both ships are on the board");
}

#[test]
fn builds_the_player_cannot_pay_for_are_not_offered() {
    covers!("rules::production::window::ProductionWindow");
    let mut rig = Rig::new(scenario(2), window(3));
    rig.state.board_mut().planet_mut(first_planet().1).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true);
    rig.run();
    let b = builds(&rig);
    assert!(b.contains(&Unit::FighterI) && b.contains(&Unit::InfantryI), "1 resource buys two of those");
    assert!(!b.contains(&Unit::CarrierI), "a carrier costs 3 and the player holds 2 trade goods");
    assert!(!b.contains(&Unit::DreadnoughtI));
}

#[test]
fn a_discount_lowers_what_the_next_build_owes_and_is_used_up() {
    covers!("rules::production::window::ProductionWindow");
    let mut w = window(3);
    w.discount_remaining = 2;
    let mut rig = Rig::new(scenario(5), w);
    rig.run();
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::CarrierI, .. })));
    pay_with_goods(&mut rig);
    assert_eq!(goods(&rig), 4, "a carrier costs 3, less the 2 discount: 1 trade good");
    assert_eq!(rig.driver.window.discount_remaining, 0, "the discount is spent");
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::DreadnoughtI, .. })));
    pay_with_goods(&mut rig);
    assert_eq!(goods(&rig), 0, "the dreadnought pays in full");
}

#[test]
fn a_free_use_costs_nothing_and_asks_for_no_payment() {
    covers!("rules::production::window::ProductionWindow");
    let mut w = window(3);
    w.free_this_use = true;
    let mut rig = Rig::new(scenario(0), w);
    rig.state.board_mut().planet_mut(first_planet().1).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true);
    rig.run();
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::DreadnoughtI, .. })));
    assert!(matches!(rig.asked().ask, Ask::Production(PAsk::Choose(_))), "no payment question, the ship is placed in space: {:?}", rig.asked().ask);
    assert_eq!(goods(&rig), 0);
    assert_eq!(rig.driver.window.produced.as_slice(), &[(Unit::DreadnoughtI, Spot::Space)]);
    assert_eq!(rig.driver.window.remaining, 2);
}

#[test]
fn overpaying_leaves_a_credit_for_the_next_build() {
    covers!("rules::production::window::ProductionWindow");
    let (_, planet) = first_planet();
    let mut rig = Rig::new(scenario(0), window(3));
    rig.run();
    // The planet is worth `resources` > 1 in the fixture or exactly 1: pay a one-cost build with it and see the surplus.
    let worth = i32::from(planet.data().resources);
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::FighterI, .. })));
    // The planet is the only thing to pay with: it is spent without a question.
    assert_eq!(rig.driver.window.credit, (worth - 1).max(0), "what the planet paid beyond the cost stays as credit");
}

#[test]
fn the_winnu_agent_is_offered_with_the_ability_ready_and_exhausts_to_discount_two() {
    covers!("rules::production::window::ProductionWindow");
    let (system, planet) = first_planet();
    let sc = Scenario::new(&[Faction::TheWinnu, Faction::TheArborec]).map_from(&[system]).control(planet, 0).unit(system, Some(planet), Unit::SpaceDockI, 0, 1).goods(0, 1).tokens(0, 3, 3, 2);
    let (mut s, k) = sc.build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::BerekarBerekon, LeaderStatus::Readied);
    s.board_mut().planet_mut(planet).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true);
    let mut w = window(3);
    w.agent_rules = true;
    let mut rig = Rig::from_parts(s.clone(), k.clone(), w.clone());
    rig.run();
    assert!(rig.opts().contains(&p(POpt::Agent)), "{:?}", rig.opts());
    assert!(!builds(&rig).contains(&Unit::CarrierI), "1 trade good does not buy a carrier");
    rig.pick_opt(p(POpt::Agent));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::BerekarBerekon), Some(&LeaderStatus::Exhausted));
    assert!(rig.driver.window.agent_spent);
    assert_eq!(rig.driver.window.discount_remaining, 2);
    assert!(!rig.opts().contains(&p(POpt::Agent)), "once per use");
    assert!(builds(&rig).contains(&Unit::CarrierI), "with the 2 discount a carrier costs 1");
    // Not offered without the rules version flag, with the agent exhausted, or after a purchase or a free use.
    let mut off = Rig::from_parts(s.clone(), k.clone(), window(3));
    off.run();
    assert!(!off.opts().contains(&p(POpt::Agent)));
    let mut free = w.clone();
    free.free_this_use = true;
    let mut f = Rig::from_parts(s.clone(), k.clone(), free);
    f.run();
    assert!(!f.opts().contains(&p(POpt::Agent)));
    let mut spent = s;
    spent.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::BerekarBerekon, LeaderStatus::Exhausted);
    let mut e = Rig::from_parts(spent, k, w);
    e.run();
    assert!(!e.opts().contains(&p(POpt::Agent)));
}

#[test]
fn the_unit_filters_of_a_use_limit_what_is_offered() {
    covers!("rules::production::window::ProductionWindow");
    let mut only = window(3);
    only.only_unit = Some(Unit::InfantryI);
    let mut rig = Rig::new(scenario(9), only);
    rig.run();
    assert_eq!(builds(&rig), vec![Unit::InfantryI]);
    let mut cheap = window(3);
    cheap.max_unit_cost = Some(1);
    let mut rig = Rig::new(scenario(9), cheap);
    rig.run();
    let b = builds(&rig);
    assert!(b.contains(&Unit::FighterI) && b.contains(&Unit::InfantryI));
    assert!(!b.contains(&Unit::CarrierI) && !b.contains(&Unit::DreadnoughtI), "nothing dearer than 1");
}

#[test]
fn a_pick_that_does_not_belong_to_the_stage_is_illegal_and_changes_nothing() {
    covers!("rules::production::window::ProductionWindow");
    let (_, planet) = first_planet();
    let mut rig = Rig::new(scenario(9), window(3));
    rig.run();
    rig.expect_illegal(Pick::Opt(p(POpt::Place { spot: Spot::Space })));
    rig.expect_illegal(Pick::Opt(p(POpt::Place { spot: Spot::Planet(planet) })));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    rig.expect_illegal(Pick::Number(1));
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Build { unit: Unit::CarrierI, .. })));
    rig.expect_illegal(Pick::Opt(p(POpt::Stop)));
    rig.expect_illegal(Pick::Opt(p(POpt::Build { unit: Unit::CarrierI, made: 1 })));
}

#[test]
fn a_build_that_nobody_can_place_or_pay_for_ends_the_use() {
    covers!("rules::production::window::ProductionWindow");
    // Paying: nothing left to pay with after the build was chosen.
    let mut w = window(3);
    w.stage = Stage::Paying { unit: Unit::CarrierI, owed: 3, made: 1, cost: 3, paid: 0 };
    let mut rig = Rig::new(scenario(0), w);
    rig.state.board_mut().planet_mut(first_planet().1).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true);
    rig.run();
    assert!(rig.is_done() && rig.driver.window.produced.is_empty());
    // A finished window refuses everything.
    let mut done = window(3);
    done.stage = Stage::Done;
    let mut rig = Rig::new(scenario(5), done);
    rig.run();
    assert!(rig.is_done());
}
