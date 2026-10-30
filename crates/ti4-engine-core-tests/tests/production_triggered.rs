//! D4-2a: the productions outside a use of PRODUCTION: `SlingRelay`, `EconomyWindow` and `HeroOffer`.

use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt};
use ti4_engine_core::rules::production::triggered::{EconomyWindow, HeroOffer, SlingRelay};
use ti4_engine_core::state::LeaderStatus;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn buildable(units: &[Unit]) -> [Option<Unit>; 9] {
    let mut out = [None; 9];
    for (i, u) in units.iter().enumerate() {
        out[i] = Some(*u);
    }
    out
}

fn production(o: POpt) -> Opt {
    Opt::Production(o)
}

fn two_seats() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec])
}

fn dock_system(n: usize) -> (System, Planet) {
    let system = plain_systems()[n];
    (system, system.data().planets[0])
}

fn space_ships(rig: &Rig<impl Window>, system: System, seat: u8) -> usize {
    rig.state.board().sys(system).units.as_slice().iter().filter(|p| p.owner() == Seat(seat)).count()
}

#[test]
fn sling_relay_with_one_dock_and_several_ships_asks_the_ship_then_pays_and_places_it_in_space() {
    covers!("rules::production::triggered::SlingRelay");
    covers!("rules::production::pay::PayStep");
    let (system, planet) = dock_system(0);
    let sc = two_seats().map_from(&[system]).control(planet, 0).unit(system, Some(planet), Unit::SpaceDockI, 0, 1).goods(0, 5);
    let mut rig = Rig::new(sc, SlingRelay::new(Seat(0), buildable(&[Unit::DestroyerI, Unit::CruiserI, Unit::CarrierI])));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Production(PAsk::SlingShip(_))), "{:?}", rig.asked().ask);
    let ships: Vec<Unit> = rig.opts().iter().filter_map(|o| if let Opt::Production(POpt::SlingShip { unit, .. }) = o { Some(*unit) } else { None }).collect();
    assert_eq!(ships, vec![Unit::DestroyerI, Unit::CruiserI, Unit::CarrierI], "every ship the seat can pay for, in buildable order");
    rig.assert_options_sound();
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::SlingShip { unit: Unit::DestroyerI, .. })));
    let mut guard = 0;
    while !rig.is_done() && guard < 6 {
        guard += 1;
        rig.pick_where(|o| matches!(o, Opt::Production(POpt::TradeGood { .. })));
    }
    assert!(rig.is_done(), "{}", rig.transcript());
    assert!(rig.driver.window.produced);
    assert_eq!(space_ships(&rig, system, 0), 1);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 4, "a destroyer costs one resource");
    ti4_engine_core_tests::assert_snapshot("production_triggered/sling", &rig.snapshot());
}

#[test]
fn sling_relay_with_two_docks_asks_the_system_first_and_an_enemy_ship_blockades_a_dock() {
    covers!("rules::production::triggered::SlingRelay");
    let ((s1, p1), (s2, p2)) = (dock_system(0), dock_system(1));
    let base = two_seats().map_from(&[s1, s2]).control(p1, 0).control(p2, 0).unit(s1, Some(p1), Unit::SpaceDockI, 0, 1).unit(s2, Some(p2), Unit::SpaceDockI, 0, 1).goods(0, 5);
    let mut rig = Rig::new(base.clone(), SlingRelay::new(Seat(0), buildable(&[Unit::DestroyerI, Unit::CruiserI])));
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Production(PAsk::SlingSystem(_))));
    assert_eq!(rig.opts().len(), 2);
    rig.assert_options_sound();
    let blockaded = base.unit(s1, None, Unit::DestroyerI, 1, 1);
    let mut blocked = Rig::new(blockaded, SlingRelay::new(Seat(0), buildable(&[Unit::DestroyerI, Unit::CruiserI])));
    blocked.run();
    assert!(matches!(blocked.asked().ask, Ask::Production(PAsk::SlingShip(_))), "only the unblockaded dock is left: its ships are asked at once");
    assert!(blocked.opts().iter().all(|o| matches!(o, Opt::Production(POpt::SlingShip { .. }))));
}

#[test]
fn sling_relay_without_a_dock_or_without_the_means_ends_at_once() {
    covers!("rules::production::triggered::SlingRelay");
    let (system, planet) = dock_system(0);
    let no_dock = two_seats().map_from(&[system]).control(planet, 0).goods(0, 5);
    let mut rig = Rig::new(no_dock, SlingRelay::new(Seat(0), buildable(&[Unit::DestroyerI])));
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.produced);
    let broke = two_seats().map_from(&[system]).unit(system, Some(planet), Unit::SpaceDockI, 0, 1);
    let mut poor = Rig::new(broke, SlingRelay::new(Seat(0), buildable(&[Unit::CarrierI])));
    poor.run();
    assert!(poor.is_done() && !poor.driver.window.produced, "a carrier costs three resources and nothing is controlled");
}

#[test]
fn sling_relay_survives_every_answer_policy() {
    covers!("rules::production::triggered::SlingRelay");
    let (system, planet) = dock_system(0);
    smoke_quiet(|| {
        let sc = two_seats().map_from(&[system]).control(planet, 0).unit(system, Some(planet), Unit::SpaceDockI, 0, 1).goods(0, 5);
        Rig::new(sc, SlingRelay::new(Seat(0), buildable(&[Unit::DestroyerI, Unit::CruiserI, Unit::CarrierI])))
    });
}

fn economy(budget: i32, goods: i16) -> Rig<EconomyWindow> {
    let (system, planet) = dock_system(0);
    let sc = two_seats().map_from(&[system]).control(planet, 0).goods(0, goods);
    Rig::new(sc, EconomyWindow::new(Seat(0), system, planet, budget, 8, buildable(&[Unit::InfantryI, Unit::CruiserI])))
}

#[test]
fn integrated_economy_builds_within_its_budget_and_stops_when_the_budget_is_spent() {
    covers!("rules::production::triggered::EconomyWindow");
    let (system, planet) = dock_system(0);
    let mut rig = economy(1, 4);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Production(PAsk::Economy(_))));
    let offered: Vec<Unit> = rig.opts().iter().filter_map(|o| if let Opt::Production(POpt::Economy { unit, .. }) = o { Some(*unit) } else { None }).collect();
    assert_eq!(offered, vec![Unit::InfantryI], "a cruiser costs two: beyond a budget of one");
    assert!(rig.opts().contains(&production(POpt::Stop)));
    rig.assert_options_sound();
    rig.pick(0);
    let mut guard = 0;
    while !rig.is_done() && guard < 6 {
        guard += 1;
        rig.pick(0);
    }
    assert!(rig.is_done(), "{}", rig.transcript());
    assert!(rig.driver.window.built);
    assert_eq!(rig.driver.window.budget, 0);
    assert_eq!(rig.state.board().planet(planet).units.as_slice().iter().filter(|p| p.owner() == Seat(0)).count(), 2, "two infantry for one resource, on the planet");
    let _ = system;
}

#[test]
fn integrated_economy_with_no_budget_or_no_means_asks_nothing_and_stopping_ends_it() {
    covers!("rules::production::triggered::EconomyWindow");
    let mut none = economy(0, 4);
    none.run();
    assert!(none.is_done() && !none.driver.window.built);
    let mut broke = economy(2, 0);
    broke.state.board_mut().planet_mut(dock_system(0).1).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true);
    broke.run();
    assert!(broke.is_done(), "nothing to pay with");
    let mut stop = economy(2, 4);
    stop.run().pick_opt(production(POpt::Stop));
    assert!(stop.is_done() && !stop.driver.window.built);
    assert_eq!(stop.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 4);
    smoke_quiet(|| economy(2, 4));
}

fn hero(status: Option<LeaderStatus>) -> Rig<HeroOffer> {
    let mut s = two_seats().state();
    if let Some(st) = status {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::HarrughGefhara, st);
    }
    s.meta_mut().production_seq = 7;
    Rig::from_parts(s, kernel_all(), HeroOffer::new(Seat(0)))
}

#[test]
fn the_hero_offer_is_made_only_to_an_unlocked_hero_and_using_it_purges_the_hero_and_marks_the_use() {
    covers!("rules::production::triggered::HeroOffer");
    for status in [None, Some(LeaderStatus::Locked), Some(LeaderStatus::Purged), Some(LeaderStatus::Readied)] {
        let mut rig = hero(status);
        rig.run();
        assert!(rig.is_done(), "{status:?}: no offer");
    }
    let mut rig = hero(Some(LeaderStatus::Unlocked));
    rig.run();
    assert_eq!(rig.asked().actor, Seat(0));
    assert_eq!(rig.opts(), vec![production(POpt::HeroUse), production(POpt::Decline)]);
    rig.assert_options_sound();
    rig.expect_illegal(Pick::Opt(production(POpt::Stop)));
    rig.pick_opt(production(POpt::HeroUse));
    assert!(rig.is_done() && rig.driver.window.used);
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert_eq!(p.leaders.status.get(&Leader::HarrughGefhara), Some(&LeaderStatus::Purged));
    assert_eq!(p.production_timers.free_production_use.get(), Some(7), "the use is marked with its sequence number");
}

#[test]
fn declining_the_hero_offer_keeps_the_hero() {
    covers!("rules::production::triggered::HeroOffer");
    let mut rig = hero(Some(LeaderStatus::Unlocked));
    rig.run().pick_opt(production(POpt::Decline));
    assert!(rig.is_done() && !rig.driver.window.used);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::HarrughGefhara), Some(&LeaderStatus::Unlocked));
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().production_timers.free_production_use.get().is_none());
    smoke_quiet(|| hero(Some(LeaderStatus::Unlocked)));
}
