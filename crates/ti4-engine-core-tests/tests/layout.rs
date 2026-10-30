//! D4-1 low layer: `bits`, `hex`, `board`, `galaxy` (adjacency, wormholes, the dense grid).

use proptest::prelude::*;
use ti4_engine_core::bits::{SysSet, UnitSet};
use ti4_engine_core::board::CoreBoard;
use ti4_engine_core::galaxy::{CoreGalaxy, PlaceError};
use ti4_engine_core::generated::System;
use ti4_engine_core::hex::{DIRECTIONS, EDGE_DIRS, GRID_RADIUS, Grid, Hex};
use ti4_engine_core::ids::{Seat, system_at};
use ti4_engine_core::tables::{SYSTEM_COUNT, WH_ALPHA, WH_ALPHA_BETA, WH_BETA, LaneTable};
use ti4_engine_core_tests::scenario::{plain_systems, spiral};

static NO_LANES: [Option<&[(u8, u8)]>; SYSTEM_COUNT] = [None; SYSTEM_COUNT];
fn lanes() -> &'static LaneTable {
    &NO_LANES
}

// ---- bits -------------------------------------------------------------------------------------

#[test]
fn sys_set_covers_every_system_and_iterates_ascending() {
    let mut s = SysSet::EMPTY;
    assert!(s.is_empty());
    assert!(s.insert(SYSTEM_COUNT - 1), "the last system fits");
    assert!(s.insert(0) && s.insert(64) && s.insert(63) && !s.insert(64));
    assert_eq!(s.iter().collect::<Vec<_>>(), vec![0, 63, 64, SYSTEM_COUNT - 1]);
    assert_eq!(s.len(), 4);
    assert!(s.contains(63) && !s.contains(62));
    s.remove(63);
    assert!(!s.contains(63) && s.len() == 3);
    let mut t = SysSet::EMPTY;
    t.insert(64);
    t.insert(100);
    assert!(s.intersects(&t));
    let mut u = s;
    u.intersect_with(&t);
    assert_eq!(u.iter().collect::<Vec<_>>(), vec![64]);
    let mut v = s;
    v.union_with(&t);
    assert_eq!(v.len(), 3 + 1, "{{0, 64, last}} and {{64, 100}}");
    assert!(!u.intersects(&SysSet::EMPTY));
    assert_eq!(format!("{u:?}"), "{64}");
}

#[test]
fn unit_set_is_a_bitset_over_unit_types() {
    let mut s = UnitSet::EMPTY;
    s.insert(ti4_engine_core::tables::UNIT_COUNT - 1);
    assert_eq!(s.len(), 1);
}

// ---- hex --------------------------------------------------------------------------------------

#[test]
fn the_six_directions_are_the_six_neighbours_and_edges_are_a_rotation_of_them() {
    let h = Hex::new(2, -1);
    let n = h.neighbours();
    for (i, (dq, dr)) in DIRECTIONS.iter().enumerate() {
        assert_eq!(n[i], Hex::new(2 + dq, -1 + dr));
    }
    let mut edges: Vec<Hex> = (0..6u8).map(|e| h.edge(e)).collect();
    let mut dirs = n.to_vec();
    edges.sort();
    dirs.sort();
    assert_eq!(edges, dirs, "the edges name the same six hexes");
    assert_eq!(EDGE_DIRS.len(), 6);
    for e in 0..6u8 {
        // The opposite edge of the neighbour on edge e is the starting hex.
        assert_eq!(h.edge(e).edge((e + 3) % 6), h, "edge {e} and its opposite");
    }
}

#[test]
fn the_grid_places_inside_its_radius_and_refuses_outside() {
    let mut g = Grid::default();
    let a = system_at(3);
    assert!(g.get(Hex::new(0, 0)).is_none());
    assert!(g.set(Hex::new(0, 0), Some(a)));
    assert!(g.set(Hex::new(GRID_RADIUS, 0), Some(a)));
    assert!(g.set(Hex::new(-GRID_RADIUS, GRID_RADIUS), Some(a)));
    assert!(!g.set(Hex::new(GRID_RADIUS + 1, 0), Some(a)));
    assert!(!g.set(Hex::new(0, -GRID_RADIUS - 1), Some(a)));
    assert_eq!(g.get(Hex::new(0, 0)), Some(a));
    assert_eq!(g.get(Hex::new(GRID_RADIUS + 5, 0)), None);
    assert!(g.set(Hex::new(0, 0), None));
    assert_eq!(g.get(Hex::new(0, 0)), None);
    // Distinct hexes are distinct cells (the index arithmetic does not alias).
    assert!(g.set(Hex::new(1, 0), Some(system_at(1))) && g.set(Hex::new(0, 1), Some(system_at(2))));
    assert_eq!((g.get(Hex::new(1, 0)), g.get(Hex::new(0, 1))), (Some(system_at(1)), Some(system_at(2))));
}

// ---- galaxy -----------------------------------------------------------------------------------

fn ring_galaxy(n: usize) -> (CoreGalaxy, Vec<System>) {
    let systems = plain_systems();
    let mut g = CoreGalaxy::new(lanes());
    for (s, h) in systems.iter().take(n).zip(spiral(n)) {
        g.place(*s, h).unwrap();
    }
    g.refresh_lane_links();
    (g, systems[..n].to_vec())
}

#[test]
fn a_ring_of_seven_makes_the_centre_touch_six_and_each_outer_touch_three() {
    let (g, s) = ring_galaxy(7);
    assert_eq!(g.adjacent(s[0]).len(), 6);
    for outer in &s[1..] {
        let adj = g.adjacent(*outer);
        assert_eq!(adj.len(), 3, "centre and the two neighbours on the ring");
        assert!(adj.contains(s[0].index()));
    }
}

#[test]
fn placing_twice_or_off_the_grid_is_refused_and_leaves_the_board_unchanged() {
    let mut g = CoreGalaxy::new(lanes());
    let s = plain_systems();
    g.place(s[0], Hex::new(0, 0)).unwrap();
    assert_eq!(g.place(s[0], Hex::new(1, 0)), Err(PlaceError::Duplicate(s[0])));
    assert_eq!(g.place(s[1], Hex::new(40, 0)), Err(PlaceError::OffGrid(s[1])));
    assert_eq!(g.coord_of(s[1]), None, "a refused placement leaves no coordinate");
    assert_eq!(g.coord_of(s[0]), Some(Hex::new(0, 0)));
    assert_eq!(g.system_at(Hex::new(0, 0)), Some(s[0]));
    assert!(g.in_play().contains(s[0].index()) && !g.in_play().contains(s[1].index()));
    g.place_off_map(s[2]).unwrap();
    assert_eq!(g.place_off_map(s[2]), Ok(()), "off the map a second placement is idempotent (only a system on the grid is refused)");
    assert_eq!(g.place_off_map(s[0]), Err(PlaceError::Duplicate(s[0])), "already on the grid");
    assert!(g.in_play().contains(s[2].index()));
    assert_eq!(g.coord_of(s[2]), None);
}

/// Three outer tiles of a ring of seven that do not touch each other by hex (outer tiles touch the centre and two outer neighbours,
/// so such a triple exists: every second tile of the ring).
fn far_triple(g: &CoreGalaxy, s: &[System]) -> (System, System, System) {
    for &x in &s[1..] {
        for &y in &s[1..] {
            for &z in &s[1..] {
                if x != y && y != z && x != z && ![(x, y), (y, z), (x, z)].iter().any(|(p, q)| g.adjacent(*p).contains(q.index())) {
                    return (x, y, z);
                }
            }
        }
    }
    panic!("a ring of seven has three pairwise far outer tiles");
}

#[test]
fn wormholes_join_systems_that_share_a_kind_and_only_those() {
    let (mut g, s) = ring_galaxy(7);
    let (a, b, c) = far_triple(&g, &s);
    g.set_wormholes(a, WH_ALPHA);
    g.set_wormholes(b, WH_ALPHA);
    g.set_wormholes(c, WH_BETA);
    assert!(g.adjacent(a).contains(b.index()) && g.adjacent(b).contains(a.index()), "alpha to alpha, both ways");
    assert!(!g.adjacent(a).contains(c.index()) && !g.adjacent(c).contains(a.index()), "beta does not join alpha");
    assert_eq!(g.wormhole_kinds(a), WH_ALPHA);
    let holders = g.wormhole_systems();
    assert!(holders.contains(a.index()) && holders.contains(b.index()) && holders.contains(c.index()));
    g.add_token_wormholes(a, WH_BETA);
    assert_eq!(g.wormhole_kinds(a), WH_ALPHA_BETA);
    assert!(g.adjacent(a).contains(c.index()) && g.adjacent(a).contains(b.index()), "now a beta holder too");
    g.suppress_wormholes_at(a);
    assert_eq!(g.wormhole_kinds(a), 0);
    assert!(!g.adjacent(b).contains(a.index()), "a suppressed system is no partner any more");
    g.set_wormholes(b, WH_ALPHA);
    g.retain_wormhole_kinds(WH_BETA);
    assert_eq!(g.wormhole_kinds(b), 0, "alpha is gone everywhere");
    assert_eq!(g.wormhole_kinds(c), WH_BETA);
}

#[test]
fn wormholes_off_removes_wormhole_adjacency_but_not_hex_adjacency() {
    let (mut g, s) = ring_galaxy(7);
    let (a, b, _) = far_triple(&g, &s);
    g.set_wormholes(a, WH_ALPHA);
    g.set_wormholes(b, WH_ALPHA);
    assert!(g.adjacent(a).contains(b.index()));
    g.wormholes_off = true;
    assert!(!g.adjacent(a).contains(b.index()));
    assert!(g.adjacent(a).contains(s[0].index()), "hex adjacency stays");
}

#[test]
fn all_linked_joins_alpha_and_beta_holders_with_each_other() {
    let (mut g, s) = ring_galaxy(7);
    let (a, b, _) = far_triple(&g, &s);
    g.set_wormholes(a, WH_ALPHA);
    g.set_wormholes(b, WH_BETA);
    assert!(!g.adjacent(a).contains(b.index()));
    g.wormholes_all_linked = true;
    assert!(g.adjacent(a).contains(b.index()), "an alpha now touches a beta");
    assert!(g.adjacent(b).contains(a.index()));
}

#[test]
fn star_links_join_only_systems_that_hold_both_kinds() {
    let (mut g, s) = ring_galaxy(7);
    let (a, b, c) = far_triple(&g, &s);
    g.set_wormholes(a, WH_ALPHA_BETA);
    g.set_wormholes(b, WH_ALPHA_BETA);
    g.set_wormholes(c, WH_ALPHA);
    assert!(g.adjacent(a).contains(c.index()), "without star links an alpha joins any alpha holder");
    g.wormhole_star_links = true;
    assert!(g.adjacent(a).contains(b.index()), "both kinds to both kinds");
    assert!(!g.adjacent(a).contains(c.index()), "a lone alpha holder is left out");
}

#[test]
fn nexus_sovereignty_suppresses_the_nexus_wormholes_unless_asked_unsuppressed() {
    let nexus = System::ALL.iter().copied().find(|s| ti4_engine_core::tables::is_nexus(*s)).expect("a nexus tile");
    let mut g = CoreGalaxy::new(lanes());
    g.place(nexus, Hex::new(0, 0)).unwrap();
    g.set_wormholes(nexus, WH_ALPHA_BETA);
    assert_eq!(g.wormhole_kinds(nexus), WH_ALPHA_BETA);
    g.nexus_wormholes_off = true;
    assert_eq!(g.wormhole_kinds(nexus), 0);
    assert_eq!(g.wormhole_kinds_unsuppressed(nexus), WH_ALPHA_BETA);
}

#[test]
fn extra_links_are_added_both_ways_as_listed() {
    let (mut g, s) = ring_galaxy(7);
    let (x, y, _) = far_triple(&g, &s);
    let mut set = SysSet::EMPTY;
    set.insert(y.index());
    g.extra_links.push((x, set));
    assert!(g.adjacent(x).contains(y.index()));
    assert!(!g.adjacent(y).contains(x.index()), "one direction as listed; the adapter lists both");
}

#[test]
fn a_system_is_never_adjacent_to_itself() {
    let (mut g, s) = ring_galaxy(7);
    g.set_wormholes(s[1], WH_ALPHA);
    g.add_token_wormholes(s[1], WH_BETA);
    for x in &s {
        assert!(!g.adjacent(*x).contains(x.index()));
    }
}

proptest! {
    #![proptest_config(ti4_engine_core_tests::generators::config())]
    #[test]
    fn adjacency_is_symmetric_and_irreflexive_on_any_placement(picks in proptest::collection::vec((0usize..60, 0i32..25, 0i32..25, 0u8..4), 1..40)) {
        let mut g = CoreGalaxy::new(lanes());
        for (si, q, r, wh) in picks {
            let s = system_at(si);
            let hex = Hex::new(q - 12, r - 12);
            if g.system_at(hex).is_some() {
                continue; // see `two_systems_on_one_hex_shadow_each_other`
            }
            if g.place(s, hex).is_ok() && wh & 1 != 0 {
                g.add_token_wormholes(s, if wh & 2 != 0 { WH_ALPHA } else { WH_BETA });
            }
        }
        let adj = g.adjacency();
        for i in 0..SYSTEM_COUNT {
            let a = system_at(i);
            prop_assert!(!adj.of(a).contains(i), "{a:?} touches itself");
            for j in adj.of(a).iter() {
                prop_assert!(adj.of(system_at(j)).contains(i), "{a:?} touches {j} but not back");
            }
            prop_assert_eq!(*adj.of(a), g.adjacent(a), "the precomputed adjacency equals the on-demand one");
        }
    }
}

// ---- board (occupancy summary) ----------------------------------------------------------------

#[test]
fn the_occupancy_summary_answers_enemy_ships_command_tokens_and_unit_systems_per_seat() {
    let mut b = CoreBoard::new();
    for i in [3usize, 5, 9] {
        b.systems.insert(i);
    }
    b.ship_owners[3] = Seat(1).bit();
    b.ship_owners[5] = Seat(0).bit();
    b.ship_owners[9] = Seat(0).bit() | Seat(2).bit();
    b.command_tokens[5] = Seat(0).bit();
    b.unit_owners[3] = Seat(1).bit() | Seat(0).bit();
    b.unit_owners[9] = Seat(2).bit();
    assert_eq!(b.enemy_ships(Seat(0)).iter().collect::<Vec<_>>(), vec![3, 9]);
    assert_eq!(b.enemy_ships(Seat(1)).iter().collect::<Vec<_>>(), vec![5, 9]);
    assert_eq!(b.own_command_tokens(Seat(0)).iter().collect::<Vec<_>>(), vec![5]);
    assert!(b.own_command_tokens(Seat(1)).is_empty());
    assert_eq!(b.unit_systems(Seat(0)).iter().collect::<Vec<_>>(), vec![3]);
    assert_eq!(b.unit_systems(Seat(2)).iter().collect::<Vec<_>>(), vec![9]);
    // A system that is not on the board is not reported even if a mask says so.
    b.ship_owners[7] = Seat(3).bit();
    assert_eq!(b.enemy_ships(Seat(0)).iter().collect::<Vec<_>>(), vec![3, 9]);
}

/// D4-found-01 (low; the string `Galaxy::placed` behaves the same and no legal game places two tiles on one hex): placing a second system on an
/// occupied hex succeeds and the grid keeps the later one, so the first system still has a coordinate but is not on the grid any more and the
/// adjacency stops being symmetric. This test pins the current behaviour; a refusal (`PlaceError::Occupied`) would be the fix.
#[test]
fn two_systems_on_one_hex_shadow_each_other() {
    let s = plain_systems();
    let mut g = CoreGalaxy::new(lanes());
    g.place(s[0], Hex::new(0, 0)).unwrap();
    g.place(s[1], Hex::new(1, 0)).unwrap();
    assert_eq!(g.place(s[2], Hex::new(0, 0)), Ok(()), "accepted although the hex is taken");
    assert_eq!(g.system_at(Hex::new(0, 0)), Some(s[2]), "the later one wins the grid");
    assert_eq!(g.coord_of(s[0]), Some(Hex::new(0, 0)), "the first keeps a coordinate");
    let adj = g.adjacency();
    assert!(adj.of(s[0]).contains(s[1].index()), "s0 still touches s1");
    assert!(!adj.of(s[1]).contains(s[0].index()), "but s1 no longer touches s0: asymmetric");
    assert!(adj.of(s[1]).contains(s[2].index()));
}
