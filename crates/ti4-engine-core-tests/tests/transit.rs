//! D4-1/2: `query::transit` (which systems may be activated, what may ride in a ship's hold, how a hold is filled).

use ti4_engine_core::hex::Hex;
use ti4_engine_core::query::transit::{activatable, loadable, Hold};
use ti4_engine_core::state::Piece;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn row() -> (Scenario, [System; 4]) {
    let p = plain_systems();
    let t = [p[0], p[1], p[2], p[3]];
    let sc = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_hexes(&[(t[0], Hex::new(0, 0)), (t[1], Hex::new(1, 0)), (t[2], Hex::new(2, 0)), (t[3], Hex::new(3, 0))]);
    (sc, t)
}

#[test]
fn the_activatable_systems_are_the_map_in_hex_order_without_ours_and_a_leave_lifts_that() {
    covers!("query::transit::activatable");
    let (sc, t) = row();
    let (s, k) = sc.command_token(t[1], 0).command_token(t[2], 1).build();
    assert_eq!(activatable(&s, &k, Seat(0), false), Some(vec![t[0], t[2], t[3]]), "a rival's token does not stop us, ours does");
    assert_eq!(activatable(&s, &k, Seat(0), true), Some(vec![t[0], t[1], t[2], t[3]]), "a Mahact commander's leave");
    assert_eq!(activatable(&s, &k, Seat(1), false), Some(vec![t[0], t[1], t[3]]));
}

fn carrier_scenario() -> (Scenario, [System; 4], Planet) {
    let (sc, t) = row();
    let planet = t[1].data().planets[0];
    let sc = sc
        .control(planet, 0)
        .unit(t[1], None, Unit::CarrierI, 0, 1)
        .unit(t[1], None, Unit::FighterI, 0, 2)
        .unit(t[1], Some(planet), Unit::InfantryI, 0, 3)
        .unit(t[1], None, Unit::CruiserI, 1, 1)
        .active_system(t[3]);
    (sc, t, planet)
}

fn carrier() -> Piece {
    Piece::new(Unit::CarrierI, Seat(0))
}

#[test]
fn the_loadable_units_are_the_seats_fighters_and_ground_forces_of_the_origin_space_first() {
    covers!("query::transit::loadable");
    let (sc, t, planet) = carrier_scenario();
    let (s, k) = sc.build();
    let cands = loadable(&s, &k, Seat(0), t[1], false);
    let kinds: Vec<(Unit, Option<Planet>)> = cands.iter().map(|c| (c.piece.kind(), c.source)).collect();
    assert_eq!(kinds, vec![(Unit::FighterI, None), (Unit::FighterI, None), (Unit::InfantryI, Some(planet)), (Unit::InfantryI, Some(planet)), (Unit::InfantryI, Some(planet))]);
    assert!(cands.iter().all(|c| c.system == t[1]));
    assert!(loadable(&s, &k, Seat(1), t[1], false).iter().all(|c| c.piece.owner() == Seat(1)));
    assert!(loadable(&s, &k, Seat(0), t[0], false).is_empty(), "nothing of ours there");
}

#[test]
fn a_system_holding_our_token_other_than_the_active_one_gives_up_no_cargo_unless_tokens_are_ignored() {
    covers!("query::transit::loadable");
    let (sc, t, _) = carrier_scenario();
    let (s, k) = sc.command_token(t[1], 0).build();
    assert!(loadable(&s, &k, Seat(0), t[1], false).is_empty());
    assert_eq!(loadable(&s, &k, Seat(0), t[1], true).len(), 5, "Gravleash-style ships ignore tokens");
    let (active, k2) = carrier_scenario().0.command_token(t[3], 0).build();
    assert!(loadable(&active, &k2, Seat(0), t[3], false).is_empty(), "nothing stands in the active system");
}

#[test]
fn a_hold_has_the_capacity_of_its_ship_offers_each_kind_once_and_fills_up() {
    covers!("query::transit::Hold");
    let (sc, t, planet) = carrier_scenario();
    let (s, k) = sc.build();
    let mut hold = Hold::for_ship(&s, &k, Seat(0), t[1], carrier(), &[t[2], t[3]]);
    assert_eq!(hold.capacity, 4, "a Carrier I carries four");
    assert_eq!(hold.free_slots(), 4);
    assert!(!hold.is_complete());
    let (options, (ground, fighters, available)) = hold.options().expect("something to take");
    assert_eq!(options.len(), 2, "interchangeable units are one option: fighters and infantry");
    assert_eq!((ground, fighters, available), (0, 0, 3));
    assert!(options.iter().any(|o| o.cand.fighter) && options.iter().any(|o| o.cand.ground && o.cand.source == Some(planet)));
    let infantry = options.iter().find(|o| o.cand.ground).unwrap().index;
    assert!(hold.load(infantry) && hold.load(infantry + 1) && hold.load(infantry + 2));
    assert_eq!(hold.free_slots(), 1);
    assert_eq!(hold.cargo().len(), 3);
    let (options, (ground, _, available)) = hold.options().expect("a slot is left");
    assert_eq!((ground, available), (3, 0), "three infantry aboard, none left to take");
    assert_eq!(options.len(), 1);
    assert!(!hold.load(infantry), "an index already aboard is not offered");
    assert!(hold.load(options[0].index));
    assert!(hold.is_complete() && hold.options().is_none(), "full");
    assert!(!hold.close(), "nothing left to decline");
}

#[test]
fn closing_a_hold_ends_the_loading_and_a_ship_with_no_capacity_carries_nothing() {
    covers!("query::transit::Hold");
    let (sc, t, _) = carrier_scenario();
    let (s, k) = sc.build();
    let mut hold = Hold::for_ship(&s, &k, Seat(0), t[1], carrier(), &[t[3]]);
    assert!(hold.close());
    assert!(hold.is_complete() && hold.cargo().is_empty());
    let cruiser = Hold::for_ship(&s, &k, Seat(0), t[1], Piece::new(Unit::CruiserI, Seat(0)), &[t[3]]);
    assert_eq!(cruiser.capacity, 0);
    assert!(cruiser.is_complete(), "a Cruiser I has no capacity");
    assert!(cruiser.options().is_none());
}

#[test]
fn the_route_adds_the_units_of_the_systems_it_enters_once_each() {
    covers!("query::transit::Hold");
    let (sc, t, _) = carrier_scenario();
    let (s, k) = sc.unit(t[2], None, Unit::FighterI, 0, 1).build();
    let direct = Hold::for_ship(&s, &k, Seat(0), t[1], carrier(), &[t[3]]);
    let through = Hold::for_ship(&s, &k, Seat(0), t[1], carrier(), &[t[2], t[2], t[3]]);
    assert_eq!(through.cands.len(), direct.cands.len() + 1, "the fighter of the system it passes, counted once");
    assert!(through.cands.iter().any(|c| c.system == t[2]));
}
