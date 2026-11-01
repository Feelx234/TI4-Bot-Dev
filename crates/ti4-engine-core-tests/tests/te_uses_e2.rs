//! Lane E2: the systems the Thunder's Edge Construction primary offers a use of PRODUCTION in (`strategy_cards::te4_construction`: the systems of
//! `state.board.keys()` whose capacity is above zero). First gap of a sweep game (`TurnGap::StrategyAbility`): a seat that played War Machine has
//! capacity in EVERY system, and the core listed all 231 cells of its board instead of the ones the string holds an entry for.

use ti4_engine_core::driver::windows::strategic::facts::facts_at;
use ti4_engine_core::state::{RoundKind, SysFlags};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

#[test]
fn a_bonus_that_is_the_same_in_every_system_lists_only_the_systems_the_string_holds_an_entry_for() {
    covers!("driver::windows::strategic::facts::te_uses");
    let p = plain_systems();
    let roster: &'static ti4_engine_core::query::tech::UnitRoster = Box::leak(Box::default());
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).map_from(&[p[0], p[1], p[2]]).build();
    // War Machine played in this activation: +5 PRODUCTION in every system (`production::war_machine_bonus`).
    let seq = s.meta().activation_seq;
    s.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::WarMachine, seq);
    // The string holds entries for the systems of its board; the core flags them (two here).
    for system in &p[..2] {
        s.board_mut().sys_mut(*system).flags.set(SysFlags::TOUCHED, true);
    }
    let touched: Vec<System> = s.board().systems().filter(|(_, c)| c.flags.has(SysFlags::TOUCHED)).map(|(system, _)| system).collect();
    assert_eq!(touched.len(), 2);
    let facts = facts_at(&s, &k, &Default::default(), Some(roster), Seat(0), StrategyCard::ConstructionThundersEdge);
    let uses = facts.te_uses.expect("the primary has its facts: the uses fit");
    assert_eq!(uses.uses().count(), touched.len(), "one use per system the string holds an entry for");
    assert!(uses.uses().all(|u| touched.contains(&u.produce.system)));
}

#[test]
fn a_late_board_lists_every_touched_system_even_beyond_a_dozen() {
    covers!("driver::windows::strategic::facts::te_uses");
    let p = plain_systems();
    let roster: &'static ti4_engine_core::query::tech::UnitRoster = Box::leak(Box::default());
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).map_ring(19).build();
    let seq = s.meta().activation_seq;
    s.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::WarMachine, seq);
    for system in &p[..19] {
        s.board_mut().sys_mut(*system).flags.set(SysFlags::TOUCHED, true);
    }
    let facts = facts_at(&s, &k, &Default::default(), Some(roster), Seat(0), StrategyCard::ConstructionThundersEdge);
    let uses = facts.te_uses.expect("nineteen systems fit: the string offers all of them");
    assert_eq!(uses.uses().count(), 19);
}

#[test]
fn only_a_touched_system_that_can_produce_is_a_use_and_a_production_of_one_is_enough() {
    covers!("driver::windows::strategic::facts::te_uses");
    let p = plain_systems();
    let roster: &'static ti4_engine_core::query::tech::UnitRoster = Box::leak(Box::default());
    let planet = p[0].data().planets[0];
    // The Arborec's Letani Warrior I (PRODUCTION 1) in the first system; the second is touched and empty; the third produces but is not touched.
    let other = p[2].data().planets[0];
    let (mut s, k) = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat])
        .phase(Phase::Action)
        .active(Some(0))
        .map_from(&[p[0], p[1], p[2]])
        .control(planet, 0)
        .unit(p[0], Some(planet), Unit::LetaniWarriorI, 0, 1)
        .unit(p[2], Some(other), Unit::LetaniWarriorI, 0, 1)
        .build();
    for system in &p[..2] {
        s.board_mut().sys_mut(*system).flags.set(SysFlags::TOUCHED, true);
    }
    s.board_mut().sys_mut(p[2]).flags.set(SysFlags::TOUCHED, false);
    let facts = facts_at(&s, &k, &Default::default(), Some(roster), Seat(0), StrategyCard::ConstructionThundersEdge);
    let uses = facts.te_uses.expect("the facts fit");
    let systems: Vec<System> = uses.uses().map(|u| u.produce.system).collect();
    assert_eq!(systems, vec![p[0]], "the touched system with a producer: not the empty one, not the untouched one");
    assert_eq!(uses.uses().next().map(|u| u.produce.limit), Some(1));
}
