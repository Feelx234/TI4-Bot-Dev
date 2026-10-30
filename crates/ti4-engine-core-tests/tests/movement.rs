//! D4-1 low layer: `movement.rs` (route search over a row of tiles), expectations from the movement rules (LRR 58): a ship moves up to
//! its move value, cannot pass systems with another player's ships, supernovae, asteroid fields (without the technology), nebulae, or
//! leave a system holding its own command token (the active system excepted).

use std::sync::Arc;

use ti4_engine_core::bits::{SysSet, UnitSet};
use ti4_engine_core::galaxy::CoreGalaxy;
use ti4_engine_core::generated::System;
use ti4_engine_core::hex::Hex;
use ti4_engine_core::movement::CoreRules;
use ti4_engine_core::tables::{SYSTEM_COUNT, WH_ALPHA};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::scenario::plain_systems;

static NO_LANES: [Option<&[(u8, u8)]>; SYSTEM_COUNT] = [None; SYSTEM_COUNT];

/// A row of tiles A B C D (hexes (0,0) .. (3,0)); the active system is the last.
fn row(tiles: [System; 4]) -> (CoreGalaxy, CoreRules) {
    let mut g = CoreGalaxy::new(&NO_LANES);
    for (i, t) in tiles.iter().enumerate() {
        g.place(*t, Hex::new(i as i32, 0)).unwrap();
    }
    let adj = Arc::new(g.adjacency());
    let rules = CoreRules::with_board(adj, ALL_SOURCES, tiles[3], SysSet::EMPTY, SysSet::EMPTY);
    (g, rules)
}

fn plain_row() -> ([System; 4], CoreGalaxy, CoreRules) {
    let p = plain_systems();
    let tiles = [p[0], p[1], p[2], p[3]];
    let (g, r) = row(tiles);
    (tiles, g, r)
}

fn anomalous(pick: impl Fn(&ti4_engine_core::generated::Anomalies) -> bool) -> System {
    System::ALL.into_iter().find(|s| !s.data().hyperlane && pick(&s.data().anomalies) && s.data().anomalies.asteroid_field as u8 + s.data().anomalies.supernova as u8 + s.data().anomalies.nebula as u8 + s.data().anomalies.gravity_rift as u8 <= 1).expect("a tile with that anomaly")
}

#[test]
fn a_ship_reaches_the_active_system_exactly_when_the_move_value_covers_the_row() {
    let (t, _, rules) = plain_row();
    assert_eq!(rules.path_from_ship(t[0], 3, None), Some(vec![t[0], t[1], t[2], t[3]]));
    assert!(rules.can_reach_ship(t[0], 3, None) && !rules.can_reach_ship(t[0], 2, None));
    assert_eq!(rules.path_from_ship(t[2], 1, None), Some(vec![t[2], t[3]]));
    assert!(!rules.can_reach_ship(t[2], 0, None), "no move value, no move");
    assert!(rules.can_reach_ship(t[0], 9, None), "more than enough");
}

#[test]
fn another_players_ships_block_the_way_through_but_not_the_destination() {
    let (t, _, mut rules) = plain_row();
    rules.enemy_ships.insert(t[1].index());
    assert!(!rules.can_reach_ship(t[0], 3, None), "cannot pass B");
    assert!(rules.can_reach_ship(t[1], 2, None), "starting in B is fine");
    rules.ignore_enemy_ships = true;
    assert!(rules.can_reach_ship(t[0], 3, None));
    rules.ignore_enemy_ships = false;
    rules.ignore_enemy_ships_from = Some(t[0]);
    assert!(rules.can_reach_ship(t[0], 3, None), "ships that start here pass enemies");
    assert!(!rules.can_reach_ship(t[1], 2, None) == false);
    let (t, _, mut rules) = plain_row();
    rules.enemy_ships.insert(t[3].index());
    assert!(rules.can_reach_ship(t[2], 1, None), "the active system may hold enemies: that is the combat");
}

#[test]
fn a_ship_may_not_depart_from_a_system_with_our_command_token_unless_it_is_the_active_one() {
    let (t, _, mut rules) = plain_row();
    rules.own_command_tokens.insert(t[0].index());
    assert!(!rules.may_depart(t[0]) && rules.may_depart(t[1]));
    assert!(!rules.can_reach_ship(t[0], 3, None));
    rules.own_command_tokens.insert(t[3].index());
    assert!(rules.may_depart(t[3]), "the active system always may");
    rules.command_tokens_ignored = true;
    assert!(rules.may_depart(t[0]) && rules.can_reach_ship(t[0], 3, None));
    let (t, _, mut rules) = plain_row();
    rules.own_command_tokens.insert(t[1].index());
    rules.token_free_types.insert(5);
    assert!(rules.can_reach_ship(t[1], 2, Some(ti4_engine_core::generated::Unit::ALL[5])), "a unit type that ignores tokens");
    assert!(!rules.can_reach_ship(t[1], 2, Some(ti4_engine_core::generated::Unit::ALL[6])));
}

#[test]
fn supernovae_and_asteroid_fields_stop_a_route_unless_opened() {
    let p = plain_systems();
    let nova = anomalous(|a| a.supernova);
    let belt = anomalous(|a| a.asteroid_field);
    let (g, mut rules) = row([p[0], nova, p[2], p[3]]);
    let _ = g;
    assert!(!rules.can_reach_ship(p[0], 3, None) && !rules.can_enter(nova));
    rules.supernovae_open = true;
    assert!(rules.can_enter(nova) && rules.can_reach_ship(p[0], 3, None));
    let (_, mut rules) = row([p[0], p[1], nova, p[3]]);
    rules.supernovae_pass_through = true;
    assert!(rules.can_reach_ship(p[0], 3, None), "pass-through only");
    assert!(!rules.can_enter(nova));
    let (_, mut rules) = row([p[0], belt, p[2], p[3]]);
    assert!(!rules.can_reach_ship(p[0], 3, None) && !rules.can_enter(belt));
    rules.asteroid_fields_open = true;
    assert!(rules.can_reach_ship(p[0], 3, None));
    rules.asteroid_fields_open = false;
    rules.anomalies_ignored = true;
    assert!(rules.can_reach_ship(p[0], 3, None), "ignoring anomalies opens everything");
}

#[test]
fn a_nebula_can_be_the_destination_but_not_a_stop_on_the_way_and_caps_a_ship_that_starts_in_it() {
    let p = plain_systems();
    let nebula = anomalous(|a| a.nebula);
    let (_, mut rules) = row([p[0], nebula, p[2], p[3]]);
    assert!(!rules.can_reach_ship(p[0], 3, None), "cannot pass through");
    rules.nebulae_open = true;
    assert!(rules.can_reach_ship(p[0], 3, None));
    assert!(rules.nebula_caps(nebula) && !rules.nebula_caps(p[0]));
    // From inside the nebula the budget is one whatever the move value.
    assert!(!rules.can_reach_ship(nebula, 3, None), "two steps away with a budget of one");
    assert!(rules.can_reach_ship(p[2], 3, None));
    rules.nebulae_ignored = true;
    assert!(!rules.nebula_caps(nebula) && rules.can_reach_ship(nebula, 2, None));
    // As the active system a nebula is enterable.
    let (_, rules) = row([p[0], p[1], p[2], nebula]);
    assert!(rules.can_enter(nebula) && rules.can_reach_ship(p[2], 1, None));
}

#[test]
fn a_gravity_rift_adds_one_to_the_move_of_a_ship_that_passes_it_and_extra_steps_once() {
    let p = plain_systems();
    let rift = anomalous(|a| a.gravity_rift);
    let (_, mut rules) = row([p[0], rift, p[2], p[3]]);
    assert!(rules.is_gravity_rift(rift) && !rules.is_gravity_rift(p[0]));
    assert!(rules.can_reach_ship(p[0], 2, None), "three steps, one gained in the rift");
    assert!(!rules.can_reach_ship(p[0], 1, None));
    rules.rift_extra_steps = 1;
    assert!(rules.can_reach_ship(p[0], 1, None), "the bonus applies once");
    rules.anomalies_ignored = true;
    assert!(rules.can_reach_ship(p[0], 3, None) && !rules.can_reach_ship(p[0], 2, None), "no gain when anomalies are ignored");
    let mut rules = row([p[0], p[1], p[2], p[3]]).1;
    rules.gravity_rift_systems.insert(p[1].index());
    assert!(rules.is_gravity_rift(p[1]), "a rift created by a card");
    assert_eq!(rules.rifts_exited(&[p[0], p[1], p[2], p[3]]), 1, "the last system is not exited");
    assert_eq!(rules.rifts_exited(&[p[0], p[1]]), 0);
    assert_eq!(rules.rifts_exited(&[]), 0);
    rules.rift_roll_exempt.insert(p[1].index());
    assert_eq!(rules.rifts_exited(&[p[0], p[1], p[2]]), 0, "exempt");
}

#[test]
fn wormholes_make_distant_tiles_adjacent_and_the_search_uses_them() {
    let p = plain_systems();
    let tiles = [p[0], p[1], p[2], p[3]];
    let mut g = CoreGalaxy::new(&NO_LANES);
    for (i, t) in tiles.iter().enumerate() {
        g.place(*t, Hex::new(i as i32, 0)).unwrap();
    }
    g.set_wormholes(tiles[0], WH_ALPHA);
    g.set_wormholes(tiles[3], WH_ALPHA);
    let rules = CoreRules::with_board(Arc::new(g.adjacency()), ALL_SOURCES, tiles[3], SysSet::EMPTY, SysSet::EMPTY);
    assert_eq!(rules.path_from_ship(tiles[0], 1, None), Some(vec![tiles[0], tiles[3]]));
}

#[test]
fn extra_adjacency_blocked_edges_and_also_adjacent_change_the_neighbours() {
    let (t, _, mut rules) = plain_row();
    let mut link = SysSet::EMPTY;
    link.insert(t[3].index());
    rules.extra_adjacency.push((t[0], link));
    assert_eq!(rules.path_from_ship(t[0], 1, None), Some(vec![t[0], t[3]]), "a one-way link A to D");
    assert!(rules.can_reach_ship(t[0], 1, None));
    let (t, _, mut rules) = plain_row();
    let mut cut = SysSet::EMPTY;
    cut.insert(t[2].index());
    rules.blocked_edges.push((t[1], cut));
    assert!(!rules.can_reach_ship(t[0], 3, None), "the border B-C is closed");
    let (t, _, mut rules) = plain_row();
    rules.also_adjacent.insert(t[0].index());
    assert_eq!(rules.path_from_ship(t[0], 1, None), Some(vec![t[0], t[3]]), "a system adjacent to the active system");
}

#[test]
fn barred_transit_and_passing_ship_types_change_what_may_be_passed() {
    let (t, _, mut rules) = plain_row();
    rules.barred_transit.insert(t[1].index());
    assert!(!rules.can_reach_ship(t[0], 3, None));
    let (t, _, mut rules) = plain_row();
    rules.enemy_ships.insert(t[1].index());
    rules.passing_ship_types.insert(7);
    let ty = Some(ti4_engine_core::generated::Unit::ALL[7]);
    assert!(rules.can_reach_ship(t[0], 3, ty), "a unit type that moves through enemy ships");
    assert!(!rules.can_reach_ship(t[0], 3, Some(ti4_engine_core::generated::Unit::ALL[8])));
    let (t, _, mut rules) = plain_row();
    rules.enemy_ships.insert(t[1].index());
    rules.unladen_pass = true;
    assert!(rules.can_reach_ship(t[0], 3, None), "an unladen ship passes at the second try");
}

#[test]
fn own_passage_types_pass_enemies_only_through_systems_holding_their_own_units_and_gain_range_there() {
    let (t, _, mut rules) = plain_row();
    rules.enemy_ships.insert(t[1].index());
    rules.own_passage_types.insert(9);
    let ty = Some(ti4_engine_core::generated::Unit::ALL[9]);
    assert!(!rules.can_reach_ship(t[0], 3, ty));
    rules.own_unit_systems.insert(t[1].index());
    assert!(rules.can_reach_ship(t[0], 3, ty));
    assert!(rules.can_reach_ship(t[0], 2, ty), "one extra step earned in a system with a unit of ours");
}

#[test]
fn all_routes_lists_every_simple_route_by_length_then_ids() {
    let p = plain_systems();
    // A diamond: A - B - D and A - C - D, active D; B and C are neighbours of both.
    let mut g = CoreGalaxy::new(&NO_LANES);
    let (a, b, c, d) = (p[0], p[1], p[2], p[3]);
    g.place(a, Hex::new(0, 0)).unwrap();
    g.place(b, Hex::new(1, 0)).unwrap();
    g.place(c, Hex::new(0, 1)).unwrap();
    g.place(d, Hex::new(1, 1)).unwrap();
    let adj = g.adjacency();
    assert!(adj.of(a).contains(b.index()) && adj.of(a).contains(c.index()) && adj.of(b).contains(d.index()) && adj.of(c).contains(d.index()));
    let rules = CoreRules::with_board(Arc::new(adj), ALL_SOURCES, d, SysSet::EMPTY, SysSet::EMPTY);
    let routes = rules.all_routes_from_ship(a, 2, None);
    assert!(routes.iter().all(|r| r.first() == Some(&a) && r.last() == Some(&d)));
    assert!(routes.windows(2).all(|w| (w[0].len(), &w[0]) <= (w[1].len(), &w[1])), "sorted by length then ids");
    assert!(routes.contains(&vec![a, b, d]) && routes.contains(&vec![a, c, d]));
    assert!(rules.all_routes_from_ship(a, 0, None).is_empty());
    let mut tokened = rules.clone();
    tokened.own_command_tokens.insert(a.index());
    assert!(tokened.all_routes_from_ship(a, 2, None).is_empty(), "may not depart");
    // The first route of the search is a shortest one.
    let path = rules.path_from_ship(a, 2, None).unwrap();
    assert_eq!(path.len(), routes.iter().map(Vec::len).min().unwrap());
}

#[test]
fn a_hyperlane_tile_is_never_a_destination() {
    let hyper = System::ALL.into_iter().find(|s| s.data().hyperlane).expect("a hyperlane tile");
    let p = plain_systems();
    let mut g = CoreGalaxy::new(&NO_LANES);
    g.place(p[0], Hex::new(0, 0)).unwrap();
    g.place(hyper, Hex::new(1, 0)).unwrap();
    let rules = CoreRules::with_board(Arc::new(g.adjacency()), ALL_SOURCES, hyper, SysSet::EMPTY, SysSet::EMPTY);
    assert!(!rules.can_enter(hyper));
    let _ = UnitSet::EMPTY;
}
