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
fn ships_over_the_fleet_supply_are_removed_in_one_question_until_the_fleet_fits() {
    covers!("rules::tactical::fleet::Enforce");
    let (mut rig, system) = rig(&[(Unit::CruiserI, 3)], 1);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Tactical(TAsk::Remove { over: Over::Supply, .. })), "{:?}", rig.asked().ask);
    assert_eq!(rig.opts().len(), 1, "units of one kind are one option");
    rig.assert_options_sound();
    // One answer for the whole excess (the multi-ask conversion): one cruiser alone leaves the fleet over, three are one too many.
    let one = ti4_engine_core::choice::cards::Counts::of(0x1);
    let three = ti4_engine_core::choice::cards::Counts::of(0x3);
    rig.expect_illegal(Pick::Opt(Opt::Tactical(TOpt::RemoveSet { counts: one })));
    rig.expect_illegal(Pick::Opt(Opt::Tactical(TOpt::RemoveSet { counts: three })));
    // The option alone: that unit first, then the first options until the fleet fits.
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

/// Regression (perf_kernel suite, 3p seed 1 sol after the Leadership decline): a supply excess of two with one destroyer left (the rest
/// of the excess is not this stage's to remove) refused the option alone (`lead_set` looked for a next unit that did not exist) and the
/// set of every unit (`removal_ok` wanted the excess gone). The string's loop removed the destroyer and stopped asking.
#[test]
fn an_excess_that_outlasts_the_units_ends_once_every_unit_is_removed() {
    covers!("rules::tactical::fleet::Enforce");
    let (mut one, system) = rig(&[(Unit::DestroyerI, 1)], -1);
    one.run();
    assert!(matches!(one.asked().ask, Ask::Tactical(TAsk::Remove { over: Over::Supply, .. })), "{:?}", one.asked().ask);
    one.pick(0);
    assert!(one.is_done(), "{}", one.transcript());
    assert_eq!(count(&one, system, Unit::DestroyerI), 0);
    let (mut set, _) = rig(&[(Unit::DestroyerI, 1)], -1);
    set.run();
    set.answer_pick(Pick::Opt(Opt::Tactical(TOpt::RemoveSet { counts: ti4_engine_core::choice::cards::Counts::of(0x1) })));
    assert!(set.is_done());
}

#[test]
fn enforcement_survives_every_answer_policy() {
    covers!("rules::tactical::fleet::Enforce");
    smoke_quiet(|| rig(&[(Unit::CruiserI, 3), (Unit::FighterI, 2)], 1).0);
}

/// The multi-ask conversion's proof for the removal: the one answer (`Enforce::removal_ok`) reaches exactly the fleets the string's
/// one-unit-per-question loop reached (each question offers the first unit of each kind; the loop stops once the fleet fits), brute-forced
/// over small fleets and supplies.
#[test]
fn the_one_removal_answer_reaches_exactly_what_the_unit_by_unit_loop_reached() {
    use std::collections::BTreeSet;
    covers!("rules::tactical::fleet::Enforce");
    let fleet = |rig: &Rig<Enforce>, system: System| -> Vec<(Unit, usize)> {
        [Unit::CruiserI, Unit::DestroyerI, Unit::CarrierI].iter().map(|u| (*u, count(rig, system, *u))).collect()
    };
    for (units, limit) in [
        (&[(Unit::CruiserI, 3)][..], 1),
        (&[(Unit::CruiserI, 2), (Unit::DestroyerI, 2)][..], 2),
        (&[(Unit::CruiserI, 2), (Unit::DestroyerI, 1), (Unit::CarrierI, 1)][..], 1),
        (&[(Unit::CruiserI, 1), (Unit::DestroyerI, 3)][..], 2),
        // The excess outlasts the units the stage removes (a limit below zero stands in for fighters charged to the fleet pool): the
        // loop ends once nothing is left to remove.
        (&[(Unit::DestroyerI, 1)][..], -1),
        (&[(Unit::CruiserI, 1), (Unit::DestroyerI, 1)][..], -1),
    ] {
        let (base, system) = rig(units, limit);
        // The old loop: every sequence of one-unit answers (an option is a kind; it removes the first unit of that kind).
        let mut old = BTreeSet::new();
        let mut stack = vec![Vec::<usize>::new()];
        while let Some(path) = stack.pop() {
            let mut r = base.replay();
            r.run();
            let mut fits = r.is_done();
            for (step, i) in path.iter().enumerate() {
                let Some(ti4_engine_core::driver::Status::Asking(c)) = r.status().cloned() else { fits = true; break };
                // One unit of option `i` alone, while more must go: the old loop's answer (removing exactly that unit).
                let set = ti4_engine_core::choice::cards::with_discard_count(0, *i, 1);
                let _ = (c, step);
                let mut probe = r.replay();
                let _ = &mut probe;
                let before = fleet(&r, system);
                // Remove that unit by hand on the state and re-run the window: the loop asks again while the fleet is over.
                let kind = match r.opts()[*i] {
                    Opt::Tactical(TOpt::Remove { kind, .. }) => kind,
                    _ => unreachable!(),
                };
                let mut s = r.state.clone();
                let at = s.board().sys(system).units.as_slice().iter().position(|p| p.kind() == kind && p.owner() == Seat(0)).unwrap();
                let piece = s.board().sys(system).units.as_slice()[at];
                ti4_engine_core::kernel::fx_move::destroy_ship(&mut s, system, piece);
                let mut next = Rig::from_parts(s, ti4_engine_core_tests::kernel_all(), Enforce::new(Seat(0), system, limits(limit)));
                next.run();
                let _ = (set, before);
                r = next;
                fits = r.is_done();
            }
            if fits || r.is_done() {
                old.insert(fleet(&r, system));
                continue;
            }
            for i in 0..r.opts().len() {
                let mut p = path.clone();
                p.push(i);
                stack.push(p);
            }
        }
        // The new question: every count vector the window accepts.
        let mut new = BTreeSet::new();
        let mut first = base.replay();
        first.run();
        if first.is_done() {
            new.insert(fleet(&first, system));
        } else {
            let n = first.opts().len();
            for x in 0..5u64.pow(u32::try_from(n).unwrap()) {
                let mut bits = 0u64;
                let mut y = x;
                for i in 0..n {
                    bits = ti4_engine_core::choice::cards::with_discard_count(bits, i, u8::try_from(y % 5).unwrap());
                    y /= 5;
                }
                let mut r = first.replay();
                r.run();
                let pick = Pick::Opt(Opt::Tactical(TOpt::RemoveSet { counts: ti4_engine_core::choice::cards::Counts::of(bits) }));
                if r.try_answer(pick).is_ok() {
                    assert!(r.is_done(), "a legal set ends the supply stage: {units:?} {bits:x}");
                    new.insert(fleet(&r, system));
                }
            }
        }
        assert_eq!(new, old, "{units:?} supply {limit}");
    }
}
