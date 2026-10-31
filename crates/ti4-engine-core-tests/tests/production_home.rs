//! Lent-bodies lane 9: `rules::production::home::HomeProduction`, a use of PRODUCTION at a seat's home system outside a tactical action (the
//! Warfare secondary): the Hacan hero's offer, the production, the effects at the end of the use and Self-Assembly Routines' offer.

use ti4_engine_core::choice::production::{Ask as PAsk, Opt as POpt};
use ti4_engine_core::kernel::children::{HomeFacts, ProduceFacts};
use ti4_engine_core::rules::production::home::HomeProduction;
use ti4_engine_core::state::{LeaderStatus, Piece};
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
    // A destroyer: no fighter, discarded.
    let (mut rig, _, _) = home(6, &[Unit::FighterI, Unit::DestroyerI], 3);
    prophecy(&mut rig);
    rig.run();
    build(&mut rig, Unit::DestroyerI);
    stop(&mut rig);
    assert!(rig.is_done());
    assert!(!rig.state.agenda().laws.contains_key(&Agenda::ProphecyOfIxth), "no fighter is not two: the law is discarded");
    // A fighter is built two at a time: kept.
    let (mut rig, _, _) = home(6, &[Unit::FighterI], 3);
    prophecy(&mut rig);
    rig.run();
    build(&mut rig, Unit::FighterI);
    stop(&mut rig);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.made().as_slice().iter().filter(|(u, _)| *u == Unit::FighterI).count(), 2);
    assert!(rig.state.agenda().laws.contains_key(&Agenda::ProphecyOfIxth), "two fighters keep it");
    // Held by another seat: never touched.
    let (mut rig, _, _) = home(6, &[Unit::DestroyerI], 3);
    rig.state.agenda_mut().laws.insert(Agenda::ProphecyOfIxth, Outcome::Seat(Seat(1)));
    rig.run();
    build(&mut rig, Unit::DestroyerI);
    stop(&mut rig);
    assert!(rig.state.agenda().laws.contains_key(&Agenda::ProphecyOfIxth));
}

/// A use that made exactly one fighter (the limit leaves room for one) discards the Prophecy of Ixth: one is fewer than two.
#[test]
fn a_single_fighter_discards_the_prophecy_of_ixth() {
    covers!("rules::production::home::HomeProduction");
    use ti4_engine_core::generated::Agenda;
    use ti4_engine_core::keys::Outcome;
    let (mut rig, _, _) = home(6, &[Unit::FighterI], 1);
    rig.state.agenda_mut().laws.insert(Agenda::ProphecyOfIxth, Outcome::Seat(Seat(0)));
    rig.run();
    build(&mut rig, Unit::FighterI);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.made().as_slice().iter().filter(|(u, _)| *u == Unit::FighterI).count(), 1, "{}", rig.transcript());
    assert!(!rig.state.agenda().laws.contains_key(&Agenda::ProphecyOfIxth));
}

// -- Package W14: the facts are read from the position (the Warfare secondary after a secondary that changed them) ---------------------------

use ti4_engine_core::query::tech::UnitRoster;
use ti4_engine_core::rules::production::home::{buildable_for, fleet_limit, home_facts};
use ti4_engine_core::tables::Scope;

fn roster() -> &'static UnitRoster {
    Box::leak(Box::new(UnitRoster::default()))
}

/// A Letnev seat with a dock, three fleet tokens and a table of two.
fn letnev_table() -> (CoreState, ti4_engine_core::kernel::Kernel, System) {
    let system = plain_systems()[0];
    let planet = system.data().planets[0];
    let sc = Scenario::new(&[Faction::TheBaronyOfLetnev, Faction::TheArborec])
        .kernel(kernel_all())
        .map_from(&[system])
        .control(planet, 0)
        .unit(system, Some(planet), Unit::SpaceDockI, 0, 1)
        .tokens(0, 3, 3, 2);
    let (state, kernel) = sc.build();
    (state, kernel, system)
}

#[test]
fn the_facts_of_a_home_use_follow_the_tokens_the_forms_and_the_commanders_of_the_position() {
    covers!("rules::production::home::home_facts");
    covers!("rules::production::home::fleet_limit");
    covers!("rules::production::home::buildable_for");
    let (mut state, kernel, system) = letnev_table();
    let scope = Scope(u32::from(kernel.sources.0));
    let facts = |state: &CoreState| home_facts(state, &kernel, roster(), Seat(0), system, true).expect("a seat").produce;
    let first = facts(&state);
    assert_eq!((first.fleet_limit, first.armada), (5, 2), "three tokens and Armada's two");
    assert_eq!((first.small_exemptions, first.flagship_free, first.discount, first.max_unit_cost), (0, false, 0, None));
    assert!(first.limit > 0, "the dock produces");
    assert_eq!(first.buildable[..6], [Some(Unit::FighterI), Some(Unit::InfantryI), Some(Unit::CarrierI), Some(Unit::CruiserI), Some(Unit::DestroyerI), Some(Unit::DreadnoughtI)]);
    assert!(first.buildable[6..].iter().all(Option::is_none), "no mech, flagship or war sun without a form of the faction or the technology");

    // A token more in the fleet pool, an upgrade researched, a second producer, the commander of Cabal and of Nomad unlocked.
    let p = state.players_mut().by_seat_mut(Seat(0)).unwrap();
    p.econ.fleet_tokens += 1;
    p.tech.owned.insert(Technology::CruiserII);
    p.leaders.status.insert(Leader::ThatWhichMoldsFlesh, LeaderStatus::Unlocked);
    p.leaders.status.insert(Leader::NavarchFeng, LeaderStatus::Unlocked);
    let second = system.data().planets.get(1).copied();
    let before = first.limit;
    if let Some(second) = second {
        state.board_mut().planet_mut(second).control = Some(Seat(0));
        state.board_mut().sys_mut(system).units.push(Piece::new(Unit::SpaceDockI, Seat(0)));
    }
    let later = facts(&state);
    assert_eq!(later.fleet_limit, 6, "the token gained is seen");
    assert_eq!(later.buildable[3], Some(Unit::CruiserII), "the upgrade researched replaces the cruiser");
    assert_eq!((later.small_exemptions, later.flagship_free), (2, true));
    assert!(second.is_none() || later.limit > before, "a producer more is seen: {before} then {}", later.limit);
    assert_eq!(buildable_for(&state, &kernel, scope, roster(), Seat(0)), Some(later.buildable));
    assert!(buildable_for(&state, &kernel, scope, roster(), Seat(7)).is_none());

    // The round of Darktalon Treilla's hero: no limit.
    let round = state.meta().round;
    state.players_mut().by_seat_mut(Seat(0)).unwrap().production_timers.fleet_supply_unlimited_until = ti4_engine_core::Opt32::some(round);
    assert_eq!(fleet_limit(&state, &kernel, false, Seat(0)), 10_000);
}

#[test]
fn fleet_regulations_hold_the_pool_at_four_before_armada_lifts_it() {
    covers!("rules::production::home::fleet_limit");
    use ti4_engine_core::generated::Agenda;
    use ti4_engine_core::keys::Outcome;
    let (mut state, kernel, _) = letnev_table();
    state.players_mut().by_seat_mut(Seat(0)).unwrap().econ.fleet_tokens = 7;
    assert_eq!(fleet_limit(&state, &kernel, false, Seat(0)), 9);
    state.agenda_mut().laws.insert(Agenda::FleetRegulations, Outcome::For);
    assert_eq!(fleet_limit(&state, &kernel, false, Seat(0)), 6, "the law caps first, the ability lifts afterwards");
    assert_eq!(fleet_limit(&state, &kernel, true, Seat(0)), 9, "a blanked law caps nothing");
}

#[test]
fn a_use_started_in_a_system_with_nothing_to_produce_with_opens_no_window() {
    covers!("rules::production::home::HomeProduction");
    let system = plain_systems()[0];
    let sc = Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat]).kernel(kernel_all()).map_from(&[system]).tokens(0, 3, 3, 2);
    let mut rig = Rig::new(sc, HomeProduction::at(Seat(0), system, roster(), true));
    let seq = rig.state.meta().production_seq;
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::HarrughGefhara, LeaderStatus::Unlocked);
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.unsupported && rig.driver.window.made().as_slice().is_empty(), "{}", rig.transcript());
    assert_eq!(rig.state.meta().production_seq, seq, "no use, no number, no offer to the hero");
}

#[test]
fn a_use_read_from_the_position_produces_like_a_use_with_given_facts() {
    covers!("rules::production::home::HomeProduction");
    let (state, kernel, system) = letnev_table();
    let mut rig = Rig::from_parts(state, kernel, HomeProduction::at(Seat(0), system, roster(), true));
    rig.state.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 6;
    rig.run();
    build(&mut rig, Unit::DestroyerI);
    stop(&mut rig);
    assert!(rig.is_done() && !rig.driver.window.unsupported, "{}", rig.transcript());
    assert_eq!(rig.driver.window.made().as_slice().iter().filter(|(u, _)| *u == Unit::DestroyerI).count(), 1);
}
