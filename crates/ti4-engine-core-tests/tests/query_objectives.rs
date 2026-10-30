//! D4-1 low layer: `query/objectives.rs` (the position readers of the scoring area): scoring planets, map shape, presence, stages and points,
//! and the payment plans an objective's price is paid with.

use proptest::prelude::*;
use ti4_engine_core::generated::{Faction, Planet, PlanetTrait, PublicObjective, System};
use ti4_engine_core::query::objectives as q;
use ti4_engine_core::state::PlanetFlags;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn table() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat, Faction::TheYssarilTribes]).map_ring(7)
}

#[test]
fn points_and_stage_come_from_the_printed_points() {
    for o in PublicObjective::ALL {
        let p = q::points_of(o);
        assert_eq!(q::stage_of(o), match p { 1 => Some(1), 2 => Some(2), _ => None }, "{o:?}");
    }
    assert!(PublicObjective::ALL.iter().any(|o| q::stage_of(*o) == Some(1)) && PublicObjective::ALL.iter().any(|o| q::stage_of(*o) == Some(2)));
}

#[test]
fn a_space_station_is_a_planet_with_that_trait_and_is_never_a_scoring_planet() {
    let station = Planet::ALL.into_iter().find(|p| p.data().traits.contains(&PlanetTrait::Spacestation)).expect("a space station");
    assert!(q::is_space_station(station));
    assert!(!q::is_space_station(plain_systems()[0].data().planets[0]));
    let plain = plain_systems()[1].data().planets[0];
    let s = table().control(station, 0).control(plain, 0).state();
    let found = q::scoring_planets(&s, ALL_SOURCES, Seat(0));
    assert_eq!(found.as_slice(), &[plain], "the station is left out");
    assert!(q::scoring_planets(&s, ALL_SOURCES, Seat(1)).is_empty());
}

#[test]
fn coexisting_planets_score_for_their_coexisters_after_the_controlled_ones() {
    let systems = plain_systems();
    let (mine, shared) = (systems[1].data().planets[0], systems[2].data().planets[0]);
    let mut s = table().control(mine, 0).control(shared, 1).state();
    let cell = s.board_mut().planet_mut(shared);
    cell.coexist.insert(Seat(0));
    cell.flags.set(PlanetFlags::COEXIST_ENTRY, true);
    assert_eq!(q::scoring_planets(&s, ALL_SOURCES, Seat(0)).as_slice(), &[mine, shared]);
    assert_eq!(q::scoring_planets(&s, ALL_SOURCES, Seat(1)).as_slice(), &[shared]);
    let out = ti4_engine_core::tables::Scope(0);
    assert!(q::scoring_planets(&s, out, Seat(0)).is_empty(), "out of scope");
}

#[test]
fn the_edge_of_the_map_is_every_system_with_an_empty_neighbouring_hex() {
    let s = table().state();
    let edge = q::edge_systems(&s);
    let systems = plain_systems();
    assert_eq!(edge.len(), 6, "a ring of seven: the six outer tiles");
    assert!(!edge.contains(systems[0].index()), "the centre is surrounded");
    let bare = ti4_engine_core::state::CoreState::all_loaded();
    assert!(q::edge_systems(&bare).is_empty(), "no tile placed");
}

#[test]
fn mecatol_is_the_old_or_the_thunders_edge_tile_by_what_the_map_and_the_board_hold() {
    assert!(q::is_mecatol(System::MecatolRex18) && q::is_mecatol(System::MecatolRex112) && !q::is_mecatol(plain_systems()[0]));
    let s = table().state();
    assert_eq!(q::mecatol_in_galaxy(&s), System::MecatolRex18, "the new tile is not on the map");
    let te = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[System::MecatolRex112, plain_systems()[0]]).state();
    assert_eq!(q::mecatol_in_galaxy(&te), System::MecatolRex112);
    assert_eq!(q::mecatol_on(&s), System::MecatolRex18);
    let mut touched = s.clone();
    touched.board_mut().sys_mut(System::MecatolRex112).flags.set(ti4_engine_core::state::SysFlags::TOUCHED, true);
    assert_eq!(q::mecatol_on(&touched), System::MecatolRex112);
}

#[test]
fn presence_is_where_a_seat_has_a_unit_in_space_or_on_a_planet_or_controls_a_planet() {
    let systems = plain_systems();
    let (p1, p2) = (systems[1].data().planets[0], systems[2].data().planets[0]);
    let s = table().unit(systems[0], None, Unit::CruiserI, 0, 1).unit(systems[1], Some(p1), Unit::InfantryI, 0, 1).control(p2, 0).state();
    let here = q::presence(&s, Seat(0));
    assert_eq!(here.iter().collect::<Vec<_>>(), vec![systems[0].index(), systems[1].index(), systems[2].index()]);
    assert!(q::presence(&s, Seat(1)).is_empty());
}

#[test]
fn the_home_system_is_the_players_own_record_else_the_factions() {
    let mut s = table().state();
    assert_eq!(q::home_system_of(&s, Seat(0)), Faction::TheArborec.data().home_system);
    let other = plain_systems()[3];
    s.players_mut().by_seat_mut(Seat(0)).unwrap().home_system = Some(other);
    assert_eq!(q::home_system_of(&s, Seat(0)), Some(other));
    assert_eq!(q::home_system_of(&s, Seat(9)), None);
    assert_eq!(q::faction_of(&s, Seat(1)), Some(Faction::TheEmbersOfMuaat));
    assert_eq!(q::faction_of(&s, Seat(9)), None);
}

#[test]
fn the_token_pool_of_a_payment_is_strategy_plus_tactic_never_fleet() {
    let s = table().tokens(0, 3, 5, 2).state();
    assert_eq!(q::token_pool(&s, Seat(0)), 5);
    assert_eq!(q::token_pool(&s, Seat(9)), 0);
}

// ---- payment plans ----------------------------------------------------------------------------

fn pl(i: usize) -> Planet {
    Planet::ALL[i + 1]
}

fn plan_list(useful: &[(Planet, i64)], cost: i64, goods: i64, per_good: i64) -> Vec<(i32, Vec<Planet>)> {
    q::plans(useful, cost, goods, per_good).as_slice().iter().map(|p| (p.trade_goods, p.planets().to_vec())).collect()
}

#[test]
fn plans_list_the_ways_to_pay_without_overpaying_by_a_whole_planet() {
    let useful = [(pl(0), 3), (pl(1), 2), (pl(2), 1)];
    assert_eq!(plan_list(&useful, 3, 0, 1), vec![(0, vec![pl(0)]), (0, vec![pl(1), pl(2)])]);
    assert_eq!(plan_list(&useful, 3, 1, 1), vec![(0, vec![pl(0)]), (0, vec![pl(1), pl(2)]), (1, vec![pl(1)])], "a trade good can cover the last point");
    assert_eq!(plan_list(&useful, 3, 3, 1).first(), Some(&(0, vec![pl(0)])), "fewest goods first");
    assert!(plan_list(&useful, 3, 3, 1).contains(&(3, vec![])), "pure goods");
    assert_eq!(plan_list(&useful, 0, 0, 1), vec![(0, vec![])], "a price of nothing is paid with nothing");
    assert!(plan_list(&[], 2, 1, 1).is_empty(), "nothing to pay with");
    assert_eq!(plan_list(&[], 2, 2, 1), vec![(2, vec![])]);
    assert_eq!(plan_list(&[], 3, 3, 2), vec![(2, vec![])], "two points a good: round up");
}

proptest! {
    #![proptest_config(ti4_engine_core_tests::generators::config())]

    #[test]
    fn every_plan_pays_the_price_and_the_list_is_sorted_and_without_repeats(worths in proptest::collection::vec(1i64..5, 0..5), cost in 0i64..9, goods in 0i64..6, per_good in 1i64..3) {
        let mut sorted = worths.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        let useful: Vec<(Planet, i64)> = sorted.iter().enumerate().map(|(i, w)| (pl(i), *w)).collect();
        let plans = q::plans(&useful, cost, goods, per_good);
        let rows: Vec<(i32, Vec<Planet>)> = plans.as_slice().iter().map(|p| (p.trade_goods, p.planets().to_vec())).collect();
        for (tg, planets) in &rows {
            let from_planets: i64 = planets.iter().map(|p| useful.iter().find(|(x, _)| x == p).unwrap().1).sum();
            prop_assert!(from_planets + i64::from(*tg) * per_good >= cost, "plan {planets:?} + {tg} goods does not pay {cost}");
            prop_assert!(i64::from(*tg) <= (goods + per_good - 1) / per_good + 1);
        }
        for w in rows.windows(2) {
            prop_assert!((w[0].0, w[0].1.len(), &w[0].1) <= (w[1].0, w[1].1.len(), &w[1].1), "sorted by goods, then planets used");
            prop_assert!(w[0] != w[1], "no repeats");
        }
    }
}
