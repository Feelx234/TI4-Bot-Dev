//! D4-1 low layer: `query/fleet.rs` (plastic limits, fleet supply, capacity, fighter support).

use ti4_engine_core::generated::{Unit, UnitBaseType};
use ti4_engine_core::query::fleet::*;
use ti4_engine_core::state::Piece;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn table() -> Scenario {
    Scenario::lobby(3).map_ring(7)
}

fn put(s: &mut CoreState, system: System, unit: Unit, seat: u8, n: usize) {
    for _ in 0..n {
        s.board_mut().sys_mut(system).units.push(Piece::new(unit, Seat(seat)));
    }
}

fn standing_of(s: &CoreState, system: System, arriving: Option<Arrival>, half: bool) -> Standing {
    standing(s, ALL_SOURCES, Seat(0), system, 8, arriving, &move |_| half)
}

#[test]
fn the_plastic_of_a_game_is_the_printed_supply() {
    let expect = [
        (UnitBaseType::Flagship, Some(1)),
        (UnitBaseType::Warsun, Some(2)),
        (UnitBaseType::Dreadnought, Some(5)),
        (UnitBaseType::Cruiser, Some(8)),
        (UnitBaseType::Destroyer, Some(8)),
        (UnitBaseType::Carrier, Some(4)),
        (UnitBaseType::Mech, Some(4)),
        (UnitBaseType::Pds, Some(6)),
        (UnitBaseType::Spacedock, Some(3)),
        (UnitBaseType::Fighter, None),
        (UnitBaseType::Infantry, None),
    ];
    for (base, limit) in expect {
        assert_eq!(plastic(base), limit, "{base:?}");
    }
}

#[test]
fn units_held_count_the_board_and_the_units_others_have_captured() {
    let systems = plain_systems();
    let planet = systems[1].data().planets[0];
    let mut s = table().unit(systems[0], None, Unit::CruiserI, 0, 3).unit(systems[1], Some(planet), Unit::PDSI, 0, 2).state();
    assert_eq!(held_on_board(&s, ALL_SOURCES, Seat(0), UnitBaseType::Cruiser), 3);
    assert_eq!(held_on_board(&s, ALL_SOURCES, Seat(0), UnitBaseType::Pds), 2);
    assert_eq!(held_on_board(&s, ALL_SOURCES, Seat(1), UnitBaseType::Cruiser), 0);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().misc.captured_units.push((Seat(0), Unit::CruiserI));
    assert_eq!(held_on_board(&s, ALL_SOURCES, Seat(0), UnitBaseType::Cruiser), 4, "a captured cruiser is still the owner's plastic");
    assert_eq!(remaining(&s, ALL_SOURCES, Seat(0), Unit::CruiserI, 0), 8 - 4);
    assert_eq!(remaining(&s, ALL_SOURCES, Seat(0), Unit::CruiserI, 2), 8 - 4 - 2, "ships on the Crimson hero card");
    assert_eq!(remaining(&s, ALL_SOURCES, Seat(0), Unit::FighterI, 0), UNCAPPED, "fighters have no plastic");
    assert_eq!(remaining(&s, ALL_SOURCES, Seat(0), Unit::InfantryI, 0), UNCAPPED);
    let full = table().unit(systems[0], None, Unit::DreadnoughtI, 0, 5).state();
    assert_eq!(remaining(&full, ALL_SOURCES, Seat(0), Unit::DreadnoughtI, 0), 0);
    assert_eq!(remaining(&full, ALL_SOURCES, Seat(0), Unit::DreadnoughtI, 3), 0, "never negative");
    assert_eq!(remaining(&full, ti4_engine_core::tables::Scope(0), Seat(0), Unit::DreadnoughtI, 0), UNCAPPED, "a unit out of the scope");
}

#[test]
fn what_counts_against_the_fleet_and_what_takes_capacity() {
    assert!(counts_against_supply(Unit::CarrierI) && counts_against_supply(Unit::DreadnoughtI) && counts_against_supply(Unit::WarSun));
    assert!(!counts_against_supply(Unit::FighterI), "fighters are not ships against supply");
    assert!(!counts_against_supply(Unit::InfantryI) && !counts_against_supply(Unit::FactionlessMech));
    assert!(is_ground_force(Unit::InfantryI) && is_ground_force(Unit::FactionlessMech) && is_ground_force(Unit::HelTitanI) && is_ground_force(Unit::HelTitanII));
    assert!(!is_ground_force(Unit::PDSI) && !is_ground_force(Unit::CarrierI));
    assert!(consumes_capacity(Unit::FighterI) && consumes_capacity(Unit::InfantryI) && consumes_capacity(Unit::FactionlessMech));
    assert!(!consumes_capacity(Unit::HelTitanI), "a structure, though a ground force");
    assert!(!consumes_capacity(Unit::CarrierI));
}

#[test]
fn printed_capacity_and_the_cost_in_capacity() {
    assert_eq!(capacity(Unit::CarrierI), 4);
    assert_eq!(capacity(Unit::DreadnoughtI), 1);
    assert_eq!(capacity(Unit::WarSun), 6);
    assert_eq!(capacity(Unit::DestroyerI), 0);
    assert_eq!(capacity(Unit::InfantryI), 0);
    assert_eq!(capacity_cost(Unit::FighterI), 1);
    assert_eq!(capacity_cost(Unit::InfantryI), 1);
    assert_eq!(capacity_cost(Unit::CarrierI), 0);
}

#[test]
fn a_space_dock_supports_three_fighters_and_a_dimensional_tear_more() {
    assert_eq!(fighter_support(Unit::SpaceDockI), 3);
    assert_eq!(fighter_support(Unit::SpaceDockII), 3);
    assert_eq!(fighter_support(Unit::DimensionalTearI), 6);
    assert_eq!(fighter_support(Unit::DimensionalTearII), 12);
    assert_eq!(fighter_support(Unit::CarrierI), 0);
}

#[test]
fn two_carriers_carry_three_fighters_with_room_over_and_charge_only_the_ships() {
    let systems = plain_systems();
    let mut s = table().state();
    put(&mut s, systems[0], Unit::CarrierI, 0, 2);
    put(&mut s, systems[0], Unit::FighterI, 0, 3);
    let st = standing_of(&s, systems[0], None, false);
    assert_eq!((st.ships, st.transport, st.consumed, st.fighters, st.carried), (2, 8, 3, 3, 0));
    assert_eq!((st.fleet_charged, st.fleet_limit, st.capacity_excess, st.fighters_charged), (2, 8, 0, 0));
    assert_eq!((st.fleet_headroom(), st.fleet_excess(), st.capacity_free()), (6, 0, 5));
    assert!(!st.upgraded && !st.half_weight);
}

#[test]
fn fighters_beyond_capacity_are_an_excess_unless_upgraded_in_which_case_they_cost_fleet_supply() {
    let systems = plain_systems();
    let mut plain = table().state();
    put(&mut plain, systems[0], Unit::CarrierI, 0, 1);
    put(&mut plain, systems[0], Unit::FighterI, 0, 6);
    let st = standing_of(&plain, systems[0], None, false);
    assert_eq!((st.transport, st.consumed, st.capacity_excess, st.fighters_charged, st.fleet_charged), (4, 6, 2, 0, 1));
    assert_eq!(st.capacity_free(), 0);

    let mut up = table().state();
    put(&mut up, systems[0], Unit::CarrierI, 0, 1);
    put(&mut up, systems[0], Unit::FighterII, 0, 6);
    let st = standing_of(&up, systems[0], None, false);
    assert!(st.upgraded);
    assert_eq!((st.fighters_charged, st.capacity_excess, st.fleet_charged), (2, 0, 3), "the two over capacity are charged to the pool");
    let half = standing_of(&up, systems[0], None, true);
    assert!(half.half_weight);
    assert_eq!((half.fighters_charged, half.fleet_charged), (2, 1 + 1), "half a ship each, rounded up");
}

#[test]
fn ground_forces_in_space_and_a_mech_use_capacity_and_are_not_ships() {
    let systems = plain_systems();
    let mut s = table().state();
    put(&mut s, systems[0], Unit::CarrierI, 0, 1);
    put(&mut s, systems[0], Unit::InfantryI, 0, 2);
    put(&mut s, systems[0], Unit::FactionlessMech, 0, 1);
    let st = standing_of(&s, systems[0], None, false);
    assert_eq!((st.ships, st.carried, st.fighters, st.consumed), (1, 3, 0, 3));
}

#[test]
fn space_docks_on_the_planets_of_the_system_excuse_fighters_from_capacity() {
    let systems = plain_systems();
    let planet = systems[0].data().planets[0];
    let mut s = table().unit(systems[0], Some(planet), Unit::SpaceDockI, 0, 1).state();
    put(&mut s, systems[0], Unit::FighterI, 0, 3);
    let st = standing_of(&s, systems[0], None, false);
    assert_eq!((st.fighter_support, st.fighters, st.consumed, st.capacity_excess), (3, 3, 0, 0), "three fighters ride free on a dock");
    put(&mut s, systems[0], Unit::FighterI, 0, 2);
    let st = standing_of(&s, systems[0], None, false);
    assert_eq!((st.consumed, st.capacity_excess), (2, 2), "the fifth and fourth fighters need carrying");
    let other_seat = table().unit(systems[0], Some(planet), Unit::SpaceDockI, 1, 1).state();
    assert_eq!(standing_of(&other_seat, systems[0], None, false).fighter_support, 0, "a rival's dock supports nothing of ours");
}

#[test]
fn an_arriving_unit_counts_as_if_it_were_already_there() {
    let systems = plain_systems();
    let mut s = table().state();
    put(&mut s, systems[0], Unit::FighterI, 0, 3);
    let none = standing_of(&s, systems[0], None, false);
    assert_eq!((none.transport, none.capacity_excess), (0, 3));
    let with_carrier = standing_of(&s, systems[0], Some(Arrival { kind: Unit::CarrierI, count: 1, in_space: true }), false);
    assert_eq!((with_carrier.transport, with_carrier.ships, with_carrier.capacity_excess), (4, 1, 0));
    let landing_dock = standing_of(&s, systems[0], Some(Arrival { kind: Unit::SpaceDockI, count: 1, in_space: false }), false);
    assert_eq!((landing_dock.fighter_support, landing_dock.consumed), (3, 0), "a dock built on a planet supports the fighters");
    let with_fighters = standing_of(&s, systems[0], Some(Arrival { kind: Unit::FighterI, count: 2, in_space: true }), false);
    assert_eq!(with_fighters.fighters, 5);
    let with_infantry = standing_of(&s, systems[0], Some(Arrival { kind: Unit::InfantryI, count: 2, in_space: true }), false);
    assert_eq!(with_infantry.carried, 2);
}

#[test]
fn another_seats_ships_and_other_systems_do_not_count() {
    let systems = plain_systems();
    let mut s = table().state();
    put(&mut s, systems[0], Unit::CarrierI, 1, 3);
    put(&mut s, systems[1], Unit::CarrierI, 0, 3);
    let st = standing_of(&s, systems[0], None, false);
    assert_eq!((st.ships, st.transport), (0, 0));
}

#[test]
fn the_fleet_pool_is_the_tokens_plus_those_others_hold_in_it_and_unlimited_for_the_round_of_the_hero() {
    let mut s = table().tokens(0, 2, 5, 1).state();
    assert_eq!(pool_base(&s, Seat(0), 0), 5);
    assert_eq!(pool_base(&s, Seat(0), 2), 7);
    assert_eq!(pool_base(&s, Seat(9), 2), 2);
    assert!(!is_unlimited(&s, Seat(0)));
    s.players_mut().by_seat_mut(Seat(0)).unwrap().production_timers.fleet_supply_unlimited_until.set(Some(1));
    assert!(is_unlimited(&s, Seat(0)) && !is_unlimited(&s, Seat(1)));
    s.meta_mut().round = 2;
    assert!(!is_unlimited(&s, Seat(0)), "only the round it was used");
    assert_eq!(UNLIMITED_FLEET_BILL, 10_000);
}

#[test]
fn standing_arithmetic_helpers_clamp_at_zero() {
    let st = Standing { fleet_limit: 3, fleet_charged: 5, transport: 2, consumed: 6, ..Standing::default() };
    assert_eq!((st.fleet_headroom(), st.fleet_excess(), st.capacity_free()), (-2, 2, 0));
    let st = Standing { fleet_limit: 5, fleet_charged: 3, transport: 6, consumed: 2, ..Standing::default() };
    assert_eq!((st.fleet_headroom(), st.fleet_excess(), st.capacity_free()), (2, 0, 4));
}
