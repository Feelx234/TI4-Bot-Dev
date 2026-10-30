//! Rule tests of `rules::leader_windows` (the pilot of the kit: the first test is the port of
//! `xxcha_with_nothing_exhausted_is_not_used` of the core's inline tests, which stays where it is).

use ti4_engine_core::choice::leaders::Opt as LOpt;
use ti4_engine_core::generated::Planet;
use ti4_engine_core::rules::leader_windows::{HacanAgent, Helmsman, XxchaAgent};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn planets_of_plain_systems(n: usize) -> Vec<(System, Planet)> {
    plain_systems().into_iter().flat_map(|s| s.data().planets.iter().map(move |p| (s, *p))).take(n).collect()
}

fn xxcha_table(exhausted: usize) -> (Scenario, Vec<Planet>) {
    let ps = planets_of_plain_systems(3);
    let mut sc = Scenario::new(&[Faction::TheXxchaKingdom, Faction::TheArborec]).map_ring(7);
    let mut out = Vec::new();
    for (_, p) in ps.iter().take(exhausted) {
        sc = sc.control(*p, 0).exhaust(*p);
        out.push(*p);
    }
    (sc, out)
}

#[test]
fn xxcha_with_nothing_exhausted_is_not_used() {
    covers!("rules::leader_windows::XxchaAgent");
    let (sc, _) = xxcha_table(0);
    let mut rig = Rig::new(sc, XxchaAgent::new(Seat(0), false));
    rig.run();
    assert!(rig.is_done());
    assert!(!rig.driver.window.used);
    assert_eq!(delta(rig.start_state(), &rig.state), "", "nothing was touched");
}

#[test]
fn xxcha_with_one_exhausted_planet_readies_it_without_a_question() {
    covers!("rules::leader_windows::XxchaAgent");
    let (sc, ps) = xxcha_table(1);
    let mut rig = Rig::new(sc, XxchaAgent::new(Seat(0), false));
    rig.run();
    assert!(rig.is_done());
    assert!(rig.driver.window.used);
    assert!(!rig.state.board().planet(ps[0]).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED));
    let d = delta(rig.start_state(), &rig.state);
    assert!(d.contains(&format!("planet[{:?}].flags", ps[0])), "{d}");
}

#[test]
fn xxcha_with_two_asks_which_refuses_a_planet_that_is_not_exhausted_and_readies_the_one_picked() {
    covers!("rules::leader_windows::XxchaAgent");
    let (sc, ps) = xxcha_table(2);
    let mut rig = Rig::new(sc, XxchaAgent::new(Seat(0), false));
    rig.run();
    assert_eq!(rig.asked().opts.len(), 2);
    rig.assert_options_sound();
    let other = planets_of_plain_systems(3)[2].1;
    rig.expect_illegal(Pick::Opt(Opt::Leaders(LOpt::Planet(other, ti4_engine_core::choice::leaders::Shown::default()))));
    let before = rig.state.clone();
    rig.pick(1);
    assert!(rig.is_done());
    let exhausted: Vec<Planet> = ps.iter().copied().filter(|p| rig.state.board().planet(*p).flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED)).collect();
    assert_eq!(exhausted, vec![ps[0]], "the second option readied the second planet");
    assert!(delta(&before, &rig.state).lines().count() >= 1);
}

#[test]
fn hacan_gains_two_capped_at_the_limit_and_replenishes_another_to_theirs() {
    covers!("rules::leader_windows::HacanAgent");
    let sc = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheArborec, Faction::TheYinBrotherhood]);
    let limit = |sc: &Scenario, seat: u8| i16::from(sc.faction(seat).data().commodities);
    let mut rig = Rig::new(sc.clone(), HacanAgent::new(Seat(0)));
    rig.run();
    assert_eq!(rig.asked().opts.as_slice()[0], Opt::Leaders(LOpt::GainCommodities));
    assert_eq!(rig.asked().opts.len(), 3, "self plus two others, in seat order");
    rig.assert_options_sound();
    rig.pick_opt(Opt::Leaders(LOpt::Replenish(Seat(2))));
    assert!(rig.is_done() && rig.driver.window.used);
    assert_eq!(rig.state.players().by_seat(Seat(2)).unwrap().econ.commodities, limit(&sc, 2));

    let mut rig = Rig::new(sc.clone().commodities(0, 0), HacanAgent::new(Seat(0)));
    rig.run().pick_opt(Opt::Leaders(LOpt::GainCommodities));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities, 2.min(limit(&sc, 0)));
    assert_eq!(rig.try_answer(Pick::Opt(Opt::Decline)).map(|_| ()), Err(DriverError::NotAsking));
}

#[test]
fn hacan_alone_at_the_table_is_not_offered() {
    covers!("rules::leader_windows::HacanAgent");
    let mut rig = Rig::new(Scenario::new(&[Faction::TheEmiratesOfHacan]), HacanAgent::new(Seat(0)));
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.used);
}

#[test]
fn helmsman_gathers_the_flagship_and_dreadnoughts_into_the_chosen_safe_system() {
    covers!("rules::leader_windows::Helmsman");
    let systems = plain_systems();
    let (a, b, c) = (systems[0], systems[1], systems[2]);
    let sc = Scenario::new(&[Faction::TheL1Z1XMindnet, Faction::TheArborec])
        .map_ring(7)
        .unit(b, None, Unit::DreadnoughtI, 0, 2)
        .unit(c, None, Unit::DreadnoughtI, 0, 1)
        .unit(c, None, Unit::CruiserI, 0, 1)
        .unit(a, None, Unit::CruiserI, 1, 1);
    let mut rig = Rig::new(sc, Helmsman::new(Seat(0), true));
    rig.run();
    // The system with a rival ship is not offered; the other six are.
    assert_eq!(rig.asked().opts.len(), 6);
    assert!(!rig.opts().contains(&Opt::Leaders(LOpt::System(a))));
    rig.assert_options_sound();
    rig.pick_opt(Opt::Leaders(LOpt::System(systems[3])));
    assert!(rig.is_done() && rig.driver.window.used);
    let there = &rig.state.board().sys(systems[3]).units;
    assert_eq!(there.as_slice().len(), 3, "all three dreadnoughts, and not the cruiser");
    assert_eq!(rig.state.board().sys(c).units.as_slice().len(), 1, "the cruiser stays");
    ti4_engine_core_tests::assert_snapshot("leader_windows/helmsman_gathers", &rig.snapshot());
}

#[test]
fn helmsman_without_a_map_or_a_ship_is_not_offered() {
    covers!("rules::leader_windows::Helmsman");
    let systems = plain_systems();
    let sc = Scenario::new(&[Faction::TheL1Z1XMindnet, Faction::TheArborec]).map_ring(7).unit(systems[1], None, Unit::DreadnoughtI, 0, 1);
    let mut rig = Rig::new(sc.clone(), Helmsman::new(Seat(0), false));
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.used, "no map: never offered");
    let mut rig = Rig::new(Scenario::new(&[Faction::TheL1Z1XMindnet, Faction::TheArborec]).map_ring(7), Helmsman::new(Seat(0), true));
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.used, "no ship: never offered");
}
