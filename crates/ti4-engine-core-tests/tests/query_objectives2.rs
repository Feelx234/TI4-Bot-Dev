//! D4 lane A3: `query/objectives.rs` with `Pos`, `SeatFacts` and `Terms` fixtures. The expectations come from the printed text of the public
//! and secret objectives (and of Keleres' I.I.H.Q. Modernization for neighbours), the thresholds from the cards.

use ti4_engine_core::generated::{Agenda, Attachment, ExploreType, FactionTag, Planet, PublicObjective as O, Relic, SecretObjective as S, System, Technology as T, Unit};
use ti4_engine_core::bits::SysSet;
use ti4_engine_core::query::economy::Spend;
use ti4_engine_core::query::objectives::{self as q, Cost, CostFamily, Family, HandStep, Imagined, Pos, Progress, SeatFacts, Terms};
use ti4_engine_core::state::{CoreState, Outcome};
use ti4_engine_core::tables::{Scope, source_bit};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn table() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheEmbersOfMuaat, Faction::TheYssarilTribes])
}

fn terms() -> Terms {
    Terms { per_good: 1, ..Terms::default() }
}

fn facts(s: &CoreState, seat: u8) -> SeatFacts {
    SeatFacts::of(s, ALL_SOURCES, Seat(seat), terms())
}

fn counting(s: &CoreState, seat: u8, map: bool, im: Imagined<'_>, obj: O) -> Option<Progress> {
    let f = facts(s, seat);
    let edge = q::edge_systems(s);
    q::counting_progress(&Pos::new(s, &f, map, im), &edge, obj)
}

fn remaining(s: &CoreState, seat: u8, map: bool, im: Imagined<'_>, obj: O) -> Option<Progress> {
    let f = facts(s, seat);
    q::remaining_position_progress(&Pos::new(s, &f, map, im), obj)
}

fn have(p: Option<Progress>) -> usize {
    p.expect("a progress").have
}

fn secret(s: &CoreState, seat: u8, map: bool, im: Imagined<'_>, obj: S) -> Option<Progress> {
    secret_with(s, seat, terms(), map, im, obj)
}

fn secret_with(s: &CoreState, seat: u8, t: Terms, map: bool, im: Imagined<'_>, obj: S) -> Option<Progress> {
    let f = SeatFacts::of(s, ALL_SOURCES, Seat(seat), t);
    q::secret_progress(&Pos::new(s, &f, map, im), obj).expect("supported")
}

/// The first planet printed on a tile.
fn pl(system: System) -> Planet {
    system.data().planets[0]
}

const NONE: Imagined<'static> = Imagined::NONE;

// ---- the printed tables -----------------------------------------------------------------------

#[test]
fn the_thresholds_of_the_counting_objectives_are_the_printed_numbers() {
    let rows: [(O, Family, usize); 24] = [
        (O::ExpandBorders, Family::NonHome, 6),
        (O::SubdueTheGalaxy, Family::NonHome, 11),
        (O::PopulateTheOuterRim, Family::OnTheRim, 3),
        (O::ControlTheBorderlands, Family::OnTheRim, 5),
        (O::CornerTheMarket, Family::SameTrait, 4),
        (O::UnifyTheColonies, Family::SameTrait, 6),
        (O::FoundResearchOutposts, Family::TechSpecialties, 3),
        (O::FormGalacticBrainTrust, Family::TechSpecialties, 5),
        (O::DevelopWeaponry, Family::UnitUpgrades, 2),
        (O::RevolutionizeWarfare, Family::UnitUpgrades, 3),
        (O::DiversifyResearch, Family::Colours { per_colour: 2 }, 2),
        (O::MasterTheSciences, Family::Colours { per_colour: 2 }, 4),
        (O::BuildDefenses, Family::Structures, 4),
        (O::ConstructMassiveCities, Family::Structures, 7),
        (O::ImproveInfrastructure, Family::StructuresAway, 3),
        (O::ProtectTheBorder, Family::StructuresAway, 5),
        (O::RaiseAFleet, Family::FleetInOneSystem, 5),
        (O::CommandAnArmada, Family::FleetInOneSystem, 8),
        (O::ExploreDeepSpace, Family::PlanetlessSystems, 3),
        (O::PatrolVastTerritories, Family::PlanetlessSystems, 5),
        (O::ReclaimAncientMonuments, Family::AttachedPlanets, 3),
        (O::DiscoverLostOutposts, Family::AttachedPlanets, 2),
        (O::MakeHistory, Family::NotableSystems, 2),
        (O::BecomeALegend, Family::NotableSystems, 4),
    ];
    for (o, family, threshold) in rows {
        assert_eq!(q::counting_row(o), Some((family, threshold)), "{o:?}");
    }
    assert_eq!(O::ALL.iter().filter(|o| q::counting_row(**o).is_some()).count(), 24, "no other objective counts");
}

#[test]
fn the_remaining_position_objectives_and_the_prices_are_the_printed_ones() {
    let rows = [
        (O::ConquerTheWeak, Family::RivalHomePlanets, 1),
        (O::EngineerAMarvel, Family::CapitalShipSystems, 1),
        (O::AchieveSupremacy, Family::CapitalShipsInRivalHomeOrMecatol, 1),
        (O::IntimidateCouncil, Family::ShipsAdjacentToMecatol, 2),
        (O::PushBoundaries, Family::WeakerNeighbours, 2),
        (O::RuleDistantLands, Family::DistinctRivalHomeReaches, 2),
    ];
    for (o, family, threshold) in rows {
        assert_eq!(q::remaining_row(o), Some((family, threshold)), "{o:?}");
        assert!(q::counting_row(o).is_none(), "{o:?} is not a counting objective");
    }
    assert_eq!(O::ALL.iter().filter(|o| q::remaining_row(**o).is_some()).count(), 6);
    assert_eq!(q::cost_of(O::ErectAMonument), Some(Cost::Spend { amount: 8, kind: Spend::Resources }));
    assert_eq!(q::cost_of(O::FoundAGoldenAge), Some(Cost::Spend { amount: 16, kind: Spend::Resources }));
    assert_eq!(q::cost_of(O::SwayTheCouncil), Some(Cost::Spend { amount: 8, kind: Spend::Influence }));
    assert_eq!(q::cost_of(O::ManipulateGalacticLaw), Some(Cost::Spend { amount: 16, kind: Spend::Influence }));
    assert_eq!(q::cost_of(O::NegotiateTradeRoutes), Some(Cost::TradeGoods(5)));
    assert_eq!(q::cost_of(O::CentralizeGalacticTrade), Some(Cost::TradeGoods(10)));
    assert_eq!(q::cost_of(O::LeadFromTheFront), Some(Cost::Tokens(3)));
    assert_eq!(q::cost_of(O::GalvanizeThePeople), Some(Cost::Tokens(6)));
    assert_eq!(q::cost_of(O::AmassWealth), Some(Cost::AllThree(3)));
    assert_eq!(q::cost_of(O::HoldVastReserves), Some(Cost::AllThree(6)));
    assert_eq!(O::ALL.iter().filter(|o| q::cost_of(**o).is_some()).count(), 10);
}

#[test]
fn an_objective_of_another_family_is_not_answered_by_the_wrong_function() {
    let s = table().state();
    assert!(counting(&s, 0, true, NONE, O::ConquerTheWeak).is_none());
    assert!(counting(&s, 0, true, NONE, O::ErectAMonument).is_none());
    assert!(remaining(&s, 0, true, NONE, O::ExpandBorders).is_none());
    let f = facts(&s, 0);
    assert!(q::bought_progress(&Pos::new(&s, &f, true, NONE), O::ExpandBorders).is_none());
}

// ---- planets ------------------------------------------------------------------------------------

#[test]
fn expand_borders_counts_the_controlled_planets_outside_every_home_system() {
    let base: Vec<Planet> = [System::Wellon, System::VefutII, System::Thibah, System::Tarmann, System::Saudor, System::MeharXull].map(pl).to_vec();
    let mut sc = table();
    for p in &base {
        sc = sc.control(*p, 0);
    }
    // Two homeworlds, its own and a rival's: neither counts.
    let s = sc.clone().control(Planet::Nestphar, 0).control(Planet::Muaat, 0).state();
    assert_eq!(counting(&s, 0, false, NONE, O::ExpandBorders), Some(Progress { family: Family::NonHome, have: 6, threshold: 6 }));
    assert!(counting(&s, 0, false, NONE, O::ExpandBorders).unwrap().satisfied());
    let sub = counting(&s, 0, false, NONE, O::SubdueTheGalaxy).unwrap();
    assert_eq!((sub.have, sub.threshold, sub.satisfied()), (6, 11, false));
    // Taking one more planet (imagined) adds one, a homeworld or a space station adds nothing.
    let more = [pl(System::Quann)];
    assert_eq!(have(counting(&s, 0, false, Imagined { planets: &more, ..NONE }, O::ExpandBorders)), 7);
    let none = [Planet::Jord, Planet::OluzStation];
    assert_eq!(have(counting(&s, 0, false, Imagined { planets: &none, ..NONE }, O::ExpandBorders)), 6);
    // Out of the sources in play the planet is not counted: only the base game is in scope here.
    let tinnes = [Planet::Tinnes];
    let f = SeatFacts::of(&s, Scope(source_bit(ti4_engine_core::generated::Source::Base)), Seat(0), terms());
    let p = Pos::new(&s, &f, false, Imagined { planets: &tinnes, ..NONE });
    assert_eq!(q::counting_progress(&p, &SysSet::EMPTY, O::ExpandBorders).unwrap().have, 6);
    // The other seats do not hold them.
    assert_eq!(have(counting(&s, 1, false, NONE, O::ExpandBorders)), 0);
}

#[test]
fn corner_the_market_counts_the_most_common_trait_and_a_dual_planet_counts_for_both() {
    // Four cultural planets and three hazardous ones: four.
    let cultural = [System::Quann, System::Lodor, System::TequranTorkan, System::QucennRarron];
    let mut sc = table();
    for sys in cultural {
        sc = sc.control(sys.data().planets[sys.data().planets.len() - 1], 0);
    }
    // Torkan and Rarron are the second planets of their tiles and cultural.
    let sc = sc.control(pl(System::VefutII), 0).control(pl(System::MeharXull), 0).control(Planet::Tequran, 0);
    let s = sc.clone().state();
    assert_eq!(counting(&s, 0, false, NONE, O::CornerTheMarket), Some(Progress { family: Family::SameTrait, have: 4, threshold: 4 }));
    assert_eq!(have(counting(&s, 0, false, NONE, O::UnifyTheColonies)), 4);
    // Olergodt is hazardous and cultural: it is the fifth cultural planet and the fourth hazardous one.
    let dual = sc.clone().control(Planet::Olergodt, 0).state();
    assert_eq!(have(counting(&dual, 0, false, NONE, O::CornerTheMarket)), 5);
    // Two dual planets alone are two of each trait, not four of one.
    let two = table().control(Planet::Olergodt, 0).control(Planet::ViraPicsIII, 0).state();
    assert_eq!(have(counting(&two, 0, false, NONE, O::CornerTheMarket)), 2);
    // An imagined planet counts with its traits.
    let imagined = [pl(System::Thibah)]; // industrial
    let ind = table().control(pl(System::Saudor), 0).state();
    assert_eq!(have(counting(&ind, 0, false, Imagined { planets: &imagined, ..NONE }, O::CornerTheMarket)), 2);
    assert_eq!(have(counting(&ind, 0, false, NONE, O::CornerTheMarket)), 1);
    // A seat without planets: none.
    assert_eq!(have(counting(&ind, 1, false, NONE, O::CornerTheMarket)), 0);
}

#[test]
fn research_outposts_count_planets_with_a_specialty_printed_or_given_by_a_research_facility() {
    let s = table()
        .control(pl(System::Wellon), 0) // cybernetic
        .control(pl(System::Thibah), 0) // propulsion
        .control(pl(System::MeharXull), 0) // warfare
        .control(pl(System::Quann), 0) // none
        .control(Planet::Tiamat, 0) // two specialties on one planet: still one planet
        .state();
    assert_eq!(counting(&s, 0, false, NONE, O::FoundResearchOutposts), Some(Progress { family: Family::TechSpecialties, have: 4, threshold: 3 }));
    let mut with = s.clone();
    with.board_mut().planet_mut(pl(System::Quann)).attach.push(Attachment::BioticResearchFacility);
    assert_eq!(have(counting(&with, 0, false, NONE, O::FormGalacticBrainTrust)), 5);
    // An attachment without a specialty does not give one.
    let mut rich = s.clone();
    rich.board_mut().planet_mut(pl(System::Quann)).attach.push(Attachment::RichWorld);
    assert_eq!(have(counting(&rich, 0, false, NONE, O::FoundResearchOutposts)), 4);
    // An imagined planet with a specialty counts, one without does not.
    let im = [pl(System::Tarmann), pl(System::Saudor)];
    assert_eq!(have(counting(&s, 0, false, Imagined { planets: &im, ..NONE }, O::FoundResearchOutposts)), 5);
}

#[test]
fn reclaim_ancient_monuments_counts_the_controlled_planets_with_attachments() {
    let mut s = table().control(pl(System::Wellon), 0).control(pl(System::VefutII), 0).control(pl(System::Thibah), 0).state();
    s.board_mut().planet_mut(pl(System::Wellon)).attach.push(Attachment::RichWorld);
    s.board_mut().planet_mut(pl(System::VefutII)).attach.push(Attachment::DysonSphere);
    s.board_mut().planet_mut(pl(System::VefutII)).attach.push(Attachment::MiningWorld);
    // An attachment on a planet someone else controls is not ours.
    s.board_mut().planet_mut(pl(System::Quann)).attach.push(Attachment::RichWorld);
    s.board_mut().planet_mut(pl(System::Quann)).control = Some(Seat(1));
    assert_eq!(counting(&s, 0, false, NONE, O::DiscoverLostOutposts), Some(Progress { family: Family::AttachedPlanets, have: 2, threshold: 2 }));
    assert_eq!(counting(&s, 0, false, NONE, O::ReclaimAncientMonuments).unwrap().threshold, 3);
    assert_eq!(have(counting(&s, 0, false, NONE, O::ReclaimAncientMonuments)), 2, "two attachments on one planet are one planet");
    assert_eq!(have(counting(&s, 1, false, NONE, O::ReclaimAncientMonuments)), 1);
}

// ---- technology ---------------------------------------------------------------------------------

#[test]
fn unit_upgrades_and_colours_count_the_owned_technologies_and_the_imagined_new_ones() {
    let s = table()
        .tech(0, T::CruiserII)
        .tech(0, T::CarrierII)
        .tech(0, T::NeuralMotivator)
        .tech(0, T::DacxiveAnimators)
        .tech(0, T::GravityDrive)
        .tech(0, T::AntimassDeflectors)
        .tech(0, T::AssaultCannon)
        .state();
    assert_eq!(counting(&s, 0, false, NONE, O::DevelopWeaponry), Some(Progress { family: Family::UnitUpgrades, have: 2, threshold: 2 }));
    assert_eq!(counting(&s, 0, false, NONE, O::RevolutionizeWarfare).unwrap().threshold, 3);
    // Two biotic and two propulsion: two colours with two technologies; the single warfare one is not enough.
    assert_eq!(counting(&s, 0, false, NONE, O::DiversifyResearch), Some(Progress { family: Family::Colours { per_colour: 2 }, have: 2, threshold: 2 }));
    assert_eq!(counting(&s, 0, false, NONE, O::MasterTheSciences).unwrap().threshold, 4);
    assert_eq!(have(counting(&s, 0, false, NONE, O::MasterTheSciences)), 2);
    // Researching one more of each: a unit upgrade, a warfare technology (three colours now).
    let new = [T::DestroyerII, T::DuraniumArmor];
    let im = Imagined { technologies: &new, ..NONE };
    assert_eq!(have(counting(&s, 0, false, im, O::DevelopWeaponry)), 3);
    assert_eq!(have(counting(&s, 0, false, im, O::MasterTheSciences)), 3);
    // A technology already owned adds nothing.
    let owned = [T::CruiserII, T::NeuralMotivator];
    let im = Imagined { technologies: &owned, ..NONE };
    assert_eq!(have(counting(&s, 0, false, im, O::DevelopWeaponry)), 2);
    assert_eq!(have(counting(&s, 0, false, im, O::MasterTheSciences)), 2);
    // The unit upgrades are not a colour.
    let ups = table().tech(0, T::CruiserII).tech(0, T::CarrierII).tech(0, T::DestroyerII).state();
    assert_eq!(have(counting(&ups, 0, false, NONE, O::DiversifyResearch)), 0);
    assert_eq!(have(counting(&s, 1, false, NONE, O::DevelopWeaponry)), 0);
}

// ---- structures -----------------------------------------------------------------------------------

#[test]
fn build_defenses_counts_structures_and_improve_infrastructure_the_planets_away_from_home() {
    let (wellon, vefut) = (pl(System::Wellon), pl(System::VefutII));
    let mut s = table()
        .unit(System::Wellon, Some(wellon), Unit::SpaceDockI, 0, 1)
        .unit(System::VefutII, Some(vefut), Unit::PDSI, 0, 2)
        .unit(System::VefutII, Some(vefut), Unit::InfantryI, 0, 3)
        .unit(System::Thibah, None, Unit::FighterI, 0, 2)
        .unit(System::Thibah, Some(pl(System::Thibah)), Unit::SpaceDockI, 1, 1)
        .state();
    assert_eq!(counting(&s, 0, false, NONE, O::BuildDefenses), Some(Progress { family: Family::Structures, have: 3, threshold: 4 }));
    assert_eq!(counting(&s, 0, false, NONE, O::ConstructMassiveCities).unwrap().threshold, 7);
    // Home is a system of the table: the structures on it do not count as "away".
    s.players_mut().by_seat_mut(Seat(0)).unwrap().home_system = Some(System::Wellon);
    assert_eq!(counting(&s, 0, false, NONE, O::ImproveInfrastructure), Some(Progress { family: Family::StructuresAway, have: 1, threshold: 3 }));
    assert_eq!(counting(&s, 0, false, NONE, O::ProtectTheBorder).unwrap().threshold, 5);
    // Imagined structures count where the planet has none yet (Thibah), not on a planet that has one.
    let built = [(System::Thibah, pl(System::Thibah)), (System::VefutII, vefut), (System::Wellon, wellon)];
    let im = Imagined { structures: &built, ..NONE };
    assert_eq!(have(counting(&s, 0, false, im, O::BuildDefenses)), 4);
    assert_eq!(have(counting(&s, 0, false, im, O::ImproveInfrastructure)), 2);
    // Without the home record the faction's own home (Nestphar's tile) is "home": all three away count planets Wellon and Vefut.
    s.players_mut().by_seat_mut(Seat(0)).unwrap().home_system = None;
    assert_eq!(have(counting(&s, 0, false, NONE, O::ImproveInfrastructure)), 2);
}

// ---- ships and systems -------------------------------------------------------------------------------

#[test]
fn raise_a_fleet_counts_the_non_fighter_ships_in_the_fullest_system() {
    let s = table()
        .unit(System::Wellon, None, Unit::CruiserI, 0, 3)
        .unit(System::Wellon, None, Unit::DreadnoughtI, 0, 1)
        .unit(System::Wellon, None, Unit::FighterI, 0, 6)
        .unit(System::Wellon, None, Unit::CruiserI, 1, 5)
        .unit(System::VefutII, None, Unit::CarrierI, 0, 2)
        .unit(System::VefutII, Some(pl(System::VefutII)), Unit::InfantryI, 0, 4)
        .state();
    assert_eq!(counting(&s, 0, false, NONE, O::RaiseAFleet), Some(Progress { family: Family::FleetInOneSystem, have: 4, threshold: 5 }));
    assert_eq!(counting(&s, 0, false, NONE, O::CommandAnArmada).unwrap().threshold, 8);
    // One more ship in the fullest system, or in the other one (which stays behind).
    let there = [System::Wellon];
    assert_eq!(have(counting(&s, 0, false, Imagined { systems: &there, ..NONE }, O::RaiseAFleet)), 5);
    let other = [System::VefutII];
    assert_eq!(have(counting(&s, 0, false, Imagined { systems: &other, ..NONE }, O::RaiseAFleet)), 4);
    assert_eq!(have(counting(&s, 2, false, NONE, O::RaiseAFleet)), 0);
}

#[test]
fn explore_deep_space_counts_units_in_systems_without_planets_and_make_history_the_notable_ones() {
    let planetless = [System::EmptySystem46, System::EmptySystem47, System::Nebula, System::Supernova43];
    let mut sc = table().unit(System::Wellon, None, Unit::CruiserI, 0, 1).unit(System::VefutII, Some(pl(System::VefutII)), Unit::InfantryI, 0, 1);
    for sys in planetless {
        sc = sc.unit(sys, None, Unit::CruiserI, 0, 1);
    }
    let s = sc.state();
    assert_eq!(counting(&s, 0, false, NONE, O::ExploreDeepSpace), Some(Progress { family: Family::PlanetlessSystems, have: 4, threshold: 3 }));
    assert_eq!(counting(&s, 0, false, NONE, O::PatrolVastTerritories).unwrap().threshold, 5);
    let more = [System::EmptySystem48, System::Wellon];
    assert_eq!(have(counting(&s, 0, false, Imagined { systems: &more, ..NONE }, O::PatrolVastTerritories)), 5);
    assert_eq!(have(counting(&s, 1, false, NONE, O::ExploreDeepSpace)), 0);

    // Notable: Mecatol Rex, every kind of anomaly, a legendary planet. Not an empty system, a plain one.
    let notable = [System::MecatolRex18, System::Nebula, System::Supernova43, System::EntropicScar, System::Asteroids44, System::GravityRift, System::Tempesta];
    let mut sc = table().unit(System::EmptySystem46, None, Unit::CruiserI, 0, 1).unit(System::Wellon, None, Unit::CruiserI, 0, 1);
    for sys in notable {
        sc = sc.unit(sys, None, Unit::CruiserI, 0, 1);
    }
    let s = sc.state();
    assert_eq!(counting(&s, 0, false, NONE, O::MakeHistory), Some(Progress { family: Family::NotableSystems, have: 7, threshold: 2 }));
    assert_eq!(counting(&s, 0, false, NONE, O::BecomeALegend).unwrap().threshold, 4);
    let one = table().unit(System::EmptySystem46, None, Unit::CruiserI, 0, 3).unit(System::Wellon, None, Unit::CruiserI, 0, 1).unit(System::Emelpar, Some(Planet::Emelpar), Unit::InfantryI, 0, 1).state();
    assert_eq!(have(counting(&one, 0, false, NONE, O::MakeHistory)), 1, "the legendary planet's tile (units on its planet) is the one");
    let go = [System::Nebula, System::EmptySystem47];
    assert_eq!(have(counting(&one, 0, false, Imagined { systems: &go, ..NONE }, O::MakeHistory)), 2);
}

#[test]
fn populate_the_outer_rim_counts_the_units_on_the_edge_of_the_map_other_than_home() {
    let systems = plain_systems();
    let s = table()
        .map_ring(7)
        .unit(systems[1], None, Unit::CruiserI, 0, 1) // the edge
        .unit(systems[2], None, Unit::FighterI, 0, 1)
        .unit(systems[3], Some(pl(systems[3])), Unit::InfantryI, 0, 1) // on a planet
        .unit(systems[0], None, Unit::CruiserI, 0, 4) // the middle is not the edge
        .state();
    assert_eq!(counting(&s, 0, true, NONE, O::PopulateTheOuterRim), Some(Progress { family: Family::OnTheRim, have: 3, threshold: 3 }));
    assert_eq!(counting(&s, 0, true, NONE, O::ControlTheBorderlands).unwrap().threshold, 5);
    assert!(counting(&s, 0, false, NONE, O::PopulateTheOuterRim).is_none(), "no map, no answer");
    // Our own home system does not count.
    let mut at_home = s.clone();
    at_home.players_mut().by_seat_mut(Seat(0)).unwrap().home_system = Some(systems[1]);
    assert_eq!(have(counting(&at_home, 0, true, NONE, O::PopulateTheOuterRim)), 2);
    // Moving to another edge system counts, to the middle or to a system already held does not.
    let go = [systems[4], systems[0], systems[1]];
    assert_eq!(have(counting(&s, 0, true, Imagined { systems: &go, ..NONE }, O::PopulateTheOuterRim)), 4);
}

// ---- remaining position ----------------------------------------------------------------------------------

#[test]
fn conquer_the_weak_counts_the_planets_of_rival_homes() {
    let s = table().control(Planet::Muaat, 0).control(Planet::Retillion, 0).control(Planet::Nestphar, 0).control(pl(System::Wellon), 0).state();
    assert_eq!(remaining(&s, 0, false, NONE, O::ConquerTheWeak), Some(Progress { family: Family::RivalHomePlanets, have: 2, threshold: 1 }));
    // The other seat's home record overrides the faction's planets.
    let mut moved = s.clone();
    moved.players_mut().by_seat_mut(Seat(1)).unwrap().home_planets.push(pl(System::Wellon));
    assert_eq!(have(remaining(&moved, 0, false, NONE, O::ConquerTheWeak)), 2, "Wellon replaces Muaat, Retillion stays");
    let im = [Planet::Shalloq];
    assert_eq!(have(remaining(&s, 0, false, Imagined { planets: &im, ..NONE }, O::ConquerTheWeak)), 3);
    assert_eq!(have(remaining(&s, 1, false, NONE, O::ConquerTheWeak)), 0);
}

#[test]
fn engineer_a_marvel_and_achieve_supremacy_count_the_flagship_and_war_sun_systems() {
    let s = table()
        .unit(System::Wellon, None, Unit::FactionlessFlagship, 0, 1)
        .unit(System::Wellon, None, Unit::PrototypeWarSunI, 0, 1)
        .unit(System::VefutII, None, Unit::PrototypeWarSunI, 0, 1)
        .unit(System::MuaatMuaat, None, Unit::FactionlessFlagship, 0, 1) // a rival's home
        .unit(System::MecatolRex18, None, Unit::PrototypeWarSunI, 0, 1)
        .unit(System::NestpharArborec, None, Unit::FactionlessFlagship, 0, 1) // our own home
        .unit(System::Thibah, None, Unit::DreadnoughtI, 0, 3)
        .state();
    assert_eq!(remaining(&s, 0, false, NONE, O::EngineerAMarvel), Some(Progress { family: Family::CapitalShipSystems, have: 5, threshold: 1 }));
    assert_eq!(remaining(&s, 0, false, NONE, O::AchieveSupremacy), Some(Progress { family: Family::CapitalShipsInRivalHomeOrMecatol, have: 2, threshold: 1 }));
    let none = table().unit(System::Thibah, None, Unit::DreadnoughtI, 0, 3).state();
    assert!(!remaining(&none, 0, false, NONE, O::EngineerAMarvel).unwrap().satisfied());
    assert_eq!(have(remaining(&s, 2, false, NONE, O::AchieveSupremacy)), 0);
    // Thunder's Edge's Mecatol tile counts when it is the one on the board.
    let mut te = table().unit(System::MecatolRex112, None, Unit::FactionlessFlagship, 0, 1).state();
    assert_eq!(have(remaining(&te, 0, false, NONE, O::AchieveSupremacy)), 0, "the board does not hold the new tile yet");
    te.board_mut().sys_mut(System::MecatolRex112).flags.set(ti4_engine_core::state::SysFlags::TOUCHED, true);
    assert_eq!(have(remaining(&te, 0, false, NONE, O::AchieveSupremacy)), 1);
}

fn hub() -> (Scenario, Vec<System>) {
    let mut systems = vec![System::MecatolRex18];
    systems.extend(plain_systems().into_iter().filter(|s| !matches!(s, System::MecatolRex18 | System::MecatolRex112)).take(18));
    (table().map_from(&systems), systems)
}

#[test]
fn intimidate_the_council_counts_the_systems_with_ships_next_to_mecatol() {
    let (sc, sys) = hub();
    let s = sc
        .unit(sys[1], None, Unit::CruiserI, 0, 3)
        .unit(sys[2], None, Unit::FighterI, 0, 1)
        .unit(sys[3], Some(pl(sys[3])), Unit::InfantryI, 0, 4) // ground forces are not ships
        .unit(System::MecatolRex18, None, Unit::CruiserI, 0, 2) // Mecatol itself is not adjacent to itself
        .unit(sys[10], None, Unit::CruiserI, 0, 2) // the second ring
        .state();
    assert_eq!(remaining(&s, 0, true, NONE, O::IntimidateCouncil), Some(Progress { family: Family::ShipsAdjacentToMecatol, have: 2, threshold: 2 }));
    assert!(remaining(&s, 0, false, NONE, O::IntimidateCouncil).is_none());
    let go = [sys[4], sys[10], sys[1]];
    assert_eq!(have(remaining(&s, 0, true, Imagined { systems: &go, ..NONE }, O::IntimidateCouncil)), 3);
    assert_eq!(have(remaining(&s, 1, true, NONE, O::IntimidateCouncil)), 0);
}

#[test]
fn push_boundaries_counts_the_neighbours_with_fewer_planets() {
    let (sc, sys) = hub();
    // Seat 0 is in the middle; seats 1 and 2 beside it.
    let sc = sc
        .unit(System::MecatolRex18, None, Unit::CruiserI, 0, 1)
        .unit(sys[1], None, Unit::CruiserI, 1, 1)
        .unit(sys[2], None, Unit::CruiserI, 2, 1)
        .control(pl(sys[3]), 0)
        .control(pl(sys[4]), 0)
        .control(pl(sys[5]), 0)
        .control(pl(sys[6]), 1)
        .control(pl(sys[7]), 1)
        .control(pl(sys[8]), 2)
        .control(pl(sys[9]), 2)
        .control(pl(sys[10]), 2)
        .control(pl(sys[11]), 2);
    let s = sc.state();
    // Three planets against two and four: ahead of one neighbour.
    assert_eq!(remaining(&s, 0, true, NONE, O::PushBoundaries), Some(Progress { family: Family::WeakerNeighbours, have: 1, threshold: 2 }));
    let more = [pl(sys[12]), pl(sys[13])];
    let ahead = Imagined { planets: &more, ..NONE };
    assert_eq!(have(remaining(&s, 0, true, ahead, O::PushBoundaries)), 2, "five planets beat both");
    let one = [pl(sys[12])];
    assert_eq!(have(remaining(&s, 0, true, Imagined { planets: &one, ..NONE }, O::PushBoundaries)), 1, "four against four is not more");
    assert!(remaining(&s, 0, false, NONE, O::PushBoundaries).is_none());
}

#[test]
fn rule_distant_lands_counts_the_rival_homes_whose_system_or_neighbours_we_hold() {
    let (sc, sys) = hub();
    let ring = |i: usize| sys[i];
    let g = sc.clone().state();
    let adj = g.galaxy().unwrap().adjacent(ring(1));
    let far = (2..7).find(|i| !adj.contains(sys[*i].index())).expect("a ring tile that is not beside the first");
    let mut s = sc.control(pl(System::MecatolRex18), 0).control(pl(ring(1)), 2).state();
    s.players_mut().by_seat_mut(Seat(1)).unwrap().home_system = Some(ring(1));
    s.players_mut().by_seat_mut(Seat(2)).unwrap().home_system = Some(ring(far));
    // Mecatol is beside every ring tile: both homes are reached.
    assert_eq!(remaining(&s, 0, true, NONE, O::RuleDistantLands), Some(Progress { family: Family::DistinctRivalHomeReaches, have: 2, threshold: 2 }));
    // Holding a planet in the first home itself reaches only that home.
    let mut in_home = s.clone();
    in_home.board_mut().planet_mut(pl(System::MecatolRex18)).control = None;
    in_home.board_mut().planet_mut(pl(ring(1))).control = Some(Seat(0));
    assert_eq!(have(remaining(&in_home, 0, true, NONE, O::RuleDistantLands)), 1);
    assert!(remaining(&in_home, 0, false, NONE, O::RuleDistantLands).is_none());
    // Our own home is no rival's.
    let mut own = in_home.clone();
    own.players_mut().by_seat_mut(Seat(0)).unwrap().home_system = Some(ring(far));
    assert_eq!(have(remaining(&own, 0, true, NONE, O::RuleDistantLands)), 1);
}

// ---- neighbours ----------------------------------------------------------------------------------------------

#[test]
fn neighbours_are_the_seats_whose_presence_is_in_or_next_to_ours_and_iihq_widens_it_to_mecatol() {
    let (sc, sys) = hub();
    let probe = sc.clone().state();
    let g = probe.galaxy().unwrap();
    let ring = g.adjacent(System::MecatolRex18);
    // Two ring tiles that are not beside each other (and not the same).
    let (a, b) = (1..7)
        .flat_map(|i| (1..7).map(move |j| (i, j)))
        .find(|(i, j)| i != j && !g.adjacent(sys[*i]).contains(sys[*j].index()))
        .expect("two ring tiles apart");
    assert!(ring.contains(sys[a].index()) && ring.contains(sys[b].index()));
    let s = sc
        .unit(sys[a], None, Unit::CruiserI, 0, 1)
        .unit(sys[b], None, Unit::CruiserI, 1, 1) // two tiles apart: not neighbours
        .unit(sys[a], None, Unit::CruiserI, 2, 1) // the same tile: neighbours
        .state();
    let list = |s: &CoreState, t: &Terms, seat: u8| q::neighbours(s, t, Seat(seat)).as_slice().to_vec();
    assert_eq!(list(&s, &terms(), 0), vec![Seat(2)]);
    assert_eq!(list(&s, &terms(), 2), vec![Seat(0)]);
    assert_eq!(list(&s, &terms(), 1), Vec::<Seat>::new());
    // Beside each other: planets controlled count as presence.
    let mut s2 = s.clone();
    let next = sys.iter().enumerate().skip(1).take(18).find(|(i, x)| *i != a && g.adjacent(**x).contains(sys[b].index())).map(|(_, x)| *x).expect("a tile beside b");
    s2.board_mut().planet_mut(pl(next)).control = Some(Seat(0));
    assert_eq!(list(&s2, &terms(), 1), vec![Seat(0)]);
    // I.I.H.Q. Modernization: the holder is a neighbour of whoever is near Mecatol, and the other way round.
    let mut with = terms();
    with.iihq.insert(Seat(0));
    assert_eq!(list(&s, &with, 0), vec![Seat(1), Seat(2)], "seat 1 is next to Mecatol");
    assert_eq!(list(&s, &with, 1), vec![Seat(0)], "seat 0 holds the Modernization and is next to Mecatol");
    // Far from Mecatol (two rings out) the Modernization does nothing.
    let far = (7..19)
        .map(|i| sys[i])
        .find(|x| *x != sys[a] && !g.adjacent(*x).contains(System::MecatolRex18.index()) && !g.adjacent(*x).contains(sys[a].index()))
        .expect("a far tile");
    let farther = table().map_from(&sys).unit(far, None, Unit::CruiserI, 1, 1).unit(sys[a], None, Unit::CruiserI, 0, 1).state();
    assert!(list(&farther, &with, 0).is_empty(), "the Modernization reaches only what is next to Mecatol");
    let on_mecatol = table().map_from(&sys).unit(System::MecatolRex18, None, Unit::CruiserI, 1, 1).unit(far, None, Unit::CruiserI, 0, 1).state();
    assert_eq!(list(&on_mecatol, &with, 0), vec![Seat(1)], "a unit on Mecatol itself is near Mecatol");
    assert!(list(&on_mecatol, &terms(), 0).is_empty());
}

// ---- the secret objectives ---------------------------------------------------------------------------------

#[test]
fn the_secret_rows_are_the_printed_thresholds_and_two_of_them_are_not_the_cores() {
    let rows = [
        (S::EstablishAPerimeter, Family::Units(ti4_engine_core::generated::UnitBaseType::Pds), 4),
        (S::FuelTheWarMachine, Family::Units(ti4_engine_core::generated::UnitBaseType::Spacedock), 3),
        (S::GatherAMightyFleet, Family::Units(ti4_engine_core::generated::UnitBaseType::Dreadnought), 5),
        (S::ControlTheRegion, Family::ShipsInSystems, 6),
        (S::MasterTheLawsOfPhysics, Family::SameColourTechnologies, 4),
        (S::MechanizeTheMilitary, Family::MechsOnDistinctPlanets, 4),
        (S::OccupyTheFringe, Family::GroundForcesOnOnePlanet, 9),
        (S::FormASpyNetwork, Family::ActionCards, 5),
        (S::LearnTheSecretsOfTheCosmos, Family::ShipSystemsBesideAnomaly, 3),
        (S::ThreatenEnemies, Family::ShipSystemsBesideRivalHome, 1),
        (S::AdaptNewStrategies, Family::FactionTechnologies, 2),
        (S::DictatePolicy, Family::Laws, 3),
        (S::StakeYourClaim, Family::SharedPlanetSystems, 1),
        (S::SeizeAnIcon, Family::LegendaryPlanets, 1),
        (S::OccupyTheSeatOfTheEmpire, Family::MecatolShipsWhileControlling, 3),
        (S::EstablishHegemony, Family::CombinedValue(Spend::Influence), 12),
        (S::HoardRawMaterials, Family::CombinedValue(Spend::Resources), 12),
    ];
    let s = table().state();
    for (o, family, threshold) in rows {
        let p = secret(&s, 0, true, NONE, o).unwrap_or_else(|| panic!("{o:?} answered"));
        assert_eq!((p.family, p.threshold), (family, threshold), "{o:?}");
    }
    for (o, kind, threshold) in [
        (S::ForgeAnAlliance, ti4_engine_core::generated::PlanetTrait::Cultural, 4),
        (S::MineRareMetals, ti4_engine_core::generated::PlanetTrait::Hazardous, 4),
        (S::MonopolizeProduction, ti4_engine_core::generated::PlanetTrait::Industrial, 4),
    ] {
        let p = secret(&s, 0, true, NONE, o).unwrap();
        assert_eq!((p.family, p.threshold), (Family::PlanetsOfTrait(kind), threshold), "{o:?}");
    }
    // A secret of the other windows (not a position one) is not answered, and two read what the core cannot see.
    assert!(secret(&s, 0, true, NONE, S::BetrayAFriend).is_none());
    assert!(secret(&s, 0, true, NONE, S::BraveTheVoid).is_none());
    let f = facts(&s, 0);
    let pos = Pos::new(&s, &f, true, NONE);
    assert_eq!(q::secret_progress(&pos, S::StrengthenBonds), Err(q::Unsupported));
    assert_eq!(q::secret_progress(&pos, S::ProduceEnMasse), Err(q::Unsupported));
    let mut unmapped = terms();
    unmapped.unmapped = true;
    let f = SeatFacts::of(&s, ALL_SOURCES, Seat(0), unmapped);
    assert_eq!(q::secret_progress(&Pos::new(&s, &f, true, NONE), S::SeizeAnIcon), Err(q::Unsupported));
    assert!(q::secret_points(S::SeizeAnIcon) == 1 && S::ALL.iter().all(|o| q::secret_points(*o) == 1));
}

#[test]
fn unit_secrets_count_the_seats_own_units_of_that_kind_wherever_they_stand() {
    let s = table()
        .unit(System::Wellon, Some(pl(System::Wellon)), Unit::PDSI, 0, 2)
        .unit(System::VefutII, Some(pl(System::VefutII)), Unit::PDSI, 0, 1)
        .unit(System::VefutII, Some(pl(System::VefutII)), Unit::PDSI, 1, 5)
        .unit(System::VefutII, Some(pl(System::VefutII)), Unit::SpaceDockI, 0, 1)
        .unit(System::Thibah, None, Unit::DreadnoughtI, 0, 3)
        .unit(System::Tarmann, None, Unit::DreadnoughtI, 0, 1)
        .unit(System::Tarmann, None, Unit::CruiserI, 0, 4)
        .state();
    assert_eq!(have(secret(&s, 0, false, NONE, S::EstablishAPerimeter)), 3);
    assert_eq!(have(secret(&s, 1, false, NONE, S::EstablishAPerimeter)), 5);
    assert_eq!(have(secret(&s, 0, false, NONE, S::FuelTheWarMachine)), 1);
    assert_eq!(have(secret(&s, 0, false, NONE, S::GatherAMightyFleet)), 4);
    assert_eq!(have(secret(&s, 2, false, NONE, S::GatherAMightyFleet)), 0);
}

#[test]
fn control_the_region_counts_the_systems_with_a_ship_and_the_imagined_one_once() {
    let sc = table()
        .unit(System::Wellon, None, Unit::CruiserI, 0, 3)
        .unit(System::VefutII, None, Unit::FighterI, 0, 1) // a fighter is a ship
        .unit(System::Thibah, Some(pl(System::Thibah)), Unit::InfantryI, 0, 2) // a ground force is not
        .unit(System::Tarmann, None, Unit::CruiserI, 1, 1);
    let s = sc.state();
    assert_eq!(have(secret(&s, 0, false, NONE, S::ControlTheRegion)), 2);
    let go = [System::Saudor, System::Wellon, System::Saudor];
    assert_eq!(have(secret(&s, 0, false, Imagined { systems: &go, ..NONE }, S::ControlTheRegion)), 3, "a system moved to twice, and one already held, are not counted again");
}

#[test]
fn planets_of_a_trait_are_the_controlled_the_imagined_and_the_borrowed_ones_once_each() {
    // Cultural planets: Quann, Lodor, Torkan, Rarron.
    let sc = table().control(pl(System::Quann), 0).control(pl(System::Lodor), 0).control(Planet::Torkan, 0);
    let s = sc.clone().state();
    assert_eq!(have(secret(&s, 0, false, NONE, S::ForgeAnAlliance)), 3);
    assert_eq!(have(secret(&s, 0, false, NONE, S::MineRareMetals)), 0);
    // The imagined one is added, an imagined planet that is held already is not.
    let im = [Planet::Rarron, pl(System::Quann)];
    assert_eq!(have(secret(&s, 0, false, Imagined { planets: &im, ..NONE }, S::ForgeAnAlliance)), 4);
    // The Firmament commander lets the seat treat a planet of someone else as its own.
    let mut t = terms();
    t.firmament.insert(Planet::Rarron);
    t.firmament.insert(Planet::Torkan);
    assert_eq!(have(secret_with(&s, 0, t, false, NONE, S::ForgeAnAlliance)), 4);
    // Dual traits count for both; a planet out of the sources in play counts for none.
    let dual = sc.control(Planet::Olergodt, 0).state();
    assert_eq!(have(secret(&dual, 0, false, NONE, S::ForgeAnAlliance)), 4);
    assert_eq!(have(secret(&dual, 0, false, NONE, S::MineRareMetals)), 1);
    let f = SeatFacts::of(&dual, Scope(source_bit(ti4_engine_core::generated::Source::Base)), Seat(0), terms());
    let p = q::secret_progress(&Pos::new(&dual, &f, false, NONE), S::MineRareMetals).unwrap().unwrap();
    assert_eq!(p.have, 0, "Olergodt is a Thunder's Edge planet");
    let ind = table().control(pl(System::Saudor), 0).control(pl(System::Thibah), 0).state();
    assert_eq!(have(secret(&ind, 0, false, NONE, S::MonopolizeProduction)), 2);
}

#[test]
fn master_the_laws_of_physics_counts_the_largest_colour_with_a_faction_technology_in_its_first_colour() {
    let s = table().tech(0, T::BioStims).tech(0, T::DacxiveAnimators).tech(0, T::HyperMetabolism).tech(0, T::Bioplasmosis).tech(0, T::GravityDrive).tech(0, T::CruiserII).state();
    assert_eq!(have(secret(&s, 0, false, NONE, S::MasterTheLawsOfPhysics)), 4, "three generic and one faction biotic technology");
    assert_eq!(have(secret(&s, 1, false, NONE, S::MasterTheLawsOfPhysics)), 0);
}

#[test]
fn ground_secrets_count_the_planets_with_a_mech_and_the_biggest_army_on_one_planet() {
    let (wellon, vefut) = (pl(System::Wellon), pl(System::VefutII));
    let s = table()
        .unit(System::Wellon, Some(wellon), Unit::InfantryI, 0, 5)
        .unit(System::Wellon, Some(wellon), Unit::FactionlessMech, 0, 2)
        .unit(System::Wellon, Some(wellon), Unit::PDSI, 0, 3)
        .unit(System::VefutII, Some(vefut), Unit::InfantryI, 0, 3)
        .unit(System::VefutII, Some(vefut), Unit::FactionlessMech, 0, 1)
        .unit(System::Thibah, Some(pl(System::Thibah)), Unit::InfantryI, 1, 20)
        .state();
    assert_eq!(have(secret(&s, 0, false, NONE, S::MechanizeTheMilitary)), 2, "two mechs on one planet are one planet");
    assert_eq!(have(secret(&s, 0, false, NONE, S::OccupyTheFringe)), 7, "infantry and mechs, not PDS");
    assert_eq!(have(secret(&s, 1, false, NONE, S::OccupyTheFringe)), 20);
    assert_eq!(have(secret(&s, 1, false, NONE, S::MechanizeTheMilitary)), 0);
    // A Titan's PDS is a ground force.
    let titan = table().unit(System::Wellon, Some(wellon), Unit::HelTitanI, 0, 2).unit(System::Wellon, Some(wellon), Unit::InfantryI, 0, 1).state();
    assert_eq!(have(secret(&titan, 0, false, NONE, S::OccupyTheFringe)), 3);
}

#[test]
fn cut_supply_lines_counts_the_systems_with_our_ship_and_a_rival_space_dock() {
    let s = table()
        .unit(System::Wellon, Some(pl(System::Wellon)), Unit::SpaceDockI, 1, 1)
        .unit(System::Wellon, None, Unit::CruiserI, 0, 1) // ship and a rival dock
        .unit(System::VefutII, Some(pl(System::VefutII)), Unit::SpaceDockI, 0, 1)
        .unit(System::VefutII, None, Unit::CruiserI, 0, 1) // our own dock
        .unit(System::Thibah, Some(pl(System::Thibah)), Unit::SpaceDockI, 2, 1) // no ship of ours
        .unit(System::Thibah, Some(pl(System::Thibah)), Unit::InfantryI, 0, 3)
        .unit(System::Tarmann, None, Unit::CruiserI, 0, 1) // ship and no dock
        .unit(System::Saudor, Some(pl(System::Saudor)), Unit::PDSI, 1, 1) // a rival PDS is not a dock
        .unit(System::Saudor, None, Unit::CruiserI, 0, 1)
        .state();
    assert_eq!(secret(&s, 0, false, NONE, S::CutSupplyLines), Some(Progress { family: Family::RivalDockSystemsWithShips, have: 1, threshold: 1 }));
}

#[test]
fn relic_fragments_and_action_cards_are_counted_from_the_hand_and_the_stack() {
    let mut s = table().hand(0, &[ActionCard::ALL[1], ActionCard::ALL[2], ActionCard::ALL[3], ActionCard::ALL[4]]).state();
    assert_eq!(have(secret(&s, 0, false, NONE, S::FormASpyNetwork)), 4);
    assert_eq!(have(secret(&s, 1, false, NONE, S::FormASpyNetwork)), 0);
    let f = &mut s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.fragments;
    f.set(ExploreType::Cultural, 2);
    f.set(ExploreType::Hazardous, 1);
    f.set(ExploreType::Frontier, 0);
    f.set(ExploreType::Industrial, 3);
    assert_eq!(have(secret(&s, 0, false, NONE, S::DestroyHereticalWorks)), 6, "the stacks of every kind added, an empty one adds nothing");
    assert_eq!(have(secret(&s, 1, false, NONE, S::DestroyHereticalWorks)), 0);
}

fn ring_with(first: System) -> (Scenario, Vec<System>) {
    let mut sys = vec![first];
    sys.extend(plain_systems().into_iter().filter(|s| *s != first && !matches!(s, System::MecatolRex18 | System::MecatolRex112)).take(6));
    (table().map_from(&sys), sys)
}

#[test]
fn learn_the_secrets_counts_the_ship_systems_next_to_an_anomaly() {
    let (sc, sys) = ring_with(System::Nebula);
    let s = sc.unit(sys[1], None, Unit::CruiserI, 0, 1).unit(sys[2], None, Unit::FighterI, 0, 1).unit(sys[3], Some(pl(sys[3])), Unit::InfantryI, 0, 1).unit(System::Nebula, None, Unit::CruiserI, 0, 1).state();
    // The ring tiles are all next to the nebula; the infantry is no ship and the nebula is not next to itself.
    assert_eq!(secret(&s, 0, true, NONE, S::LearnTheSecretsOfTheCosmos), Some(Progress { family: Family::ShipSystemsBesideAnomaly, have: 2, threshold: 3 }));
    assert!(secret(&s, 0, false, NONE, S::LearnTheSecretsOfTheCosmos).is_none(), "no map: the string engine answers");
    // Beside only plain tiles: nothing. A legendary planet is no anomaly.
    let (sc, sys) = ring_with(System::Tempesta);
    let plain = sc.unit(sys[1], None, Unit::CruiserI, 0, 1).state();
    assert_eq!(have(secret(&plain, 0, true, NONE, S::LearnTheSecretsOfTheCosmos)), 0);
    // Each kind of anomaly counts.
    for anomaly in [System::Supernova43, System::EntropicScar, System::Asteroids44, System::GravityRift] {
        let (sc, sys) = ring_with(anomaly);
        let s = sc.unit(sys[1], None, Unit::CruiserI, 0, 1).state();
        assert_eq!(have(secret(&s, 0, true, NONE, S::LearnTheSecretsOfTheCosmos)), 1, "{anomaly:?}");
    }
}

#[test]
fn threaten_enemies_counts_the_ship_systems_next_to_a_homeworld_that_is_not_ours() {
    let (sc, sys) = ring_with(System::MuaatMuaat);
    let s = sc.unit(sys[1], None, Unit::CruiserI, 0, 1).unit(sys[2], None, Unit::CruiserI, 0, 1).unit(System::MuaatMuaat, None, Unit::CruiserI, 0, 1).state();
    let mut t = terms();
    t.faction_tag = Some(FactionTag::Arborec);
    assert_eq!(secret_with(&s, 0, t.clone(), true, NONE, S::ThreatenEnemies), Some(Progress { family: Family::ShipSystemsBesideRivalHome, have: 2, threshold: 1 }));
    // It is ours: it does not count. Without a map the string engine answers.
    t.faction_tag = Some(FactionTag::Muaat);
    assert_eq!(have(secret_with(&s, 0, t.clone(), true, NONE, S::ThreatenEnemies)), 0);
    assert!(secret_with(&s, 0, t, false, NONE, S::ThreatenEnemies).is_none());
    // Next to an ordinary system: nothing.
    let (sc, sys) = ring_with(System::Wellon);
    let plain = sc.unit(sys[1], None, Unit::CruiserI, 0, 1).state();
    assert_eq!(have(secret(&plain, 0, true, NONE, S::ThreatenEnemies)), 0);
}

#[test]
fn foster_cohesion_needs_every_other_player_as_a_neighbour() {
    let (sc, sys) = hub();
    let s = sc.unit(System::MecatolRex18, None, Unit::CruiserI, 0, 1).unit(sys[1], None, Unit::CruiserI, 1, 1).unit(sys[2], None, Unit::CruiserI, 2, 1).state();
    let p = secret(&s, 0, true, NONE, S::FosterCohesion).unwrap();
    assert_eq!((p.family, p.have, p.threshold), (Family::Neighbours, 2, 2));
    assert!(p.satisfied());
    // Seat 2 gone from the middle's reach: one of two.
    let g = s.galaxy().unwrap();
    let far = (7..19).map(|i| sys[i]).find(|x| !g.adjacent(*x).contains(System::MecatolRex18.index()) && !g.adjacent(*x).contains(sys[1].index())).expect("a far tile");
    let apart = table().map_from(&sys).unit(System::MecatolRex18, None, Unit::CruiserI, 0, 1).unit(sys[1], None, Unit::CruiserI, 1, 1).unit(far, None, Unit::CruiserI, 2, 1).state();
    let p = secret(&apart, 0, true, NONE, S::FosterCohesion).unwrap();
    assert_eq!((p.have, p.threshold), (1, 2));
    assert!(!p.satisfied());
    assert!(secret(&s, 0, false, NONE, S::FosterCohesion).is_none());
    // Alone at the table: nobody to be the neighbour of.
    let alone = Scenario::new(&[Faction::TheArborec]).map_from(&sys).state();
    assert!(secret(&alone, 0, true, NONE, S::FosterCohesion).is_none());
}

#[test]
fn defy_space_and_time_counts_our_units_in_the_nexus() {
    let s = table()
        .unit(System::MalliceNexus, None, Unit::CruiserI, 0, 2)
        .unit(System::MalliceNexus, None, Unit::FighterI, 0, 1)
        .unit(System::MalliceNexus, None, Unit::CruiserI, 1, 4)
        .unit(System::MalliceLockedNexus, None, Unit::CruiserI, 0, 1)
        .unit(System::Wellon, None, Unit::CruiserI, 0, 5)
        .state();
    assert_eq!(have(secret(&s, 0, false, NONE, S::DefySpaceAndTime)), 4, "both faces of the system are looked at");
    assert_eq!(have(secret(&s, 1, false, NONE, S::DefySpaceAndTime)), 4);
    assert_eq!(have(secret(&s, 2, false, NONE, S::DefySpaceAndTime)), 0);
    let f = SeatFacts::of(&s, Scope(source_bit(ti4_engine_core::generated::Source::Base)), Seat(0), terms());
    let p = q::secret_progress(&Pos::new(&s, &f, false, NONE), S::DefySpaceAndTime).unwrap().unwrap();
    assert_eq!(p.have, 0, "the Nexus is a Prophecy of Kings tile");
}

#[test]
fn become_the_gatekeeper_counts_alpha_and_beta_among_the_systems_with_our_ships() {
    let s = table().unit(System::AlphaWormhole, None, Unit::CruiserI, 0, 1).state();
    assert_eq!(secret(&s, 0, false, NONE, S::BecomeTheGatekeeper), Some(Progress { family: Family::WormholeKinds, have: 1, threshold: 2 }));
    let both = table().unit(System::AlphaWormhole, None, Unit::CruiserI, 0, 1).unit(System::BetaWormhole, None, Unit::FighterI, 0, 1).state();
    assert_eq!(have(secret(&both, 0, false, NONE, S::BecomeTheGatekeeper)), 2);
    // Two systems of one kind are one kind; ground forces on planets are no ships.
    let twice = table().unit(System::AlphaWormhole, None, Unit::CruiserI, 0, 1).unit(System::AsteroidsAlphaWormhole, None, Unit::CruiserI, 0, 1).unit(System::Wellon, None, Unit::CruiserI, 0, 1).state();
    assert_eq!(have(secret(&twice, 0, false, NONE, S::BecomeTheGatekeeper)), 1);
    let rival = table().unit(System::AlphaWormhole, None, Unit::CruiserI, 1, 1).state();
    assert_eq!(have(secret(&rival, 0, false, NONE, S::BecomeTheGatekeeper)), 0);
    // Scope: the Prophecy of Kings alpha tile is not in a base-only game.
    let f = SeatFacts::of(&s, Scope(source_bit(ti4_engine_core::generated::Source::Pok)), Seat(0), terms());
    assert_eq!(q::secret_progress(&Pos::new(&s, &f, false, NONE), S::BecomeTheGatekeeper).unwrap().unwrap().have, 0);
}

#[test]
fn adapt_new_strategies_dictate_policy_and_the_icon_secrets_read_the_technologies_the_laws_and_the_legends() {
    let mut s = table().tech(0, T::Bioplasmosis).tech(0, T::ChaosMapping).tech(0, T::BioStims).tech(0, T::CruiserII).tech(0, T::CrimsonLegionnaireII).state();
    assert_eq!(have(secret(&s, 0, false, NONE, S::AdaptNewStrategies)), 3, "faction technologies of every kind, generic ones not");
    assert_eq!(have(secret(&s, 1, false, NONE, S::AdaptNewStrategies)), 0);
    assert_eq!(have(secret(&s, 0, false, NONE, S::DictatePolicy)), 0);
    s.agenda_mut().laws.insert(Agenda::FleetRegulations, Outcome::Empty);
    let others: Vec<Agenda> = Agenda::ALL.iter().copied().filter(|a| *a != Agenda::FleetRegulations).take(2).collect();
    for a in others {
        s.agenda_mut().laws.insert(a, Outcome::Empty);
    }
    assert_eq!(secret(&s, 0, false, NONE, S::DictatePolicy), Some(Progress { family: Family::Laws, have: 3, threshold: 3 }));
    // Seize an Icon: a legendary planet, own, imagined or borrowed.
    let icon = table().control(Planet::Tempesta, 0).control(pl(System::Wellon), 0).control(Planet::Emelpar, 1).state();
    assert_eq!(secret(&icon, 0, false, NONE, S::SeizeAnIcon), Some(Progress { family: Family::LegendaryPlanets, have: 1, threshold: 1 }));
    let im = [Planet::Faunus, Planet::Tempesta];
    assert_eq!(have(secret(&icon, 0, false, Imagined { planets: &im, ..NONE }, S::SeizeAnIcon)), 2);
    let mut t = terms();
    t.firmament.insert(Planet::Emelpar);
    assert_eq!(have(secret_with(&icon, 0, t, false, NONE, S::SeizeAnIcon)), 2);
    assert_eq!(have(secret(&icon, 2, false, NONE, S::SeizeAnIcon)), 0);
}

#[test]
fn stake_your_claim_needs_a_planet_in_a_system_where_another_player_holds_one_too() {
    let (qucenn, rarron) = (Planet::Qucenn, Planet::Rarron);
    let s = table().control(qucenn, 0).control(rarron, 1).control(pl(System::Wellon), 0).state();
    assert_eq!(secret(&s, 0, false, NONE, S::StakeYourClaim), Some(Progress { family: Family::SharedPlanetSystems, have: 1, threshold: 1 }));
    assert_eq!(have(secret(&s, 1, false, NONE, S::StakeYourClaim)), 1);
    assert_eq!(have(secret(&s, 2, false, NONE, S::StakeYourClaim)), 0);
    // Two planets in the system, both ours: not shared.
    let mine = table().control(qucenn, 0).control(rarron, 0).state();
    assert_eq!(have(secret(&mine, 0, false, NONE, S::StakeYourClaim)), 0);
    // The Firmament commander: the rival's planet counts as ours, so the system is no longer shared with anyone else.
    let mut t = terms();
    t.firmament.insert(rarron);
    assert_eq!(have(secret_with(&s, 0, t.clone(), false, NONE, S::StakeYourClaim)), 0);
    // ... and a planet of ours that the commander lends away leaves the pair the other way round.
    t.firmament.clear();
    t.firmament.insert(qucenn);
    assert_eq!(have(secret_with(&s, 2, t, false, NONE, S::StakeYourClaim)), 1);
}

#[test]
fn occupy_the_seat_of_the_empire_counts_our_ships_on_mecatol_while_we_hold_it() {
    let sc = table()
        .unit(System::MecatolRex18, None, Unit::CruiserI, 0, 2)
        .unit(System::MecatolRex18, None, Unit::FighterI, 0, 1)
        .unit(System::MecatolRex18, None, Unit::CruiserI, 1, 4)
        .unit(System::MecatolRex18, Some(Planet::MecatolRex), Unit::InfantryI, 0, 3);
    let holding = sc.clone().control(Planet::MecatolRex, 0).state();
    assert_eq!(secret(&holding, 0, false, NONE, S::OccupyTheSeatOfTheEmpire), Some(Progress { family: Family::MecatolShipsWhileControlling, have: 3, threshold: 3 }));
    assert_eq!(have(secret(&holding, 1, false, NONE, S::OccupyTheSeatOfTheEmpire)), 0, "seat 1 does not hold Mecatol");
    let not = sc.clone().state();
    assert_eq!(have(secret(&not, 0, false, NONE, S::OccupyTheSeatOfTheEmpire)), 0);
    // The commander lends the planet someone else holds.
    let lent = sc.control(Planet::MecatolRex, 2).state();
    assert_eq!(have(secret(&lent, 0, false, NONE, S::OccupyTheSeatOfTheEmpire)), 0);
    let mut t = terms();
    t.firmament.insert(Planet::MecatolRex);
    assert_eq!(have(secret_with(&lent, 0, t, false, NONE, S::OccupyTheSeatOfTheEmpire)), 3);
    // An uncontrolled planet is not "held" even if lent.
    let mut t = terms();
    t.firmament.insert(Planet::MecatolRex);
    let free = table().unit(System::MecatolRex18, None, Unit::CruiserI, 0, 2).state();
    assert_eq!(have(secret_with(&free, 0, t, false, NONE, S::OccupyTheSeatOfTheEmpire)), 0);
}

#[test]
fn hegemony_and_raw_materials_add_the_printed_values_of_every_planet_once() {
    // Starpoint 3/1, Zohbat 3/1, Lodor 3/1, Torkan 0/3.
    let s = table().control(Planet::Starpoint, 0).control(Planet::Zohbat, 0).control(Planet::Lodor, 0).control(Planet::Torkan, 0).state();
    assert_eq!(secret(&s, 0, false, NONE, S::HoardRawMaterials), Some(Progress { family: Family::CombinedValue(Spend::Resources), have: 9, threshold: 12 }));
    assert_eq!(secret(&s, 0, false, NONE, S::EstablishHegemony), Some(Progress { family: Family::CombinedValue(Spend::Influence), have: 6, threshold: 12 }));
    // An imagined planet adds its values, one that is held already does not add twice.
    let im = [Planet::Capha, Planet::Lodor];
    assert_eq!(have(secret(&s, 0, false, Imagined { planets: &im, ..NONE }, S::HoardRawMaterials)), 12);
    assert_eq!(have(secret(&s, 0, false, Imagined { planets: &im, ..NONE }, S::EstablishHegemony)), 6);
    // The values are the printed ones, not the readied or modified ones.
    let mut ex = s.clone();
    ex.board_mut().planet_mut(Planet::Starpoint).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true);
    ex.board_mut().planet_mut(Planet::Zohbat).attach.push(Attachment::Geoform);
    assert_eq!(have(secret(&ex, 0, false, NONE, S::HoardRawMaterials)), 9);
    // Lent planets are added; planets out of scope are worth nothing.
    let mut t = terms();
    t.firmament.insert(Planet::Rarron);
    assert_eq!(have(secret_with(&s, 0, t, false, NONE, S::EstablishHegemony)), 9);
    let f = SeatFacts::of(&s, Scope(source_bit(ti4_engine_core::generated::Source::Pok)), Seat(0), terms());
    assert_eq!(q::secret_progress(&Pos::new(&s, &f, false, NONE), S::HoardRawMaterials).unwrap().unwrap().have, 0);
}

// ---- the bought objectives -------------------------------------------------------------------------------------

fn bought(s: &CoreState, seat: u8, t: Terms, im: Imagined<'_>, obj: O) -> Option<q::CostProgress> {
    let f = SeatFacts::of(s, ALL_SOURCES, Seat(seat), t);
    q::bought_progress(&Pos::new(s, &f, false, im), obj)
}

#[test]
fn trade_good_and_token_prices_are_paid_with_what_the_seat_holds() {
    let s = table().goods(0, 7).tokens(0, 3, 5, 2).goods(1, 2).tokens(1, 0, 9, 0).state();
    let p = bought(&s, 0, terms(), NONE, O::NegotiateTradeRoutes).unwrap();
    assert_eq!((p.family, p.have, p.target, p.satisfied()), (CostFamily::TradeGoods, 5, 5, true), "more than the price is capped at the price");
    assert_eq!(bought(&s, 0, terms(), NONE, O::CentralizeGalacticTrade).map(|p| (p.have, p.target, p.satisfied())), Some((7, 10, false)));
    assert_eq!(bought(&s, 1, terms(), NONE, O::NegotiateTradeRoutes).map(|p| p.have), Some(2));
    // Tokens: strategy and tactic pools, never the fleet pool.
    assert_eq!(bought(&s, 0, terms(), NONE, O::LeadFromTheFront).map(|p| (p.family, p.have, p.target)), Some((CostFamily::Tokens, 3, 3)));
    assert_eq!(bought(&s, 0, terms(), NONE, O::GalvanizeThePeople).map(|p| (p.have, p.target)), Some((5, 6)));
    assert_eq!(bought(&s, 1, terms(), NONE, O::LeadFromTheFront).map(|p| p.have), Some(0), "nine fleet tokens pay nothing");
    // Imagined planets add nothing to these prices.
    let im = [pl(System::Wellon)];
    assert_eq!(bought(&s, 0, terms(), Imagined { planets: &im, ..NONE }, O::CentralizeGalacticTrade).map(|p| p.have), Some(7));
    // A seat that does not exist holds nothing.
    assert_eq!(bought(&s, 9, terms(), NONE, O::NegotiateTradeRoutes).map(|p| p.have), Some(0));
    assert_eq!(bought(&s, 9, terms(), NONE, O::LeadFromTheFront).map(|p| p.have), Some(0));
    // Not a priced objective.
    assert!(bought(&s, 0, terms(), NONE, O::ExpandBorders).is_none());
}

#[test]
fn spending_prices_are_paid_with_readied_planets_and_trade_goods() {
    // Resources: Starpoint 3, Zohbat 3, Lodor 3, Quann 2 (control) and a planet someone else holds.
    let sc = table().control(Planet::Starpoint, 0).control(Planet::Zohbat, 0).control(Planet::Lodor, 0).control(Planet::Quann, 0).control(Planet::Capha, 1);
    let s = sc.clone().state();
    let p = bought(&s, 0, terms(), NONE, O::ErectAMonument).unwrap();
    assert_eq!((p.family, p.have, p.target, p.satisfied()), (CostFamily::Spend(Spend::Resources), 8, 8, true));
    // A readied planet pays, an exhausted one does not: 11 resources, then 8 after Starpoint is spent.
    assert_eq!(bought(&s, 0, terms(), NONE, O::FoundAGoldenAge).map(|p| (p.have, p.target)), Some((11, 16)));
    let spent = sc.clone().exhaust(Planet::Starpoint).state();
    assert_eq!(bought(&spent, 0, terms(), NONE, O::FoundAGoldenAge).map(|p| p.have), Some(8));
    // Trade goods pay one point each; commodities a hook lets the seat spend are counted too.
    let goods = sc.clone().goods(0, 3).state();
    assert_eq!(bought(&goods, 0, terms(), NONE, O::FoundAGoldenAge).map(|p| p.have), Some(14));
    let mut keleres = terms();
    keleres.commodities = 2;
    assert_eq!(bought(&goods, 0, keleres, NONE, O::FoundAGoldenAge).map(|p| p.have), Some(16));
    // A good that pays two points (per_good).
    let mut dear = terms();
    dear.per_good = 2;
    assert_eq!(bought(&goods, 0, dear, NONE, O::FoundAGoldenAge).map(|p| p.have), Some(16));
    // The Last Bastion's dock adds to a planet's resources only.
    let mut helios = terms();
    helios.helios.push((Planet::Quann, 2));
    let two = table().control(Planet::Quann, 0).state();
    assert_eq!(bought(&two, 0, terms(), NONE, O::ErectAMonument).map(|p| p.have), Some(2));
    assert_eq!(bought(&two, 0, helios.clone(), NONE, O::ErectAMonument).map(|p| p.have), Some(4));
    assert_eq!(bought(&two, 0, helios, NONE, O::SwayTheCouncil).map(|p| p.have), Some(1), "Quann has 1 influence and Helios adds none");
    // Influence uses the influence values.
    assert_eq!(bought(&s, 0, terms(), NONE, O::SwayTheCouncil).map(|p| (p.family, p.have, p.target)), Some((CostFamily::Spend(Spend::Influence), 4, 8)));
    assert_eq!(bought(&s, 1, terms(), NONE, O::SwayTheCouncil).map(|p| p.have), Some(0));
    assert_eq!(bought(&s, 0, terms(), NONE, O::ManipulateGalacticLaw).map(|p| p.target), Some(16));
}

#[test]
fn a_seat_with_ocean_cards_gets_no_answer_for_the_planet_prices_only() {
    let s = table().control(Planet::Starpoint, 0).goods(0, 9).tokens(0, 4, 0, 0).state();
    let mut oceans = terms();
    oceans.oceans = true;
    for o in [O::ErectAMonument, O::SwayTheCouncil, O::AmassWealth, O::HoldVastReserves] {
        assert!(bought(&s, 0, oceans.clone(), NONE, o).is_none(), "{o:?}");
    }
    assert_eq!(bought(&s, 0, oceans.clone(), NONE, O::NegotiateTradeRoutes).map(|p| p.have), Some(5));
    assert_eq!(bought(&s, 0, oceans.clone(), NONE, O::LeadFromTheFront).map(|p| p.have), Some(3));
    let f = SeatFacts::of(&s, ALL_SOURCES, Seat(0), oceans);
    assert!(q::useful_planets(&s, &f, Spend::Resources).is_none());
    assert!(q::first_plan(&s, &f, Spend::Resources, 2).is_none());
    assert!(q::all_three_plan(&s, &f, 1).is_none());
}

#[test]
fn imagined_planets_add_their_values_to_a_spending_price_up_to_the_price() {
    let s = table().control(Planet::Quann, 0).state(); // 2 resources, 1 influence
    let im = [Planet::Starpoint, Planet::Quann, Planet::Lodor]; // 3/1, held already, 3/1
    let with = Imagined { planets: &im, ..NONE };
    assert_eq!(bought(&s, 0, terms(), with, O::ErectAMonument).map(|p| p.have), Some(8), "2 held + 3 + 3");
    assert_eq!(bought(&s, 0, terms(), with, O::SwayTheCouncil).map(|p| p.have), Some(3), "1 held + 1 + 1");
    let lot = [Planet::Starpoint, Planet::Lodor, Planet::Zohbat, Planet::Capha, Planet::Mellon];
    assert_eq!(bought(&s, 0, terms(), Imagined { planets: &lot, ..NONE }, O::ErectAMonument).map(|p| p.have), Some(8), "capped at the price");
    assert_eq!(bought(&s, 0, terms(), NONE, O::ErectAMonument).map(|p| p.have), Some(2));
    // A station or a planet out of scope adds nothing: the station is dropped, so is a Thunder's Edge planet in a base-only game.
    let st = [Planet::OluzStation];
    assert_eq!(bought(&s, 0, terms(), Imagined { planets: &st, ..NONE }, O::ErectAMonument).map(|p| p.have), Some(2));
}

#[test]
fn amass_wealth_needs_disjoint_planets_for_the_resources_and_the_influence() {
    // Capha 3 resources, Torkan 3 influence, 3 trade goods.
    let s = table().control(Planet::Capha, 0).control(Planet::Torkan, 0).goods(0, 3).state();
    let p = bought(&s, 0, terms(), NONE, O::AmassWealth).unwrap();
    assert_eq!((p.family, p.have, p.target, p.satisfied()), (CostFamily::AllThree, 3, 3, true));
    assert_eq!(bought(&s, 0, terms(), NONE, O::HoldVastReserves).map(|p| (p.have, p.target)), Some((3, 6)));
    // Two trade goods: two of each.
    let two = table().control(Planet::Capha, 0).control(Planet::Torkan, 0).goods(0, 2).state();
    assert_eq!(bought(&two, 0, terms(), NONE, O::AmassWealth).map(|p| p.have), Some(2));
    // One planet cannot pay both: Lodor (3 resources, 1 influence) and 3 goods pays only 1 of each.
    let one = table().control(Planet::Lodor, 0).goods(0, 3).state();
    assert_eq!(bought(&one, 0, terms(), NONE, O::AmassWealth).map(|p| p.have), Some(1));
    // Nothing at all: nothing.
    assert_eq!(bought(&table().state(), 0, terms(), NONE, O::AmassWealth).map(|p| p.have), Some(0));
    // No such seat: no answer.
    let f = SeatFacts::of(&s, ALL_SOURCES, Seat(9), terms());
    assert!(q::bought_progress_base(&s, &f, O::AmassWealth).is_none());
}

#[test]
fn the_useful_planets_are_the_readied_controlled_ones_worth_something_best_first() {
    let sc = table()
        .control(Planet::Quann, 0) // 2/1
        .control(Planet::Lodor, 0) // 3/1
        .control(Planet::Capha, 0) // 3/0
        .control(Planet::Torkan, 0) // 0/3
        .control(Planet::Zohbat, 0)
        .exhaust(Planet::Zohbat) // 3/1 but exhausted
        .control(Planet::Starpoint, 1);
    let s = sc.state();
    let f = facts(&s, 0);
    let res = q::useful_planets(&s, &f, Spend::Resources).unwrap();
    // Worth first, then planet order (Lodor and Capha tie at three).
    let mut tie = [Planet::Lodor, Planet::Capha];
    tie.sort();
    assert_eq!(res.as_slice(), &[(tie[0], 3), (tie[1], 3), (Planet::Quann, 2)]);
    let inf = q::useful_planets(&s, &f, Spend::Influence).unwrap();
    assert_eq!(inf.as_slice().first(), Some(&(Planet::Torkan, 3)));
    assert_eq!(inf.len(), 3, "Capha has no influence, Zohbat is exhausted, Starpoint is the rival's");
    // Only six planets are listed.
    let many = [Planet::Quann, Planet::Lodor, Planet::Capha, Planet::Torkan, Planet::Starpoint, Planet::Zohbat, Planet::Mellon, Planet::Rarron];
    let mut sc = table();
    for p in many {
        sc = sc.control(p, 0);
    }
    let s = sc.state();
    let f = facts(&s, 0);
    let all = q::useful_planets(&s, &f, Spend::Influence).unwrap();
    assert_eq!(all.len(), 6);
    assert!(all.as_slice().iter().all(|(_, w)| *w >= 1) && all.as_slice().windows(2).all(|w| w[0].1 >= w[1].1));
    // The Last Bastion dock raises the resources of its planet.
    let mut t = terms();
    t.helios.push((Planet::Quann, 4));
    let f = SeatFacts::of(&s, ALL_SOURCES, Seat(0), t);
    let res = q::useful_planets(&s, &f, Spend::Resources).unwrap();
    assert_eq!(res.as_slice().first(), Some(&(Planet::Quann, 6)));
    assert_eq!(q::useful_planets(&s, &f, Spend::Influence).unwrap().iter().find(|(p, _)| *p == Planet::Quann), Some(&(Planet::Quann, 1)));
}

#[test]
fn the_first_plan_pays_with_the_fewest_goods_then_the_fewest_planets() {
    let s = table().control(Planet::Lodor, 0).control(Planet::Quann, 0).goods(0, 1).state();
    let f = facts(&s, 0);
    // Five resources: both planets, no goods.
    let plan = q::first_plan(&s, &f, Spend::Resources, 5).unwrap();
    assert_eq!(plan.trade_goods, 0);
    let mut got = plan.planets().to_vec();
    got.sort();
    let mut want = vec![Planet::Lodor, Planet::Quann];
    want.sort();
    assert_eq!(got, want);
    // Six: five from the planets and a good.
    let six = q::first_plan(&s, &f, Spend::Resources, 6).unwrap();
    assert_eq!(six.trade_goods, 1);
    assert_eq!(six.planets().len(), 2);
    // Seven: not payable.
    assert!(q::first_plan(&s, &f, Spend::Resources, 7).is_none());
    // Nothing to pay: the empty plan.
    let zero = q::first_plan(&s, &f, Spend::Resources, 0).unwrap();
    assert_eq!((zero.trade_goods, zero.planets().len()), (0, 0));
}

#[test]
fn the_all_three_plan_is_the_first_disjoint_pair_that_the_goods_can_carry() {
    let s = table().control(Planet::Capha, 0).control(Planet::Torkan, 0).goods(0, 3).state();
    let f = facts(&s, 0);
    let (res, inf) = q::all_three_plan(&s, &f, 3).unwrap();
    assert_eq!((res.planets().to_vec(), res.trade_goods), (vec![Planet::Capha], 0));
    assert_eq!((inf.planets().to_vec(), inf.trade_goods), (vec![Planet::Torkan], 0));
    // Four of each: the planets give three, one good each side makes two goods, plus the four of the price: more than the three held.
    assert!(q::all_three_plan(&s, &f, 4).is_none());
    // No goods held: nothing to put in the third pile.
    let broke = table().control(Planet::Capha, 0).control(Planet::Torkan, 0).state();
    let f = facts(&broke, 0);
    assert!(q::all_three_plan(&broke, &f, 3).is_none());
    assert!(q::all_three_plan(&broke, &f, 0).is_some());
}

// ---- secrets in hand --------------------------------------------------------------------------------------------

fn with_secrets(hand: &[S], scored: &[S], relics: &[Relic]) -> CoreState {
    let mut s = table().state();
    for o in hand {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(*o);
    }
    for o in scored {
        s.objectives_mut().scored[0].secret.insert(*o);
    }
    for r in relics {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(*r);
    }
    s
}

#[test]
fn the_secret_hand_limit_is_three_counting_the_scored_ones() {
    let (a, b, c, d) = (S::SeizeAnIcon, S::BraveTheVoid, S::DictatePolicy, S::DarkenTheSkies);
    assert_eq!(q::HAND_LIMIT, 3);
    let three = with_secrets(&[a, b, c], &[], &[]);
    assert_eq!(q::secrets_held(&three, Seat(0)), 3);
    assert_eq!(q::hand_step(&three, Seat(0)), HandStep::Within);
    // Four in hand: one has to go back, and which one is asked.
    let four = with_secrets(&[a, b, c, d], &[], &[]);
    assert_eq!(q::secrets_held(&four, Seat(0)), 4);
    match q::hand_step(&four, Seat(0)) {
        HandStep::Ask { held, hand } => assert_eq!((held, hand.as_slice().to_vec()), (4, vec![a, b, c, d])),
        other => panic!("{other:?}"),
    }
    // Two in hand and two scored: over the limit, only the unscored ones can go back.
    let scored = with_secrets(&[a, b], &[c, d], &[]);
    assert_eq!(q::secrets_held(&scored, Seat(0)), 4);
    match q::hand_step(&scored, Seat(0)) {
        HandStep::Ask { held, hand } => assert_eq!((held, hand.len()), (4, 2)),
        other => panic!("{other:?}"),
    }
    // One in hand and three scored: it goes back without a question.
    let one = with_secrets(&[a], &[b, c, d], &[]);
    assert_eq!(q::hand_step(&one, Seat(0)), HandStep::Return(a));
    // All scored: nothing in hand to give back.
    let none = with_secrets(&[], &[a, b, c, d], &[]);
    assert_eq!(q::hand_step(&none, Seat(0)), HandStep::Within);
    // The Obsidian lifts the limit by one.
    let obsidian = with_secrets(&[a, b, c, d], &[], &[Relic::TheObsidian]);
    assert_eq!(q::hand_step(&obsidian, Seat(0)), HandStep::Within);
    let five = with_secrets(&[a, b, c, d, S::ProveEndurance], &[], &[Relic::TheObsidian]);
    assert!(matches!(q::hand_step(&five, Seat(0)), HandStep::Ask { held: 5, .. }));
    // A seat that is not at the table holds nothing.
    assert_eq!(q::secrets_held(&three, Seat(9)), 0);
    assert_eq!(q::hand_step(&three, Seat(9)), HandStep::Within);
}
