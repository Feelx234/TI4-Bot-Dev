//! D4 lane A2: the agenda table effects the first file did not reach (Rearmament, Swords to Plowshares, Core Mining, Demilitarized Zone,
//! Wormhole Reconstruction, Enforced Travel Ban, Wormhole Research, Minister of Antiquities, Clandestine Operations in detail) and the
//! `Table::of` mapping from (agenda, outcome) to the effect, against the agenda texts.

use ti4_engine_core::containers::SmallList;
use ti4_engine_core::rules::agenda::effects::Resolve;
use ti4_engine_core::rules::agenda::table_effects::{EffectFacts, Table};
use ti4_engine_core::state::{Code, EventKind, Outcome};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn facts(for_voters: &[u8]) -> EffectFacts {
    EffectFacts {
        for_voters: for_voters.iter().map(|s| Seat(*s)).collect::<SmallList<Seat, 8>>(),
        against_voters: SmallList::new(),
        mahact_held: [0; 8],
        scope: ALL_SOURCES,
        default_scope: ALL_SOURCES,
        source: Code(1),
        pool: Code(2),
        has_galaxy: true,
        defense_act: Code(3),
        hooked: 0,
        drop_unofficial: false,
        blanked: false,
    }
}

fn run(sc: Scenario, agenda: Agenda, outcome: Outcome, f: &EffectFacts) -> Rig<Resolve> {
    let window = Resolve::of_facts(agenda, outcome, f).unwrap_or_else(|| panic!("an effect for {agenda:?} {outcome:?}"));
    let mut rig = Rig::from_parts(sc.state(), kernel_all(), window);
    rig.run();
    rig
}

fn two(map: &[System]) -> Scenario {
    Scenario::lobby(2).map_from(map)
}

fn units_on(rig: &Rig<Resolve>, planet: Planet, seat: u8) -> Vec<Unit> {
    rig.state.board().planet(planet).units.iter().filter(|p| p.owner() == Seat(seat)).map(|p| p.kind()).collect()
}

fn goods(rig: &Rig<Resolve>, seat: u8) -> i16 {
    rig.state.players().by_seat(Seat(seat)).unwrap().econ.trade_goods
}

// ---- the mapping ------------------------------------------------------------------------------------

#[test]
fn each_agenda_outcome_maps_to_its_effect_and_the_rest_is_not_expressed() {
    covers!("rules::agenda::table_effects::Table");
    let of = |a, o| Table::of(a, o, true);
    assert_eq!(of(Agenda::RegulatedConscription, Outcome::For), Some((Table::Nothing, false)));
    assert_eq!(of(Agenda::RegulatedConscription, Outcome::Against), Some((Table::Nothing, false)), "either way nothing");
    assert_eq!(of(Agenda::ArmedForcesStandardization, Outcome::Seat(Seat(2))), Some((Table::Standardize(Seat(2)), false)));
    assert_eq!(of(Agenda::ClandestineOperations, Outcome::For), Some((Table::Clandestine { both: true }, false)));
    assert_eq!(of(Agenda::ClandestineOperations, Outcome::Against), Some((Table::Clandestine { both: false }, false)));
    assert_eq!(of(Agenda::FleetRegulations, Outcome::For), Some((Table::Nothing, false)), "the For is a law with no immediate effect");
    assert_eq!(of(Agenda::FleetRegulations, Outcome::Against), Some((Table::Regulations, false)));
    assert_eq!(of(Agenda::EconomicEquality, Outcome::For), Some((Table::Equality { pay: true }, false)));
    assert_eq!(of(Agenda::EconomicEquality, Outcome::Against), Some((Table::Equality { pay: false }, false)));
    assert_eq!(of(Agenda::ArticlesOfWar, Outcome::Against), Some((Table::ArticlesWar, false)));
    assert_eq!(of(Agenda::PublicExecution, Outcome::Seat(Seat(0))), Some((Table::Execution(Seat(0)), true)), "deferred: it passes the speaker token");
    assert_eq!(of(Agenda::ConventionsOfWar, Outcome::Against), Some((Table::ConventionsHands, false)));
    assert_eq!(of(Agenda::UnconventionalMeasures, Outcome::For), Some((Table::VotersDraw, false)));
    assert_eq!(of(Agenda::UnconventionalMeasures, Outcome::Against), Some((Table::VotersHands, false)));
    assert_eq!(of(Agenda::MinisterOfAntiquities, Outcome::Seat(Seat(1))), Some((Table::Relic(Seat(1)), false)));
    assert_eq!(of(Agenda::PublicizeWeaponSchematics, Outcome::Against), Some((Table::Schematics, false)));
    assert_eq!(of(Agenda::SharedResearch, Outcome::Against), Some((Table::SharedResearch, false)));
    assert_eq!(of(Agenda::WormholeReconstruction, Outcome::Against), Some((Table::WormholeRecon, false)));
    assert_eq!(of(Agenda::ArmsReduction, Outcome::For), Some((Table::ArmsReduction, false)));
    assert_eq!(of(Agenda::ArmsReduction, Outcome::Against), Some((Table::Nothing, true)), "the Against waits for the next strategy phase");
    assert_eq!(of(Agenda::RearmamentAgreement, Outcome::Against), Some((Table::Rearm, false)));
    assert_eq!(of(Agenda::RearmamentAgreement, Outcome::For), None, "the For places mechs through supply and faction hooks: the string's");
    assert_eq!(of(Agenda::SwordsToPlowshares, Outcome::For), Some((Table::Plowshares, false)));
    assert_eq!(of(Agenda::SwordsToPlowshares, Outcome::Against), None);
    assert_eq!(of(Agenda::IncentiveProgram, Outcome::For), Some((Table::Incentive(1), false)));
    assert_eq!(of(Agenda::IncentiveProgram, Outcome::Against), Some((Table::Incentive(2), false)));
    let p = Planet::MecatolRex;
    assert_eq!(of(Agenda::CompensatedDisarmament, Outcome::Planet(p)), Some((Table::Disarmament(p), false)));
    assert_eq!(of(Agenda::CoreMining, Outcome::Planet(p)), Some((Table::CoreMining(p), false)));
    assert_eq!(of(Agenda::DemilitarizedZone, Outcome::Planet(p)), Some((Table::Demilitarized(p), false)));
    // Effects that read the map end deferred without it.
    assert_eq!(Table::of(Agenda::EnforcedTravelBan, Outcome::Against, true), Some((Table::TravelBan, false)));
    assert_eq!(Table::of(Agenda::EnforcedTravelBan, Outcome::Against, false), Some((Table::Nothing, true)));
    assert_eq!(Table::of(Agenda::WormholeResearch, Outcome::For, true), Some((Table::WormholeShips, true)));
    assert_eq!(Table::of(Agenda::WormholeResearch, Outcome::For, false), Some((Table::Nothing, true)));
    assert_eq!(Table::of(Agenda::WormholeResearch, Outcome::Against, true), Some((Table::Nothing, true)));
    assert!(Table::WormholeShips.needs_map() && Table::TravelBan.needs_map() && !Table::Rearm.needs_map());
    assert!(Table::Relic(Seat(0)).draws_from_deck() && Table::VotersDraw.draws_from_deck() && Table::Incentive(1).draws_from_deck() && !Table::Plowshares.draws_from_deck());
}

// ---- units ------------------------------------------------------------------------------------------

#[test]
fn rearmament_against_turns_every_mech_on_a_planet_into_an_infantry_of_its_owner() {
    covers!("rules::agenda::table_effects::Table");
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let sc = two(&[system]).control(planet, 0).unit(system, Some(planet), Unit::FactionlessMech, 0, 2).unit(system, Some(planet), Unit::InfantryI, 0, 1).unit(system, Some(planet), Unit::FactionlessMech, 1, 1).unit(system, Some(planet), Unit::PDSI, 1, 1);
    let rig = run(sc, Agenda::RearmamentAgreement, Outcome::Against, &facts(&[]));
    assert_eq!(units_on(&rig, planet, 0), vec![Unit::InfantryI, Unit::InfantryI, Unit::InfantryI]);
    assert_eq!(units_on(&rig, planet, 1), vec![Unit::InfantryI, Unit::PDSI], "the other seat's mech too; structures stay");
    // A damaged mech comes back as a healthy infantry; a galvanized one stays galvanized.
    let (mut s, k) = two(&[system]).control(planet, 0).unit(system, Some(planet), Unit::FactionlessMech, 0, 2).build();
    let units = s.board_mut().planet_mut(planet).units.as_mut_slice();
    units[0] = units[0].with_sustained(true);
    units[1] = units[1].with_galvanized(true);
    let window = Resolve::of_facts(Agenda::RearmamentAgreement, Outcome::Against, &facts(&[])).unwrap();
    let mut rig = Rig::from_parts(s, k, window);
    rig.run();
    let after: Vec<(Unit, bool, bool)> = rig.state.board().planet(planet).units.iter().map(|p| (p.kind(), p.sustained(), p.galvanized())).collect();
    assert_eq!(after, vec![(Unit::InfantryI, false, false), (Unit::InfantryI, false, true)]);
}

#[test]
fn swords_to_plowshares_for_destroys_half_the_infantry_on_each_planet_rounded_up_and_pays_a_good_each() {
    covers!("rules::agenda::table_effects::Table");
    let sys = plain_systems();
    let (a, b) = (sys[0].data().planets[0], sys[1].data().planets[0]);
    let sc = two(&[sys[0], sys[1]]).control(a, 0).control(b, 0).unit(sys[0], Some(a), Unit::InfantryI, 0, 5).unit(sys[1], Some(b), Unit::InfantryI, 0, 2).unit(sys[1], Some(b), Unit::FactionlessMech, 0, 1).goods(0, 1).control(sys[1].data().planets[0], 0);
    let rig = run(sc, Agenda::SwordsToPlowshares, Outcome::For, &facts(&[]));
    assert_eq!(units_on(&rig, a, 0).len(), 2, "5 infantry: half rounded up (3) destroyed");
    assert_eq!(units_on(&rig, b, 0).iter().filter(|u| **u == Unit::InfantryI).count(), 1, "2 infantry: 1 destroyed");
    assert!(units_on(&rig, b, 0).contains(&Unit::FactionlessMech), "mechs are not infantry");
    assert_eq!(goods(&rig, 0), 1 + 3 + 1, "one trade good for each infantry destroyed");
    assert_eq!(goods(&rig, 1), 0, "nothing for a seat without infantry");
    // Only the planets a seat controls count: infantry of a seat on a planet another seat controls are safe.
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let foreign = two(&[system]).control(planet, 1).unit(system, Some(planet), Unit::InfantryI, 0, 4);
    let rig = run(foreign, Agenda::SwordsToPlowshares, Outcome::For, &facts(&[]));
    assert_eq!(units_on(&rig, planet, 0).len(), 4);
}

#[test]
fn core_mining_destroys_one_infantry_on_the_elected_planet_and_demilitarized_zone_every_unit() {
    covers!("rules::agenda::table_effects::Table");
    let (system, planet) = ti4_engine_core_tests::scenario::first_planet();
    let sc = || two(&[system]).control(planet, 0).unit(system, Some(planet), Unit::InfantryI, 0, 3).unit(system, Some(planet), Unit::InfantryI, 1, 1).unit(system, Some(planet), Unit::FactionlessMech, 0, 1).unit(system, Some(planet), Unit::SpaceDockI, 0, 1);
    let rig = run(sc(), Agenda::CoreMining, Outcome::Planet(planet), &facts(&[]));
    let left: Vec<Unit> = rig.state.board().planet(planet).units.iter().map(|p| p.kind()).collect();
    assert_eq!(left.iter().filter(|u| **u == Unit::InfantryI).count(), 3, "of four infantry exactly one is destroyed");
    assert!(left.contains(&Unit::FactionlessMech) && left.contains(&Unit::SpaceDockI));
    let rig = run(sc(), Agenda::DemilitarizedZone, Outcome::Planet(planet), &facts(&[]));
    assert!(rig.state.board().planet(planet).units.is_empty(), "all units on the planet, structures included");
    // The destroyed ground forces are staged for the coordinator, one row each.
    let staged = rig.state.marks().tail.iter().filter(|(k, _)| matches!(k, ti4_engine_core::keys::MarkKey::PrivateStagedGround(_))).count();
    assert_eq!(staged, 5, "four infantry and a mech; the dock is no ground force");
    // Another planet is untouched.
    let other = plain_systems()[1].data().planets[0];
    let rig = run(two(&[system, plain_systems()[1]]).control(other, 1).unit(plain_systems()[1], Some(other), Unit::InfantryI, 1, 2), Agenda::DemilitarizedZone, Outcome::Planet(planet), &facts(&[]));
    assert_eq!(units_on(&rig, other, 1).len(), 2);
}

// ---- tokens -----------------------------------------------------------------------------------------

#[test]
fn clandestine_operations_for_takes_two_tokens_from_the_fullest_pools_against_one_fleet_token() {
    covers!("rules::agenda::table_effects::Table");
    let pools = |rig: &Rig<Resolve>, seat: u8| {
        let e = rig.state.players().by_seat(Seat(seat)).unwrap().econ;
        (e.tactic_tokens, e.fleet_tokens, e.strategic_tokens)
    };
    let sc = || Scenario::lobby(3).tokens(0, 3, 3, 2).tokens(1, 5, 1, 1).tokens(2, 0, 0, 0);
    let rig = run(sc(), Agenda::ClandestineOperations, Outcome::For, &facts(&[]));
    assert_eq!(pools(&rig, 0), (2, 2, 2), "two tokens off the fullest pools, a tie going to the last pool: fleet 3->2, then tactic 3->2");
    assert_eq!(pools(&rig, 1), (3, 1, 1), "two off the tactic pool");
    assert_eq!(pools(&rig, 2), (0, 0, 0), "nothing below zero");
    let rig = run(sc(), Agenda::ClandestineOperations, Outcome::Against, &facts(&[]));
    assert_eq!(pools(&rig, 0), (3, 2, 2));
    assert_eq!(pools(&rig, 1), (5, 0, 1));
    assert_eq!(pools(&rig, 2), (0, 0, 0), "an empty fleet pool loses nothing");
}

#[test]
fn wormhole_reconstruction_against_puts_a_token_in_each_wormhole_system_that_holds_a_ship_of_ours() {
    covers!("rules::agenda::table_effects::Table");
    let hole = System::ALL.iter().copied().find(|s| !s.data().wormholes.is_empty() && !s.data().hyperlane && s.data().planets.is_empty() || !s.data().wormholes.is_empty() && !s.data().hyperlane).expect("a wormhole tile");
    let plain = plain_systems()[0];
    let sc = two(&[plain, hole]).unit(hole, None, Unit::CruiserI, 0, 1).unit(plain, None, Unit::CruiserI, 1, 1);
    let rig = run(sc, Agenda::WormholeReconstruction, Outcome::Against, &facts(&[]));
    assert!(rig.state.board().sys(hole).cmd.contains(Seat(0)), "our ship sits in a wormhole system");
    assert!(!rig.state.board().sys(hole).cmd.contains(Seat(1)));
    assert!(!rig.state.board().sys(plain).cmd.contains(Seat(1)), "no wormhole in the plain system");
    let placed = rig.state.pending().staged.iter().filter(|e| e.kind == EventKind::CommandTokenPlaced).count();
    assert_eq!(placed, 1);
}

// ---- the map effects ----------------------------------------------------------------------------------

#[test]
fn enforced_travel_ban_against_destroys_pds_in_and_beside_wormhole_systems() {
    covers!("rules::agenda::table_effects::Table");
    let hole = System::ALL.iter().copied().find(|s| !s.data().wormholes.is_empty() && !s.data().planets.is_empty() && !s.data().hyperlane).expect("a wormhole tile with a planet");
    let plain = plain_systems().into_iter().find(|s| *s != hole && s.data().planets.len() >= 1).unwrap();
    let far = plain_systems().into_iter().filter(|s| *s != hole && *s != plain).nth(5).unwrap();
    let (hp, np, fp) = (hole.data().planets[0], plain.data().planets[0], far.data().planets[0]);
    // The hole at the centre, the plain tile next to it (hex 1), the far tile six steps out (not adjacent).
    use ti4_engine_core::hex::Hex;
    let sc = Scenario::lobby(2)
        .map_hexes(&[(hole, Hex::new(0, 0)), (plain, Hex::new(1, 0)), (far, Hex::new(3, 0))])
        .control(hp, 0)
        .control(np, 1)
        .control(fp, 1)
        .unit(hole, Some(hp), Unit::PDSI, 0, 1)
        .unit(hole, Some(hp), Unit::InfantryI, 0, 1)
        .unit(plain, Some(np), Unit::PDSII, 1, 1)
        .unit(far, Some(fp), Unit::PDSI, 1, 1);
    let (mut s, k) = sc.build();
    for sy in [hole, plain, far] {
        s.board_mut().sys_mut(sy).flags.set(ti4_engine_core::state::SysFlags::TOUCHED, true);
    }
    let window = Resolve::of_facts(Agenda::EnforcedTravelBan, Outcome::Against, &facts(&[])).unwrap();
    let mut rig = Rig::from_parts(s, k, window);
    rig.run();
    assert_eq!(units_on(&rig, hp, 0), vec![Unit::InfantryI], "the PDS in the wormhole system is destroyed, the infantry stay");
    assert!(units_on(&rig, np, 1).is_empty(), "adjacent to a wormhole system");
    assert_eq!(units_on(&rig, fp, 1), vec![Unit::PDSI], "two tiles away: safe");
}

#[test]
fn wormhole_research_for_destroys_every_ship_in_alpha_and_beta_systems_and_no_others() {
    covers!("rules::agenda::table_effects::Table");
    use ti4_engine_core::generated::Wormhole;
    let alpha = System::ALL.iter().copied().find(|s| s.data().wormholes.contains(&Wormhole::Alpha) && !s.data().hyperlane).expect("an alpha tile");
    let gamma = System::ALL.iter().copied().find(|s| s.data().wormholes.contains(&Wormhole::Gamma) && !s.data().wormholes.contains(&Wormhole::Alpha) && !s.data().wormholes.contains(&Wormhole::Beta) && !s.data().hyperlane);
    let plain = plain_systems()[0];
    let mut tiles = vec![plain, alpha];
    if let Some(g) = gamma {
        tiles.push(g);
    }
    let mut sc = two(&tiles).unit(alpha, None, Unit::CruiserI, 0, 2).unit(alpha, None, Unit::FighterI, 1, 1).unit(plain, None, Unit::CruiserI, 0, 1);
    if let Some(g) = gamma {
        sc = sc.unit(g, None, Unit::CruiserI, 1, 1);
    }
    let (mut s, k) = sc.build();
    for sy in &tiles {
        s.board_mut().sys_mut(*sy).flags.set(ti4_engine_core::state::SysFlags::TOUCHED, true);
    }
    let window = Resolve::of_facts(Agenda::WormholeResearch, Outcome::For, &facts(&[])).unwrap();
    let mut rig = Rig::from_parts(s, k, window);
    rig.run();
    assert!(rig.state.board().sys(alpha).units.is_empty(), "every ship in the alpha system");
    assert_eq!(rig.state.board().sys(plain).units.len(), 1);
    if let Some(g) = gamma {
        assert_eq!(rig.state.board().sys(g).units.len(), 1, "a gamma wormhole is neither alpha nor beta");
    }
}

#[test]
fn minister_of_antiquities_gives_the_elected_player_the_top_relic() {
    covers!("rules::agenda::table_effects::Table");
    let top = ti4_engine_core::generated::Relic::ALL[3];
    let mut s = Scenario::lobby(3).state();
    s.decks_mut().relic = ti4_engine_core::containers::Deck::new(vec![top, ti4_engine_core::generated::Relic::ALL[4]]);
    let window = Resolve::of_facts(Agenda::MinisterOfAntiquities, Outcome::Seat(Seat(2)), &facts(&[])).unwrap();
    let mut rig = Rig::from_parts(s, kernel_all(), window);
    rig.run();
    assert!(rig.state.players().by_seat(Seat(2)).unwrap().cards.relics.as_slice().contains(&top));
    assert!(rig.state.players().by_seat(Seat(0)).unwrap().cards.relics.as_slice().is_empty());
    assert_eq!(rig.state.decks().relic.cards().len(), 1, "one card left in the deck");
}
