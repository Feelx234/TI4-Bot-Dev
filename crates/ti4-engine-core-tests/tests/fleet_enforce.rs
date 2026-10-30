//! D4-2b: `rules::tactical::fleet::Enforce` (units over the fleet supply, then over the capacity, are removed one question each).

use ti4_engine_core::choice::tactical::{Ask as TAsk, Opt as TOpt, Over};
use ti4_engine_core::rules::tactical::fleet::{Enforce, Limits};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn limits(enforce: i64) -> Limits {
    Limits { enforce, shown: enforce, halves: [None, None] }
}

fn count(rig: &Rig<Enforce>, system: System, unit: Unit) -> usize {
    rig.state.board().sys(system).units.as_slice().iter().filter(|p| p.kind() == unit && p.owner() == Seat(0)).count()
}

fn rig(units: &[(Unit, usize)], limit: i64) -> (Rig<Enforce>, System) {
    let system = plain_systems()[0];
    let mut sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[system]);
    for (u, n) in units {
        sc = sc.unit(system, None, *u, 0, *n);
    }
    (Rig::new(sc, Enforce::new(Seat(0), system, limits(limit))), system)
}

#[test]
fn ships_over_the_fleet_supply_are_removed_one_question_each_until_the_fleet_fits() {
    covers!("rules::tactical::fleet::Enforce");
    let (mut rig, system) = rig(&[(Unit::CruiserI, 3)], 1);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Tactical(TAsk::Remove { over: Over::Supply, .. })), "{:?}", rig.asked().ask);
    assert_eq!(rig.opts().len(), 1, "units of one kind are one option");
    rig.assert_options_sound();
    rig.pick(0);
    assert!(matches!(rig.asked().ask, Ask::Tactical(TAsk::Remove { over: Over::Supply, .. })), "two cruisers are still over a supply of one");
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.removed, 2);
    assert_eq!(count(&rig, system, Unit::CruiserI), 1);
    ti4_engine_core_tests::assert_snapshot("fleet_enforce/supply", &rig.snapshot());
}

#[test]
fn a_fleet_that_fits_asks_nothing() {
    covers!("rules::tactical::fleet::Enforce");
    let (mut rig, system) = rig(&[(Unit::CruiserI, 2), (Unit::CarrierI, 1), (Unit::FighterI, 3)], 8);
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.removed, 0);
    assert_eq!(count(&rig, system, Unit::CruiserI), 2);
}

#[test]
fn fighters_with_no_room_to_ride_are_removed_over_the_capacity() {
    covers!("rules::tactical::fleet::Enforce");
    let (mut rig, system) = rig(&[(Unit::FighterI, 3)], 8);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Tactical(TAsk::Remove { over: Over::Capacity, .. })), "{:?}", rig.asked().ask);
    let mut guard = 0;
    while !rig.is_done() && guard < 6 {
        guard += 1;
        rig.pick(0);
    }
    assert!(rig.is_done(), "{}", rig.transcript());
    assert_eq!(count(&rig, system, Unit::FighterI), 0, "nothing carries them");
    assert_eq!(rig.driver.window.removed, 3);
}

#[test]
fn a_removal_that_names_the_wrong_kind_or_no_candidate_is_illegal() {
    covers!("rules::tactical::fleet::Enforce");
    let (mut rig, _) = rig(&[(Unit::CruiserI, 3)], 1);
    rig.run();
    rig.expect_illegal(Pick::Opt(Opt::Tactical(TOpt::Remove { index: 0, kind: Unit::CarrierI })));
    rig.expect_illegal(Pick::Opt(Opt::Tactical(TOpt::Remove { index: 9, kind: Unit::CruiserI })));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
}

#[test]
fn enforcement_survives_every_answer_policy() {
    covers!("rules::tactical::fleet::Enforce");
    smoke_quiet(|| rig(&[(Unit::CruiserI, 3), (Unit::FighterI, 2)], 1).0);
}
