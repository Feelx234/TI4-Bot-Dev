//! D4-1 low layer: `query/reach.rs` (move values with their boosts, and which ships of a seat can reach the active system).

use ti4_engine_core::containers::Opt32;
use ti4_engine_core::hex::Hex;
use ti4_engine_core::query::reach::{self, SeatFleet, SeatReach};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

/// A row A B C D, the active system D; seat 0 plays the Arborec (no movement bonus of its own), seat 1 the Hacan.
fn row() -> (Scenario, [System; 4]) {
    let p = plain_systems();
    let t = [p[0], p[1], p[2], p[3]];
    let sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmiratesOfHacan])
        .map_hexes(&[(t[0], Hex::new(0, 0)), (t[1], Hex::new(1, 0)), (t[2], Hex::new(2, 0)), (t[3], Hex::new(3, 0))])
        .active_system(t[3])
        .tokens(0, 3, 3, 2);
    (sc, t)
}

fn printed(kind: Unit) -> i32 {
    i32::from(kind.data().movement.expect("a ship with a move value"))
}

#[test]
fn a_ships_move_value_is_its_printed_one_plus_one_per_boost() {
    let (sc, t) = row();
    let (s, k) = sc.unit(t[1], None, Unit::CruiserI, 0, 1).build();
    let v = |boosts| reach::effective_move_value(&s, &k, Seat(0), t[1], Unit::CruiserI, boosts);
    assert_eq!(v((false, false)), Some(printed(Unit::CruiserI)));
    assert_eq!(v((true, false)), Some(printed(Unit::CruiserI) + 1));
    assert_eq!(v((false, true)), Some(printed(Unit::CruiserI) + 1));
    assert_eq!(v((true, true)), Some(printed(Unit::CruiserI) + 2));
    assert_eq!(reach::effective_move_value_for_ship(&s, &k, Seat(0), t[1], Unit::CruiserI, 0, (false, false)), v((false, false)));
    assert_eq!(reach::anchor_move_value(&s, &k, Seat(0), t[1], Unit::CruiserI, 0), printed(Unit::CruiserI));
}

#[test]
fn flank_speed_adds_one_for_the_activation_it_was_played_in() {
    let (sc, t) = row();
    let (mut s, k) = sc.unit(t[1], None, Unit::CruiserI, 0, 1).build();
    s.meta_mut().activation_seq = 4;
    s.players_mut().by_seat_mut(Seat(0)).unwrap().card_timers.move_bonus_activation = Opt32::some(4);
    assert_eq!(reach::effective_move_value(&s, &k, Seat(0), t[1], Unit::CruiserI, (false, false)), Some(printed(Unit::CruiserI) + 1));
    assert_eq!(reach::effective_move_value(&s, &k, Seat(1), t[1], Unit::CruiserI, (false, false)), Some(printed(Unit::CruiserI)), "only for the seat that played it");
    s.meta_mut().activation_seq = 5;
    assert_eq!(reach::effective_move_value(&s, &k, Seat(0), t[1], Unit::CruiserI, (false, false)), Some(printed(Unit::CruiserI)), "the next activation");
}

#[test]
fn a_fleet_lists_the_ships_that_can_reach_the_active_system_and_skips_the_ones_already_there() {
    let (sc, t) = row();
    let (s, k) = sc.unit(t[2], None, Unit::CarrierI, 0, 1).unit(t[1], None, Unit::CarrierI, 0, 1).unit(t[3], None, Unit::CruiserI, 0, 1).build();
    let seat = SeatReach::new(&s, &k, Seat(0), true).expect("a seat with a map");
    let rules = seat.rules_for(&s, &k, t[3]);
    let fleet = SeatFleet::new(&s, &k, Seat(0));
    let found = fleet.movable(&rules);
    let origins: Vec<System> = found.iter().map(|m| m.origin).collect();
    assert_eq!(origins, vec![t[2]], "a carrier moves one: C is one step from D, B is two, and the cruiser already stands in D");
    assert!(!found[0].gravity_drive && !found[0].ionian);
    assert_eq!(found[0].capacity, i64::from(Unit::CarrierI.data().capacity.unwrap_or(0)));
}

#[test]
fn the_gravity_drive_lets_one_more_ship_reach_until_it_is_used_this_activation() {
    let (sc, t) = row();
    let base = sc.unit(t[1], None, Unit::CarrierI, 0, 1).tech(0, Technology::GravityDrive);
    let (mut s, k) = base.build();
    s.meta_mut().activation_seq = 2;
    let found = |s: &CoreState| {
        let seat = SeatReach::new(s, &k, Seat(0), true).unwrap();
        SeatFleet::new(s, &k, Seat(0)).movable(&seat.rules_for(s, &k, t[3]))
    };
    let first = found(&s);
    assert_eq!(first.len(), 1);
    assert!(first[0].gravity_drive, "only with the boost");
    s.players_mut().by_seat_mut(Seat(0)).unwrap().timers.gravity_drive_used_activation = Opt32::some(2);
    assert!(found(&s).is_empty(), "the drive was used in this activation");
    s.meta_mut().activation_seq = 3;
    assert_eq!(found(&s).len(), 1, "ready again for the next activation");
}

#[test]
fn a_fleet_reports_the_move_value_of_every_ship_with_each_boost() {
    let (sc, t) = row();
    let (s, k) = sc.unit(t[1], None, Unit::CarrierI, 0, 1).unit(t[1], None, Unit::CruiserI, 0, 1).build();
    let fleet = SeatFleet::new(&s, &k, Seat(0));
    let carrier = printed(Unit::CarrierI);
    assert_eq!(fleet.move_value(t[1], 0, (false, false)), Some(carrier));
    assert_eq!(fleet.move_value(t[1], 0, (true, false)), Some(carrier + 1));
    assert_eq!(fleet.move_value(t[1], 0, (false, true)), Some(carrier + 1));
    assert_eq!(fleet.move_value(t[1], 0, (true, true)), Some(carrier + 2));
    assert_eq!(fleet.move_value(t[1], 1, (false, false)), Some(printed(Unit::CruiserI)), "the second ship of the system");
    assert_eq!(fleet.move_value(t[1], 2, (false, false)), None);
    assert_eq!(fleet.move_value(t[0], 0, (false, false)), None);
}

#[test]
fn another_seats_ships_are_not_in_a_seats_fleet() {
    let (sc, t) = row();
    let (s, k) = sc.unit(t[2], None, Unit::CarrierI, 1, 1).build();
    let mine = SeatFleet::new(&s, &k, Seat(0));
    assert_eq!(mine.move_value(t[2], 0, (false, false)), None);
    assert_eq!(SeatFleet::new(&s, &k, Seat(1)).move_value(t[2], 0, (false, false)), Some(printed(Unit::CarrierI)));
}

#[test]
fn law_blanking_is_a_turn_mark_that_counts_only_in_the_action_phase() {
    let (sc, _) = row();
    let s = sc.state();
    assert!(!reach::blanked(&s), "no mark");
    assert!(reach::scope_of(&kernel_all()).0 != 0);
    assert!(reach::technology_text(&sc_with_tech(), Seat(0), Technology::GravityDrive));
    assert!(!reach::technology_text(&s, Seat(0), Technology::GravityDrive));
}

fn sc_with_tech() -> CoreState {
    row().0.tech(0, Technology::GravityDrive).state()
}
