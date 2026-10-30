//! Tests of the kit itself: the builders reject what the rules forbid, `delta` is empty for equal states and exact for a change, the
//! snapshot helper fails on a missing or stale file, `covers!` writes its registry, the plain kernel is not the real one.

use std::panic::catch_unwind;

use ti4_engine_core_tests::kernel::assert_hooks_wired;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{first_planet, plain_systems, spiral};
use ti4_engine_core_tests::{covers as covers_mod, snap};

#[test]
fn the_builder_rejects_a_ground_force_in_space() {
    let (system, _) = first_planet();
    let r = catch_unwind(|| Scenario::lobby(2).unit(system, None, Unit::InfantryI, 0, 1));
    assert!(r.is_err(), "infantry must be placed on a planet");
}

#[test]
fn the_builder_rejects_a_ship_on_a_planet_and_a_planet_of_another_tile() {
    let (system, planet) = first_planet();
    assert!(catch_unwind(|| Scenario::lobby(2).unit(system, Some(planet), Unit::CruiserI, 0, 1)).is_err());
    let other = plain_systems()[1];
    let foreign = other.data().planets[0];
    assert!(catch_unwind(|| Scenario::lobby(2).unit(system, Some(foreign), Unit::InfantryI, 0, 1)).is_err());
}

#[test]
fn the_builder_rejects_an_unseated_seat_and_a_repeated_faction() {
    let (system, _) = first_planet();
    assert!(catch_unwind(|| Scenario::lobby(2).unit(system, None, Unit::CruiserI, 5, 1)).is_err());
    assert!(catch_unwind(|| Scenario::new(&[Faction::TheArborec, Faction::TheArborec])).is_err());
    assert!(catch_unwind(|| Scenario::new(&[Faction::TheArborec, Faction::Neutral])).is_err());
}

#[test]
fn a_scenario_seats_real_factions_in_seat_order_with_the_real_hook_table() {
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheNekroVirus, Faction::TheYssarilTribes]);
    assert_eq!(sc.seats(), vec![Seat(0), Seat(1), Seat(2)]);
    let (state, kernel) = sc.build();
    assert_eq!(state.players().iter().map(|p| p.faction).collect::<Vec<_>>(), vec![Faction::TheEmbersOfMuaat, Faction::TheNekroVirus, Faction::TheYssarilTribes]);
    assert_eq!(state.meta().rules_version, LATEST_RULES);
    assert_hooks_wired(&kernel);
}

#[test]
fn the_plain_kernel_has_no_faction_hooks_and_the_real_one_has() {
    assert_eq!(kernel_plain().hooks.registered(), 0);
    assert!(kernel_all().hooks.registered() > 0);
    assert!(catch_unwind(|| assert_hooks_wired(&kernel_plain())).is_err(), "assert_hooks_wired fails with the plain kernel");
}

#[test]
fn the_real_kernel_fills_the_movement_slots_of_muaat_which_the_plain_kernel_leaves_empty() {
    let all = kernel_all();
    let plain = kernel_plain();
    assert!(all.hooks.movement.registered() > 0);
    assert_eq!(plain.hooks.movement.registered(), 0);
}

#[test]
fn delta_of_equal_states_is_empty_and_of_a_change_names_the_change() {
    let (system, planet) = first_planet();
    let sc = Scenario::lobby(2).map_ring(7).unit(system, Some(planet), Unit::InfantryI, 1, 2);
    let a = sc.clone().state();
    let b = sc.state();
    assert_eq!(delta(&a, &b), "");
    let mut c = a.clone();
    c.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 4;
    let d = delta(&a, &c);
    assert!(d.contains("player0.econ.trade_goods: 0 -> 4"), "{d}");
    assert_eq!(d.lines().count(), 1, "{d}");
}

#[test]
fn delta_reports_a_piece_that_moved() {
    let (system, planet) = first_planet();
    let a = Scenario::lobby(2).unit(system, Some(planet), Unit::InfantryI, 0, 1).state();
    let b = Scenario::lobby(2).unit(system, Some(planet), Unit::InfantryI, 0, 2).state();
    let d = delta(&a, &b);
    assert!(d.starts_with(&format!("planet[{planet:?}]")), "{d}");
    assert!(d.lines().all(|l| l.contains("->")), "{d}");
}

#[test]
fn the_map_puts_the_first_system_in_the_centre_and_the_ring_around_it() {
    let hexes = spiral(7);
    assert_eq!(hexes[0], ti4_engine_core::hex::Hex::new(0, 0));
    let mut ring: Vec<_> = hexes[1..].to_vec();
    ring.sort();
    ring.dedup();
    assert_eq!(ring.len(), 6);
    let sc = Scenario::lobby(2).map_ring(7);
    let state = sc.state();
    let systems = plain_systems();
    let adj = state.map().adjacency().expect("an adjacency");
    assert_eq!(adj.of(systems[0]).len(), 6, "the centre touches the whole first ring");
}

#[test]
fn a_permuted_scenario_keeps_the_calls_of_one_cell_in_order() {
    let (system, planet) = first_planet();
    let sc = Scenario::lobby(3)
        .unit(system, Some(planet), Unit::InfantryI, 0, 1)
        .unit(system, Some(planet), Unit::InfantryII, 0, 1)
        .goods(1, 3)
        .goods(1, 5)
        .control(planet, 2);
    for seed in 0..20 {
        let p = sc.permuted(seed);
        let goods: Vec<i16> = p.ops().iter().filter_map(|o| if let ti4_engine_core_tests::scenario::Op::Goods { n, .. } = o { Some(*n) } else { None }).collect();
        assert_eq!(goods, vec![3, 5]);
        assert_eq!(p.ops().len(), 5);
        assert_eq!(p.state().board().planet(planet).units.as_slice().len(), 2);
    }
}

#[test]
fn a_missing_snapshot_fails() {
    let r = catch_unwind(|| assert_snapshot("kit/this-snapshot-does-not-exist", "anything"));
    assert!(r.is_err());
}

#[test]
fn a_blessed_snapshot_that_matches_passes_and_one_that_differs_fails() {
    assert_snapshot("kit/hello", "line one  \nline two\n");
    let r = catch_unwind(|| assert_snapshot("kit/hello", "line one\nline three"));
    assert!(r.is_err());
}

#[test]
fn snapshots_are_not_stale() {
    assert_eq!(snap::stale_snapshots(), Vec::<String>::new(), "snapshot files that no test names: delete them");
}

#[test]
fn covers_writes_the_registry() {
    let dir = std::env::temp_dir().join(format!("d4-covers-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("covers.txt");
    // SAFETY-free: the variable is only read by `register`, and no other test of this binary sets it.
    // (Single-threaded setup is not guaranteed, so the test also checks the in-process registry.)
    covers!("rules::kit::ProbeWindow");
    assert!(covers_mod::registered().iter().any(|(k, l)| k == "window" && l == "rules::kit::ProbeWindow"));
    covers_card!(ActionCard::Spy);
    assert!(covers_mod::registered().iter().any(|(k, l)| k == "card" && l == "Spy"));
    let on_disk = std::fs::read_to_string(covers_mod::covers_file()).unwrap_or_default();
    assert!(on_disk.contains("window\trules::kit::ProbeWindow\t"), "the registry file {} lacks the line", covers_mod::covers_file().display());
    let _ = file;
}
