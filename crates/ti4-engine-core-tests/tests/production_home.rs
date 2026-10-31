//! Lent-bodies lane 9: `rules::production::home::HomeProduction`, a use of PRODUCTION at a seat's home system outside a tactical action (the
//! Warfare secondary): the Hacan hero's offer, the production, the effects at the end of the use and Self-Assembly Routines' offer.

use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt};
use ti4_engine_core::kernel::children::{HomeFacts, ProduceFacts};
use ti4_engine_core::rules::production::home::HomeProduction;
use ti4_engine_core::state::LeaderStatus;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn buildable(units: &[Unit]) -> [Option<Unit>; 9] {
    let mut out = [None; 9];
    for (i, u) in units.iter().enumerate() {
        out[i] = Some(*u);
    }
    out
}

fn facts(system: System, limit: i32, units: &[Unit], agent_rules: bool) -> HomeFacts {
    HomeFacts {
        produce: ProduceFacts { who: Seat(0), system, limit, max_unit_cost: None, small_exemptions: 0, discount: 0, fleet_limit: 8, armada: 0, flagship_free: false, buildable: buildable(units) },
        agent_rules,
    }
}

/// Seat 0 (an Arborec) holds a space dock on the first planet of the system and `goods` trade goods; the table has the factions' link.
fn home(goods: i16, units: &[Unit], limit: i32) -> (Rig<HomeProduction>, System, Planet) {
    home_with(goods, units, limit, true)
}

fn home_with(goods: i16, units: &[Unit], limit: i32, agent_rules: bool) -> (Rig<HomeProduction>, System, Planet) {
    let system = plain_systems()[0];
    let planet = system.data().planets[0];
    let sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat])
        .map_from(&[system])
        .control(planet, 0)
        .unit(system, Some(planet), Unit::SpaceDockI, 0, 1)
        .goods(0, goods)
        .tokens(0, 3, 3, 2);
    let mut table = ti4_engine_core::kernel::stage::AbilityTable::default();
    ti4_engine_core_factions::self_assembly::register_links(&mut table);
    let rig = Rig::new(sc, HomeProduction::new(facts(system, limit, units, agent_rules))).with_table(table);
    (rig, system, planet)
}

fn pay(rig: &mut Rig<HomeProduction>) {
    while !rig.is_done() && matches!(rig.asked().ask, Ask::Production(PAsk::Pay(_))) {
        rig.pick_where(|o| matches!(o, Opt::Production(POpt::TradeGood { .. })));
    }
}

fn build(rig: &mut Rig<HomeProduction>, unit: Unit) {
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Build { unit: u, .. }) if *u == unit));
    pay(rig);
}

fn stop(rig: &mut Rig<HomeProduction>) {
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Stop)));
}

fn goods(rig: &Rig<HomeProduction>) -> i16 {
    rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods
}

#[test]
fn a_use_of_production_builds_and_reports_what_it_made() {
    covers!("rules::production::home::HomeProduction");
    let (mut rig, system, _) = home(6, &[Unit::DestroyerI, Unit::InfantryI], 3);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Production(PAsk::Choose(_))), "{}", rig.transcript());
    build(&mut rig, Unit::DestroyerI);
    assert!(!rig.is_done());
    stop(&mut rig);
    assert!(rig.is_done() && !rig.driver.window.unsupported, "{}", rig.transcript());
    assert_eq!(goods(&rig), 5, "a destroyer costs one resource");
    let made = rig.driver.window.made();
    assert_eq!(made.as_slice().len(), 1);
    assert_eq!(made.as_slice()[0].0, Unit::DestroyerI);
    assert_eq!(rig.state.board().sys(system).units.as_slice().iter().filter(|p| p.kind() == Unit::DestroyerI).count(), 1);
}

#[test]
fn a_use_that_asks_nothing_has_no_end_of_use_effects_and_makes_nothing() {
    covers!("rules::production::home::HomeProduction");
    // Nothing is affordable: the window ends at once.
    let (mut rig, _, _) = home(0, &[Unit::CarrierI], 3);
    rig.state.board_mut().planet_mut(plain_systems()[0].data().planets[0]).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true);
    rig.run();
    assert!(rig.is_done() && rig.driver.window.made().as_slice().is_empty());
    assert!(rig.start_state().players().by_seat(Seat(0)).unwrap().econ == rig.state.players().by_seat(Seat(0)).unwrap().econ);
}

#[test]
fn the_discount_a_tactical_action_left_unspent_is_dropped_from_the_version_that_offers_the_agent() {
    covers!("rules::production::home::HomeProduction");
    let (mut rig, _, _) = home(6, &[Unit::DestroyerI], 3);
    rig.state.transient_mut().production.discount_remaining = 5;
    rig.run();
    assert_eq!(rig.state.transient().production.discount_remaining, 0);
    build(&mut rig, Unit::DestroyerI);
    assert_eq!(goods(&rig), 5, "the stale discount does not make the destroyer free");
    // Before that version the discount stays in the position (and is not read by this window either).
    let (mut old, _, _) = home_with(6, &[Unit::DestroyerI], 3, false);
    old.state.transient_mut().production.discount_remaining = 5;
    old.run();
    assert_eq!(old.state.transient().production.discount_remaining, 5);
}

#[test]
fn the_hacan_hero_is_offered_to_the_use_and_makes_every_unit_free() {
    covers!("rules::production::home::HomeProduction");
    let (mut rig, _, _) = home(6, &[Unit::DestroyerI, Unit::CarrierI], 3);
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::HarrughGefhara, LeaderStatus::Unlocked);
    let seq = rig.state.meta().production_seq;
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Production(PAsk::Hero(_))), "{}", rig.transcript());
    assert_eq!(rig.state.meta().production_seq, seq + 1, "the use counter goes up before the offer names it");
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::HeroUse)));
    build(&mut rig, Unit::CarrierI);
    stop(&mut rig);
    assert!(rig.is_done());
    assert_eq!(goods(&rig), 6, "the carrier cost nothing");
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::HarrughGefhara), Some(&LeaderStatus::Purged));
}

#[test]
fn a_declined_hero_pays_and_a_locked_one_is_not_offered() {
    covers!("rules::production::home::HomeProduction");
    let (mut rig, _, _) = home(6, &[Unit::CarrierI], 3);
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::HarrughGefhara, LeaderStatus::Unlocked);
    rig.run();
    rig.pick_where(|o| matches!(o, Opt::Production(POpt::Decline)));
    build(&mut rig, Unit::CarrierI);
    stop(&mut rig);
    assert_eq!(goods(&rig), 3, "three resources");
    let (mut locked, _, _) = home(6, &[Unit::CarrierI], 3);
    locked.state.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::HarrughGefhara, LeaderStatus::Locked);
    let seq = locked.state.meta().production_seq;
    locked.run();
    assert!(matches!(locked.asked().ask, Ask::Production(PAsk::Choose(_))));
    assert_eq!(locked.state.meta().production_seq, seq, "no offer, no new use number");
}

#[test]
fn auto_factories_pays_a_fleet_token_for_three_ships_that_are_no_fighters() {
    covers!("rules::production::home::HomeProduction");
    for (ships, tokens) in [(3, 1), (2, 0)] {
        let (mut rig, _, _) = home(9, &[Unit::DestroyerI], 3);
        rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().cards.breakthrough = Some(Breakthrough::AutoFactories);
        let before = rig.state.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens;
        rig.run();
        for _ in 0..ships {
            build(&mut rig, Unit::DestroyerI);
        }
        if !rig.is_done() {
            stop(&mut rig);
        }
        assert!(rig.is_done(), "{}", rig.transcript());
        assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens - before, tokens, "{ships} ships");
    }
    // The token is for ships that are no fighters.
    let (mut rig, _, _) = home(9, &[Unit::FighterI], 4);
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().cards.breakthrough = Some(Breakthrough::AutoFactories);
    let before = rig.state.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens;
    rig.run();
    build(&mut rig, Unit::FighterI);
    if !rig.is_done() {
        build(&mut rig, Unit::FighterI);
    }
    while !rig.is_done() {
        stop(&mut rig);
    }
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens, before);
}

#[test]
fn a_planet_value_swap_lasts_one_use() {
    covers!("rules::production::home::HomeProduction");
    let (mut rig, _, planet) = home(6, &[Unit::DestroyerI], 3);
    rig.state.transient_mut().production.value_swapped_planet = Some(planet);
    rig.run();
    build(&mut rig, Unit::DestroyerI);
    stop(&mut rig);
    assert!(rig.is_done());
    assert_eq!(rig.state.transient().production.value_swapped_planet, None);
}

#[test]
fn self_assembly_offers_a_mech_after_a_use_that_made_something() {
    covers!("rules::production::home::HomeProduction");
    let (mut rig, system, planet) = home(6, &[Unit::DestroyerI], 3);
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::SelfAssemblyRoutines);
    rig.run();
    build(&mut rig, Unit::DestroyerI);
    stop(&mut rig);
    assert!(matches!(rig.asked().ask, Ask::Technology(_)), "{}", rig.transcript());
    rig.pick_where(|o| matches!(o, Opt::Technology(_)) );
    assert!(rig.is_done() && !rig.driver.window.unsupported);
    assert!(rig.state.board().planet(planet).units.as_slice().iter().any(|p| p.owner() == Seat(0) && p.kind().data().base_type == UnitBaseType::Mech), "{system:?}");
}

#[test]
fn self_assembly_is_not_asked_when_nothing_was_made_or_the_card_is_not_owned() {
    covers!("rules::production::home::HomeProduction");
    let (mut rig, _, _) = home(6, &[Unit::DestroyerI], 3);
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::SelfAssemblyRoutines);
    rig.run();
    stop(&mut rig);
    assert!(rig.is_done(), "a use that made nothing offers no mech: {}", rig.transcript());
    let (mut rig, _, _) = home(6, &[Unit::DestroyerI], 3);
    rig.run();
    build(&mut rig, Unit::DestroyerI);
    stop(&mut rig);
    assert!(rig.is_done());
}

#[test]
fn a_table_without_the_factions_link_declines_the_run_when_self_assembly_is_owned() {
    covers!("rules::production::home::HomeProduction");
    let system = plain_systems()[0];
    let planet = system.data().planets[0];
    let sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).map_from(&[system]).control(planet, 0).unit(system, Some(planet), Unit::SpaceDockI, 0, 1).goods(0, 6).tokens(0, 3, 3, 2);
    let mut rig = Rig::new(sc, HomeProduction::new(facts(system, 3, &[Unit::DestroyerI], true)));
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::SelfAssemblyRoutines);
    rig.run();
    build(&mut rig, Unit::DestroyerI);
    stop(&mut rig);
    assert!(rig.is_done() && rig.driver.window.unsupported, "{}", rig.transcript());
}

#[test]
fn a_use_without_capacity_makes_no_offer_to_the_hacan_hero() {
    covers!("rules::production::home::HomeProduction");
    let (mut rig, _, _) = home(6, &[Unit::DestroyerI], 0);
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::HarrughGefhara, LeaderStatus::Unlocked);
    let seq = rig.state.meta().production_seq;
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.state.meta().production_seq, seq, "no use, no number");
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::HarrughGefhara), Some(&LeaderStatus::Unlocked));
}

/// Package W13: the Prophecy of Ixth ("using PRODUCTION discards it unless two or more fighters were produced") is discarded by the window at
/// the end of a use that made fewer than two fighters, and kept by one that made two.
#[test]
fn the_prophecy_of_ixth_is_discarded_by_a_use_that_made_fewer_than_two_fighters() {
    covers!("rules::production::home::HomeProduction");
    use ti4_engine_core::generated::Agenda;
    use ti4_engine_core::keys::Outcome;
    let prophecy = |rig: &mut Rig<HomeProduction>| {
        rig.state.agenda_mut().laws.insert(Agenda::ProphecyOfIxth, Outcome::Seat(Seat(0)));
    };
    // One fighter and a destroyer: discarded.
    let (mut rig, _, _) = home(6, &[Unit::FighterI, Unit::DestroyerI], 3);
    prophecy(&mut rig);
    rig.run();
    build(&mut rig, Unit::FighterI);
    stop(&mut rig);
    assert!(rig.is_done());
    assert!(!rig.state.agenda().laws.contains_key(&Agenda::ProphecyOfIxth), "one fighter is not two: the law is discarded");
    // Two fighters: kept.
    let (mut rig, _, _) = home(6, &[Unit::FighterI], 3);
    prophecy(&mut rig);
    rig.run();
    build(&mut rig, Unit::FighterI);
    build(&mut rig, Unit::FighterI);
    stop(&mut rig);
    assert!(rig.is_done());
    assert!(rig.state.agenda().laws.contains_key(&Agenda::ProphecyOfIxth), "two fighters keep it");
    // Held by another seat: never touched.
    let (mut rig, _, _) = home(6, &[Unit::DestroyerI], 3);
    rig.state.agenda_mut().laws.insert(Agenda::ProphecyOfIxth, Outcome::Seat(Seat(1)));
    rig.run();
    build(&mut rig, Unit::DestroyerI);
    stop(&mut rig);
    assert!(rig.state.agenda().laws.contains_key(&Agenda::ProphecyOfIxth));
}
