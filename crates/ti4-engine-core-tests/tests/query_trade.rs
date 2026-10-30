//! D4-1 low layer: `query/trade.rs` (who is a neighbour, who may transact, the per-turn limits). Rules: players are neighbours when they have
//! units or planets in the same system or in adjacent systems (LRR 60.1); a pair transacts once per turn.

use std::cell::Cell;

use ti4_engine_core::galaxy::Adjacency;
use ti4_engine_core::query::trade::{self as q, Cx, Env, Limits};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

#[derive(Default)]
struct Rules {
    iihq: Vec<u8>,
    wide: Vec<u8>,
    reach: bool,
    exempt: bool,
    asked: Cell<u32>,
}

impl Env for Rules {
    fn has_iihq(&self, who: Seat) -> bool {
        self.asked.set(self.asked.get() + 1);
        self.iihq.contains(&who.0)
    }
    fn wide(&self, who: Seat) -> bool {
        self.wide.contains(&who.0)
    }
    fn transaction_reach(&self, _: Seat, _: Seat) -> bool {
        self.reach
    }
    fn exempt_from_limit(&self, _: Seat, _: Seat) -> bool {
        self.exempt
    }
    fn trades_action_cards(&self, _: Seat) -> bool {
        false
    }
    fn black_market(&self) -> bool {
        false
    }
}

fn four() -> Scenario {
    Scenario::lobby(4).map_ring(7)
}

fn adjacency(s: &CoreState) -> std::sync::Arc<Adjacency> {
    s.map().adjacency().expect("a map").clone()
}

/// Two outer tiles of the ring that touch each other, and one that touches neither of them.
fn pair_and_far(adj: &Adjacency, tiles: &[System]) -> (System, System, System) {
    let outer = &tiles[1..];
    for &x in outer {
        for &y in outer {
            if x != y && adj.of(x).contains(y.index()) {
                if let Some(&z) = outer.iter().find(|&&z| z != x && z != y && !adj.of(x).contains(z.index()) && !adj.of(y).contains(z.index())) {
                    return (x, y, z);
                }
            }
        }
    }
    panic!("a ring of seven has two touching outer tiles and one far from both");
}

fn cx<'a>(s: &'a CoreState, adj: &'a Adjacency, env: &'a Rules) -> Cx<'a> {
    Cx { core: s, adj, mecatol: plain_systems()[0], limits: Limits { per_action: 1, per_round: 2 }, env }
}

#[test]
fn players_with_units_in_one_system_or_in_touching_systems_are_neighbours_and_others_are_not() {
    let tiles = &plain_systems()[..7];
    let proto = four().state();
    let adj = adjacency(&proto);
    let (x, y, z) = pair_and_far(&adj, tiles);
    let s = four().unit(x, None, Unit::CruiserI, 0, 1).unit(y, None, Unit::CruiserI, 1, 1).unit(z, None, Unit::CruiserI, 2, 1).unit(x, None, Unit::DestroyerI, 3, 1).state();
    let env = Rules::default();
    let c = cx(&s, &adj, &env);
    assert!(q::are_neighbours(&c, Seat(0), Seat(1)), "touching systems");
    assert!(q::are_neighbours(&c, Seat(1), Seat(0)), "symmetric");
    assert!(q::are_neighbours(&c, Seat(0), Seat(3)), "the same system");
    assert!(!q::are_neighbours(&c, Seat(0), Seat(2)), "far apart");
    assert!(!q::are_neighbours(&c, Seat(0), Seat(0)), "never one's own neighbour");
    assert_eq!(q::neighbours(&c, Seat(0)).as_slice(), &[Seat(1), Seat(3)], "in seating order");
    assert_eq!(q::neighbours(&c, Seat(2)).as_slice(), &[] as &[Seat]);
    // A controlled planet is presence too.
    let planet = x.data().planets[0];
    let held = four().control(planet, 2).unit(x, None, Unit::CruiserI, 0, 1).state();
    let c2 = cx(&held, &adj, &env);
    assert!(q::are_neighbours(&c2, Seat(0), Seat(2)));
}

#[test]
fn the_imperial_hq_makes_its_holder_a_neighbour_of_whoever_is_near_mecatol() {
    let tiles = &plain_systems()[..7];
    let proto = four().state();
    let adj = adjacency(&proto);
    let (x, y, z) = pair_and_far(&adj, tiles);
    let _ = y;
    // Seat 2 sits far out; seat 0 sits at the centre (Mecatol's tile in this kit map).
    let s = four().unit(tiles[0], None, Unit::CruiserI, 0, 1).unit(z, None, Unit::CruiserI, 2, 1).unit(x, None, Unit::CruiserI, 1, 1).state();
    let plain = Rules::default();
    assert!(q::are_neighbours(&cx(&s, &adj, &plain), Seat(0), Seat(2)), "everything touches the centre of a ring");
    let _ = x;
    // The centre is adjacent to all of the ring: use a far tile for seat 3 and check the IIHQ path with a presence outside the ring.
    let iihq = Rules { iihq: vec![3], ..Rules::default() };
    let t3 = four().unit(z, None, Unit::CruiserI, 3, 1).unit(tiles[0], None, Unit::CruiserI, 1, 1).state();
    assert!(q::mecatol_neighbours(&cx(&t3, &adj, &iihq), Seat(3), Seat(1)), "the holder of the IIHQ and a player at Mecatol");
    assert!(!q::mecatol_neighbours(&cx(&t3, &adj, &plain), Seat(3), Seat(1)));
    assert!(q::near_mecatol(&cx(&t3, &adj, &plain), Seat(1)));
}

#[test]
fn partners_are_the_neighbours_or_everyone_for_a_seat_that_ignores_neighbours() {
    let tiles = &plain_systems()[..7];
    let proto = four().state();
    let adj = adjacency(&proto);
    let (x, y, z) = pair_and_far(&adj, tiles);
    let s = four().unit(x, None, Unit::CruiserI, 0, 1).unit(y, None, Unit::CruiserI, 1, 1).unit(z, None, Unit::CruiserI, 2, 1).state();
    let plain = Rules::default();
    assert_eq!(q::partners(&cx(&s, &adj, &plain), Seat(0)).as_slice(), &[Seat(1)]);
    let wide = Rules { wide: vec![0], ..Rules::default() };
    assert_eq!(q::partners(&cx(&s, &adj, &wide), Seat(0)).as_slice(), &[Seat(1), Seat(2), Seat(3)], "everyone else, in seating order");
}

#[test]
fn a_transaction_needs_reach_in_either_direction_and_wide_players_reach_everyone() {
    let tiles = &plain_systems()[..7];
    let proto = four().state();
    let adj = adjacency(&proto);
    let (x, y, z) = pair_and_far(&adj, tiles);
    let s = four().unit(x, None, Unit::CruiserI, 0, 1).unit(y, None, Unit::CruiserI, 1, 1).unit(z, None, Unit::CruiserI, 2, 1).state();
    let plain = Rules::default();
    let c = cx(&s, &adj, &plain);
    assert!(q::may_transact(&c, Seat(0), Seat(1)));
    assert!(!q::may_transact(&c, Seat(0), Seat(2)));
    let reach = Rules { reach: true, ..Rules::default() };
    assert!(q::may_transact(&cx(&s, &adj, &reach), Seat(0), Seat(2)), "an ability grants the reach");
    let wide_proposer = Rules { wide: vec![0], ..Rules::default() };
    assert!(q::may_transact(&cx(&s, &adj, &wide_proposer), Seat(0), Seat(2)));
    assert!(q::may_transact(&cx(&s, &adj, &wide_proposer), Seat(2), Seat(0)), "and anyone may trade with them");
    assert!(!q::may_transact(&cx(&s, &adj, &wide_proposer), Seat(0), Seat(9)), "but not with a stranger");
}

#[test]
fn a_pair_opens_once_a_turn_unless_exempt() {
    let proto = four().state();
    let adj = adjacency(&proto);
    let mut s = four().state();
    let plain = Rules::default();
    assert!(!q::transacted_with(&s, Seat(0), Seat(1)));
    assert!(q::may_open_again(&cx(&s, &adj, &plain), Seat(0), Seat(1)));
    s.trade_mut().transactions_this_turn.get_or_insert_with(Seat(0), Default::default).insert(Seat(1));
    assert!(q::transacted_with(&s, Seat(0), Seat(1)));
    assert!(!q::transacted_with(&s, Seat(1), Seat(0)), "recorded for the proposer");
    assert!(!q::may_open_again(&cx(&s, &adj, &plain), Seat(0), Seat(1)));
    let exempt = Rules { exempt: true, ..Rules::default() };
    assert!(q::may_open_again(&cx(&s, &adj, &exempt), Seat(0), Seat(1)));
}

#[test]
fn negotiations_are_capped_per_action_and_per_round() {
    let proto = four().state();
    let adj = adjacency(&proto);
    let mut s = four().state();
    let env = Rules::default();
    assert!(q::may_initiate_negotiation(&cx(&s, &adj, &env), Seat(0)));
    s.trade_mut().negotiations_this_action.get_or_insert_with(Seat(0), || 0);
    *s.trade_mut().negotiations_this_action.get_or_insert_with(Seat(0), || 0) = 1;
    assert!(!q::may_initiate_negotiation(&cx(&s, &adj, &env), Seat(0)), "one per action");
    assert!(q::may_initiate_negotiation(&cx(&s, &adj, &env), Seat(1)));
    let mut round = four().state();
    *round.trade_mut().negotiations_this_round.get_or_insert_with(Seat(1), || 0) = 2;
    assert!(!q::may_initiate_negotiation(&cx(&round, &adj, &env), Seat(1)), "two per round");
}
