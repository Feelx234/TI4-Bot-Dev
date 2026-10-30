//! D4-2b: `rules::leaders` (status moves, the effects of leaders that ask nothing, the end-of-round purge).

use ti4_engine_core::generated::{Leader, LeaderType, Planet, Unit};
use ti4_engine_core::query::board;
use ti4_engine_core::rules::leaders as l;
use ti4_engine_core::state::{LeaderStatus as St, PlanetFlags, SysFlags};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{first_planet, plain_systems};

fn table() -> Scenario {
    Scenario::new(&[Faction::TheBaronyOfLetnev, Faction::TheEmiratesOfHacan, Faction::TheL1Z1XMindnet]).map_ring(7)
}

fn leaders_of(faction: Faction) -> Vec<Leader> {
    Leader::ALL.into_iter().filter(|x| x.data().faction.factions() == [faction]).collect()
}

fn give(s: &mut CoreState, seat: u8, leader: Leader, st: St) {
    s.players_mut().by_seat_mut(Seat(seat)).unwrap().leaders.status.insert(leader, st);
}

fn status(s: &CoreState, seat: u8, leader: Leader) -> Option<St> {
    l::status(s, Seat(seat), leader)
}

#[test]
fn leader_kinds_and_windows_come_from_the_printed_data() {
    assert_eq!(l::kind(Leader::DarktalonTreilla), LeaderType::Hero);
    let agent = Leader::ALL.into_iter().find(|x| x.data().kind == LeaderType::Agent).unwrap();
    assert_eq!(l::kind(agent), LeaderType::Agent);
    for x in Leader::ALL {
        assert_eq!(l::uses_the_action(x), x.data().window == ti4_engine_core::generated::LeaderWindow::Action);
        assert_eq!(l::is_action_window(x), matches!(x.data().window, ti4_engine_core::generated::LeaderWindow::Action | ti4_engine_core::generated::LeaderWindow::ActionPhase));
        if l::uses_the_action(x) {
            assert!(l::is_action_window(x));
        }
    }
}

#[test]
fn of_kind_lists_a_seats_leaders_of_that_kind_in_id_order() {
    let ls = leaders_of(Faction::TheBaronyOfLetnev);
    let mut s = table().state();
    for x in &ls {
        give(&mut s, 0, *x, St::Locked);
    }
    let heroes = l::of_kind(&s, Seat(0), LeaderType::Hero);
    assert!(heroes.as_slice().iter().all(|x| l::kind(*x) == LeaderType::Hero) && !heroes.is_empty());
    assert!(heroes.as_slice().windows(2).all(|w| w[0] < w[1]));
    assert_eq!(l::of_kind(&s, Seat(0), LeaderType::Agent).len() + l::of_kind(&s, Seat(0), LeaderType::Commander).len() + heroes.len(), ls.iter().filter(|x| matches!(l::kind(**x), LeaderType::Agent | LeaderType::Commander | LeaderType::Hero)).count());
    assert!(l::of_kind(&s, Seat(1), LeaderType::Hero).is_empty());
    assert!(l::of_kind(&s, Seat(9), LeaderType::Hero).is_empty());
}

#[test]
fn only_a_readied_agent_and_an_unlocked_hero_are_usable_now() {
    let ls = leaders_of(Faction::TheBaronyOfLetnev);
    let agent = *ls.iter().find(|x| l::kind(**x) == LeaderType::Agent).unwrap();
    let hero = *ls.iter().find(|x| l::kind(**x) == LeaderType::Hero).unwrap();
    let commander = *ls.iter().find(|x| l::kind(**x) == LeaderType::Commander).unwrap();
    for (a, h, expect) in [(St::Readied, St::Unlocked, 2), (St::Exhausted, St::Unlocked, 1), (St::Readied, St::Locked, 1), (St::Locked, St::Purged, 0), (St::Unlocked, St::Readied, 0)] {
        let mut s = table().state();
        give(&mut s, 0, agent, a);
        give(&mut s, 0, hero, h);
        give(&mut s, 0, commander, St::Unlocked);
        assert_eq!(l::usable(&s, Seat(0)).len(), expect, "agent {a:?}, hero {h:?}");
    }
}

#[test]
fn status_moves_follow_ready_exhaust_purge_and_unlock() {
    let hero = Leader::DarktalonTreilla;
    let agent = leaders_of(Faction::TheBaronyOfLetnev).into_iter().find(|x| l::kind(*x) == LeaderType::Agent).unwrap();
    let mut s = table().state();
    give(&mut s, 0, agent, St::Readied);
    assert!(!l::ready(&mut s, Seat(0), agent), "already ready");
    assert!(l::exhaust(&mut s, Seat(0), agent));
    assert_eq!(status(&s, 0, agent), Some(St::Exhausted));
    assert!(!l::exhaust(&mut s, Seat(0), agent), "already exhausted");
    assert!(l::ready(&mut s, Seat(0), agent));
    assert!(!l::exhaust(&mut s, Seat(1), agent) && !l::ready(&mut s, Seat(1), agent), "not held by that seat");
    assert!(!l::exhaust(&mut s, Seat(9), agent));
    assert!(!l::purge(&mut s, Seat(0), hero), "a leader the seat does not hold");
    l::unlock(&mut s, Seat(0), &[hero]);
    assert_eq!(status(&s, 0, hero), Some(St::Unlocked), "unlock also gives a leader the seat lacks");
    assert!(l::purge(&mut s, Seat(0), hero));
    assert_eq!(status(&s, 0, hero), Some(St::Purged));
    assert!(l::purge(&mut s, Seat(0), hero), "purging again is still a purge");
    l::unlock(&mut s, Seat(9), &[hero]);
}

#[test]
fn ready_all_readies_only_the_exhausted() {
    let ls = leaders_of(Faction::TheBaronyOfLetnev);
    let mut s = table().state();
    give(&mut s, 0, ls[0], St::Exhausted);
    give(&mut s, 0, ls[1], St::Locked);
    give(&mut s, 0, ls[2], St::Exhausted);
    let readied = l::ready_all(&mut s, Seat(0));
    assert_eq!(readied.as_slice(), &[ls[0], ls[2]]);
    assert_eq!(status(&s, 0, ls[0]), Some(St::Readied));
    assert_eq!(status(&s, 0, ls[1]), Some(St::Locked));
    assert!(l::ready_all(&mut s, Seat(0)).is_empty());
    assert!(l::ready_all(&mut s, Seat(9)).is_empty());
}

#[test]
fn scored_objectives_count_public_and_secret_of_one_seat() {
    let mut s = table().state();
    assert_eq!(l::scored_count(&s, Seat(0)), 0);
    s.objectives_mut().scored[0].public.insert(ti4_engine_core::generated::PublicObjective::ALL[0]);
    s.objectives_mut().scored[0].secret.insert(ti4_engine_core::generated::SecretObjective::ALL[0]);
    s.objectives_mut().scored[1].public.insert(ti4_engine_core::generated::PublicObjective::ALL[1]);
    assert_eq!(l::scored_count(&s, Seat(0)), 2);
    assert_eq!(l::scored_count(&s, Seat(1)), 1);
    assert_eq!(l::HERO_OBJECTIVES, 3);
}

#[test]
fn xxekir_groms_unlock_combines_planet_values() {
    let mut s = table().state();
    assert!(!l::combines_planet_values(&s, Seat(0)));
    give(&mut s, 0, Leader::XxekirGrom, St::Locked);
    assert!(!l::combines_planet_values(&s, Seat(0)));
    give(&mut s, 0, Leader::XxekirGrom, St::Unlocked);
    assert!(l::combines_planet_values(&s, Seat(0)) && !l::combines_planet_values(&s, Seat(1)));
}

#[test]
fn the_letnev_hero_makes_the_fleet_unlimited_for_one_round_and_the_end_of_the_round_purges_it() {
    let hero = Leader::DarktalonTreilla;
    let mut s = table().round(2).state();
    give(&mut s, 0, hero, St::Unlocked);
    assert!(l::activate_letnev_hero(&mut s, Seat(0)));
    assert!(ti4_engine_core::query::fleet::is_unlimited(&s, Seat(0)));
    assert!(!l::activate_letnev_hero(&mut s, Seat(0)), "already active this round");
    assert!(!l::activate_letnev_hero(&mut s, Seat(9)));
    let purged = l::end_of_round(&mut s);
    assert_eq!(purged.as_slice(), &[Seat(0)]);
    assert_eq!(status(&s, 0, hero), Some(St::Purged));
    assert!(!ti4_engine_core::query::fleet::is_unlimited(&s, Seat(0)), "the round is over");
    assert!(l::end_of_round(&mut s).is_empty(), "nothing the next time");
    // A hero that was never used is not purged; one that was used but is not unlocked is not purged either.
    let mut idle = table().round(2).state();
    give(&mut idle, 0, hero, St::Unlocked);
    assert!(l::end_of_round(&mut idle).is_empty());
    assert_eq!(status(&idle, 0, hero), Some(St::Unlocked));
}

#[test]
fn the_hacan_hero_marks_the_production_it_frees_and_the_sol_hero_clears_our_tokens() {
    let systems = plain_systems();
    let mut s = table().command_token(systems[1], 0).command_token(systems[2], 0).command_token(systems[2], 1).state();
    s.meta_mut().production_seq = 4;
    assert!(l::activate_hacan_hero(&mut s, Seat(1)));
    assert!(s.players().by_seat(Seat(1)).unwrap().production_timers.free_production_use.is(4));
    assert!(!l::activate_hacan_hero(&mut s, Seat(9)));
    assert!(l::sol_hero(&mut s, Seat(0)));
    assert!(!board::has_token(&s, systems[1], Seat(0)) && !board::has_token(&s, systems[2], Seat(0)));
    assert!(board::has_token(&s, systems[2], Seat(1)), "another seat's token stays");
}

#[test]
fn an_agent_that_grants_an_extra_die_records_the_round_and_the_unit() {
    let mut s = table().state();
    s.meta_mut().combat_round_seq = 7;
    assert!(l::extra_die_agent(&mut s, Seat(0), Unit::DreadnoughtI));
    let t = s.players().by_seat(Seat(0)).unwrap().combat_timers;
    assert!(t.extra_die_round.is(7));
    assert_eq!(t.extra_die_unit, Some(Unit::DreadnoughtI));
    assert!(!l::extra_die_agent(&mut s, Seat(9), Unit::DreadnoughtI));
}

#[test]
fn the_l1z1x_agent_turns_one_infantry_on_a_planet_with_an_entry_into_a_mech() {
    let (system, planet) = first_planet();
    let mut s = table().unit(system, Some(planet), Unit::InfantryI, 2, 2).unit(system, Some(planet), Unit::InfantryI, 0, 1).state();
    assert!(s.board().planet(planet).flags.has(PlanetFlags::UNITS_ENTRY));
    assert!(l::l1z1x_agent(&mut s, ALL_SOURCES, Seat(2), system, Unit::FactionlessMech));
    let kinds: Vec<(Unit, u8)> = s.board().planet(planet).units.as_slice().iter().map(|p| (p.kind(), p.owner().0)).collect();
    assert_eq!(kinds.iter().filter(|(k, o)| *k == Unit::FactionlessMech && *o == 2).count(), 1, "exactly one became a mech");
    assert_eq!(kinds.iter().filter(|(k, o)| *k == Unit::InfantryI && *o == 2).count(), 1);
    assert_eq!(kinds.iter().filter(|(k, o)| *k == Unit::InfantryI && *o == 0).count(), 1, "another seat's infantry untouched");
    assert!(s.board().sys(system).flags.has(SysFlags::TOUCHED));
    let mut none = table().unit(system, Some(planet), Unit::InfantryI, 0, 1).state();
    assert!(!l::l1z1x_agent(&mut none, ALL_SOURCES, Seat(2), system, Unit::FactionlessMech), "no infantry of the target");
    let _ = Planet::COUNT;
}

#[test]
fn the_sol_commander_places_units_on_the_planet() {
    let (system, planet) = first_planet();
    let mut s = table().state();
    assert!(l::sol_commander(&mut s, Seat(0), system, planet, Unit::InfantryI, 2));
    assert_eq!(s.board().planet(planet).units.len(), 2);
    assert!(s.board().planet(planet).units.as_slice().iter().all(|p| p.owner() == Seat(0) && p.kind() == Unit::InfantryI));
}
