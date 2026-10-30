//! D4 lane A3: `query/transit.rs` beyond lane A1: the systems that may be activated (hyperlanes, the Fracture, the Nexus, Chaos Mapping), the hold of a
//! ship with a free rider, and the wormhole hops of a route.

use ti4_engine_core::hex::Hex;
use ti4_engine_core::query::transit::{self, activatable, Hold};
use ti4_engine_core::state::{CoreState, Piece, SysFlags};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

const DUEL: [Faction; 2] = [Faction::TheEmbersOfMuaat, Faction::TheArborec];

fn row4() -> (Scenario, [System; 4]) {
    let p = plain_systems();
    let t = [p[0], p[1], p[2], p[3]];
    let sc = Scenario::new(&DUEL).map_hexes(&[(t[0], Hex::new(0, 0)), (t[1], Hex::new(1, 0)), (t[2], Hex::new(2, 0)), (t[3], Hex::new(3, 0))]);
    (sc, t)
}

fn touch(s: &mut CoreState, system: System) {
    s.board_mut().sys_mut(system).flags.set(SysFlags::TOUCHED, true);
}

// ---- activatable ---------------------------------------------------------------------------------

#[test]
fn nothing_can_be_activated_without_a_map() {
    let bare = CoreState::all_loaded();
    assert_eq!(activatable(&bare, &kernel_all(), Seat(0), false), None);
}

#[test]
fn a_hyperlane_tile_is_never_activated() {
    let lane = System::ALL.iter().copied().find(|s| s.data().hyperlane).expect("a hyperlane tile");
    let p = plain_systems();
    let s = Scenario::new(&DUEL).map_hexes(&[(p[0], Hex::new(0, 0)), (lane, Hex::new(1, 0)), (p[1], Hex::new(2, 0))]);
    let (s, k) = s.build();
    assert_eq!(activatable(&s, &k, Seat(0), false), Some(vec![p[0], p[1]]));
}

#[test]
fn systems_the_board_has_an_entry_for_follow_the_map_once_each_in_id_order() {
    let (sc, t) = row4();
    let (mut s, k) = sc.build();
    // Two tiles of the Fracture (not on the printed map) and one printed tile that is touched as well.
    touch(&mut s, System::FractureVoidRight);
    touch(&mut s, System::FractureEgressLeft);
    touch(&mut s, t[1]);
    let found = activatable(&s, &k, Seat(0), false).unwrap();
    assert_eq!(found, vec![t[0], t[1], t[2], t[3], System::FractureEgressLeft, System::FractureVoidRight]);
    // A touched hyperlane is skipped too, and our token on a Fracture tile keeps it out.
    s.board_mut().sys_mut(System::FractureEgressLeft).cmd.insert(Seat(0));
    let found = activatable(&s, &k, Seat(0), false).unwrap();
    assert_eq!(found, vec![t[0], t[1], t[2], t[3], System::FractureVoidRight]);
    assert!(activatable(&s, &k, Seat(0), true).unwrap().contains(&System::FractureEgressLeft));
}

#[test]
fn the_map_is_listed_in_hex_order_whatever_the_order_the_tiles_were_placed_in() {
    let p = plain_systems();
    let sc = Scenario::new(&DUEL).map_hexes(&[(p[2], Hex::new(2, 0)), (p[0], Hex::new(0, 0)), (p[1], Hex::new(1, 0))]);
    let (s, k) = sc.build();
    let found = activatable(&s, &k, Seat(0), false).unwrap();
    // Hex order is the order of the hexes themselves, not of placement.
    let mut by_hex = vec![(Hex::new(2, 0), p[2]), (Hex::new(0, 0), p[0]), (Hex::new(1, 0), p[1])];
    by_hex.sort();
    assert_eq!(found, by_hex.into_iter().map(|(_, s)| s).collect::<Vec<_>>());
}

#[test]
fn the_wormhole_nexus_is_activatable_from_rules_version_21_when_it_is_in_play_off_the_map() {
    let nexus = System::MalliceLockedNexus;
    let (sc, t) = row4();
    let (mut s, k) = sc.build();
    let mut g = (**s.galaxy().unwrap()).clone();
    g.place_off_map(nexus).unwrap();
    s.map = ti4_engine_core::state::MapSlice::from_galaxy(g);
    s.meta_mut().rules_version = 21;
    let found = activatable(&s, &k, Seat(0), false).unwrap();
    assert_eq!(found, vec![t[0], t[1], t[2], t[3], nexus], "last, after the map");
    s.meta_mut().rules_version = 20;
    assert_eq!(activatable(&s, &k, Seat(0), false).unwrap(), vec![t[0], t[1], t[2], t[3]]);
    // Our token on it keeps us from activating it.
    s.meta_mut().rules_version = 21;
    s.board_mut().sys_mut(nexus).cmd.insert(Seat(0));
    assert_eq!(activatable(&s, &k, Seat(0), false).unwrap(), vec![t[0], t[1], t[2], t[3]]);
    assert_eq!(activatable(&s, &k, Seat(1), false).unwrap().last(), Some(&nexus));
    // A Nexus that is on the map is listed with the map, once.
    let on_map = Scenario::new(&DUEL).map_hexes(&[(plain_systems()[0], Hex::new(0, 0)), (nexus, Hex::new(1, 0))]);
    let (s2, k2) = on_map.build();
    assert_eq!(activatable(&s2, &k2, Seat(0), false).unwrap(), vec![plain_systems()[0], nexus]);
}

#[test]
fn chaos_mapping_keeps_the_others_out_of_an_asteroid_field_that_holds_a_saar_ship() {
    let field = System::ALL.iter().copied().find(|s| s.data().planets.is_empty() && s.data().anomalies.asteroid_field).expect("an asteroid field");
    let nebula = System::Nebula;
    let p = plain_systems();
    let duel = [Faction::TheClanOfSaar, Faction::TheArborec];
    let base = Scenario::new(&duel).map_hexes(&[(p[0], Hex::new(0, 0)), (field, Hex::new(1, 0)), (nebula, Hex::new(2, 0)), (p[1], Hex::new(3, 0))]);
    let (s, k) = base.clone().unit(field, None, Unit::CruiserI, 0, 1).unit(nebula, None, Unit::CruiserI, 0, 1).tech(0, Technology::ChaosMapping).build();
    let all = vec![p[0], field, nebula, p[1]];
    assert_eq!(activatable(&s, &k, Seat(0), false).unwrap(), all, "the Saar activate their own field");
    assert_eq!(activatable(&s, &k, Seat(1), false).unwrap(), vec![p[0], nebula, p[1]], "the field is closed, the nebula with a Saar ship is not");
    // No technology, no ship, or ground forces only: the field stays open.
    let (open, k2) = base.clone().unit(field, None, Unit::CruiserI, 0, 1).build();
    assert_eq!(activatable(&open, &k2, Seat(1), false).unwrap(), all);
    let (empty, k3) = base.tech(0, Technology::ChaosMapping).build();
    assert_eq!(activatable(&empty, &k3, Seat(1), false).unwrap(), all);
}

// ---- the hold ------------------------------------------------------------------------------------------

fn argent() -> (Scenario, [System; 4], Planet) {
    let p = plain_systems();
    let t = [p[0], p[1], p[2], p[3]];
    let planet = t[1].data().planets[0];
    let sc = Scenario::new(&[Faction::TheArgentFlight, Faction::TheArborec])
        .map_hexes(&[(t[0], Hex::new(0, 0)), (t[1], Hex::new(1, 0)), (t[2], Hex::new(2, 0)), (t[3], Hex::new(3, 0))])
        .control(planet, 0)
        .unit(t[1], None, Unit::CarrierI, 0, 1)
        .unit(t[1], Some(planet), Unit::InfantryI, 0, 4)
        .unit(t[1], Some(planet), Unit::AerieSentinel, 0, 1)
        .active_system(t[3]);
    (sc, t, planet)
}

#[test]
fn the_aerie_sentinel_rides_in_the_hold_without_using_a_slot() {
    let (sc, t, _) = argent();
    let (s, k) = sc.build();
    let ship = Piece::new(Unit::CarrierI, Seat(0));
    let mut hold = Hold::for_ship(&s, &k, Seat(0), t[1], ship, &[t[3]]);
    assert_eq!(hold.capacity, 4);
    assert_eq!(hold.cands.iter().filter(|c| c.free).count(), 1);
    assert!(hold.cands.iter().all(|c| c.free == (c.piece.kind() == Unit::AerieSentinel)));
    let (options, _) = hold.options().unwrap();
    let infantry = options.iter().find(|o| o.cand.piece.kind() == Unit::InfantryI).unwrap().index;
    for i in 0..4 {
        assert!(hold.load(infantry + i), "infantry {i}");
    }
    assert_eq!(hold.free_slots(), 0, "four infantry fill the four slots");
    assert!(!hold.is_complete(), "the sentinel still rides free");
    let (options, _) = hold.options().expect("the free rider is offered");
    assert_eq!(options.len(), 1);
    assert_eq!(options[0].cand.piece.kind(), Unit::AerieSentinel);
    assert!(hold.load(options[0].index));
    assert_eq!(hold.free_slots(), 0, "a free rider uses no slot");
    assert!(hold.is_complete() && hold.options().is_none());
    assert_eq!(hold.cargo().len(), 5);
}

#[test]
fn a_ship_without_capacity_carries_nothing_not_even_a_free_rider() {
    let (sc, t, _) = argent();
    let (s, k) = sc.unit(t[1], None, Unit::CruiserI, 0, 1).build();
    let hold = Hold::for_ship(&s, &k, Seat(0), t[1], Piece::new(Unit::CruiserI, Seat(0)), &[t[3]]);
    assert_eq!(hold.capacity, 0);
    assert!(hold.cands.is_empty(), "a ground force rides on a ship that can carry, the sentinel included");
    assert!(hold.closed && hold.is_complete() && hold.options().is_none());
}

#[test]
fn identical_units_are_one_option_but_a_damaged_one_or_one_on_another_planet_or_system_is_its_own() {
    let p = plain_systems();
    let t = [p[0], p[1], p[2], p[3]];
    let (a, b) = (t[1].data().planets[0], t[2].data().planets[0]);
    let sc = Scenario::new(&DUEL)
        .map_hexes(&[(t[0], Hex::new(0, 0)), (t[1], Hex::new(1, 0)), (t[2], Hex::new(2, 0)), (t[3], Hex::new(3, 0))])
        .unit(t[1], None, Unit::CarrierI, 0, 1)
        .unit(t[1], Some(a), Unit::InfantryI, 0, 2)
        .unit(t[1], Some(a), Unit::FactionlessMech, 0, 2)
        .unit(t[2], Some(b), Unit::InfantryI, 0, 1)
        .unit(t[1], None, Unit::FighterI, 0, 2);
    let (mut s, k) = sc.build();
    // The second mech on planet a is damaged: the sustain-damage mark tells it from the first.
    let planet = s.board_mut().planet_mut(a);
    let mech_index = planet.units.as_slice().iter().rposition(|u| u.kind() == Unit::FactionlessMech).unwrap();
    let hurt = planet.units.as_slice()[mech_index].with_sustained(true);
    planet.units.as_mut_slice()[mech_index] = hurt;
    let hold = Hold::for_ship(&s, &k, Seat(0), t[1], Piece::new(Unit::CarrierI, Seat(0)), &[t[2], t[3]]);
    let (options, (ground, fighters, available)) = hold.options().unwrap();
    let kinds: Vec<(Unit, Option<Planet>, System)> = options.iter().map(|o| (o.cand.piece.kind(), o.cand.source, o.cand.system)).collect();
    assert_eq!(
        kinds,
        vec![(Unit::FighterI, None, t[1]), (Unit::InfantryI, Some(a), t[1]), (Unit::FactionlessMech, Some(a), t[1]), (Unit::FactionlessMech, Some(a), t[1]), (Unit::InfantryI, Some(b), t[2])]
    );
    assert_eq!((ground, fighters, available), (0, 0, 5));
    assert!(options.iter().all(|o| o.remaining == 4));
    assert_eq!(options.iter().filter(|o| o.cand.ground).count(), 4);
    assert!(options[0].cand.fighter && !options[0].cand.ground);
}

#[test]
fn closing_the_hold_is_refused_once_it_offers_nothing_and_loading_a_foreign_index_is_refused() {
    let (sc, t, _) = argent();
    let (s, k) = sc.build();
    let mut hold = Hold::for_ship(&s, &k, Seat(0), t[1], Piece::new(Unit::CarrierI, Seat(0)), &[]);
    assert!(!hold.load(999), "not a candidate");
    assert!(hold.loaded.is_empty());
    assert!(hold.close());
    assert!(hold.closed && hold.options().is_none() && hold.is_complete());
    assert!(!hold.load(0), "a closed hold takes nothing");
    assert!(!hold.close(), "and is not closed twice");
}

// ---- wormhole hops ---------------------------------------------------------------------------------------

#[test]
fn a_hop_over_a_wormhole_link_is_counted_and_a_step_over_a_border_is_not() {
    // Two alpha tiles far apart along a line of plain tiles: adjacent through their wormholes only.
    let (a, a2, b) = (System::AlphaWormhole, System::AsteroidsAlphaWormhole, System::BetaWormhole);
    let p = plain_systems();
    let sc = Scenario::new(&DUEL).map_hexes(&[(a, Hex::new(0, 0)), (p[0], Hex::new(1, 0)), (p[1], Hex::new(2, 0)), (a2, Hex::new(3, 0)), (b, Hex::new(4, 0))]);
    let s = sc.state();
    assert_eq!(transit::wormhole_hops(&s, Seat(0), a, &[p[0]]), Some(0), "one step along the border");
    assert_eq!(transit::wormhole_hops(&s, Seat(0), a, &[a2]), Some(1), "through the wormhole");
    assert_eq!(transit::wormhole_hops(&s, Seat(0), a2, &[a, p[0]]), Some(1), "back and then one step");
    assert_eq!(transit::wormhole_hops(&s, Seat(0), a, &[a2, a, a2]), Some(3));
    assert_eq!(transit::wormhole_hops(&s, Seat(0), a, &[]), Some(0));
    assert_eq!(transit::wormhole_hops(&s, Seat(0), a, &[a]), Some(0), "staying put is no hop");
    assert_eq!(transit::wormhole_hops(&s, Seat(0), a2, &[b]), Some(0), "a2 and b are neighbours by the border");
    // A tile with no wormhole of its own is not a hop target even when far away.
    assert_eq!(transit::wormhole_hops(&s, Seat(0), a, &[p[1]]), Some(0));
    // Alpha and beta are not linked for everyone but the Ghosts (Quantum Entanglement).
    let far = Scenario::new(&[Faction::TheGhostsOfCreuss, Faction::TheArborec]).map_hexes(&[(a, Hex::new(0, 0)), (p[0], Hex::new(1, 0)), (p[1], Hex::new(2, 0)), (b, Hex::new(5, 0))]).state();
    assert_eq!(transit::wormhole_hops(&far, Seat(0), a, &[b]), Some(1));
    assert_eq!(transit::wormhole_hops(&far, Seat(1), a, &[b]), Some(0));
    assert_eq!(transit::wormhole_hops(&CoreState::all_loaded(), Seat(0), a, &[b]), None);
}

#[test]
fn the_ghosts_hop_between_alpha_and_beta_even_when_the_nexus_switches_ordinary_wormholes_off() {
    let nexus = System::MalliceNexus;
    let (alpha, p) = (System::AlphaWormhole, plain_systems());
    let build = |factions: &[Faction]| {
        let mut s = Scenario::new(factions).map_hexes(&[(alpha, Hex::new(0, 0)), (p[0], Hex::new(1, 0)), (nexus, Hex::new(4, 0))]).state();
        let mut g = (**s.galaxy().unwrap()).clone();
        g.nexus_wormholes_off = true;
        g.set_wormholes(nexus, ti4_engine_core::tables::WH_ALPHA_BETA);
        s.map = ti4_engine_core::state::MapSlice::from_galaxy(g);
        s
    };
    // The Nexus' own wormholes are off: for the others the nexus has no wormhole, and no hop is counted.
    let others = build(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]);
    assert_eq!(transit::wormhole_hops(&others, Seat(0), alpha, &[nexus]), Some(0));
    // The Ghosts' Quantum Entanglement is not stopped by it.
    let ghosts = build(&[Faction::TheGhostsOfCreuss, Faction::TheArborec]);
    assert_eq!(transit::wormhole_hops(&ghosts, Seat(0), alpha, &[nexus]), Some(1));
    assert_eq!(transit::wormhole_hops(&ghosts, Seat(1), alpha, &[nexus]), Some(0));
}
