//! D4-2a: `rules::production` (prices, producers, capacity, structure limits). The window of a production is built by the adapter from many
//! facts; the arithmetic it stands on is tested here from the printed unit data and the rules of PRODUCTION (LRR 67).

use ti4_engine_core::generated::{Agenda, UnitBaseType};
use ti4_engine_core::rules::production::{self as prod, Cx};
use ti4_engine_core::state::{Outcome, RoundKind};
use ti4_engine_core::tables::Scope;
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn table() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheEmiratesOfHacan]).map_ring(7)
}

fn cx<'a>(s: &'a CoreState, k: &'a Kernel) -> Cx<'a> {
    Cx::new(s, k, ALL_SOURCES, false)
}

#[test]
fn a_price_is_the_cost_rounded_up_with_two_cheap_units_per_resource_unless_one_unit_at_a_time() {
    assert_eq!(prod::price(Some(1), false), (1, 2), "infantry and fighters: two for one resource");
    assert_eq!(prod::price(Some(1), true), (1, 1), "single unit production");
    assert_eq!(prod::price(Some(2), false), (1, 1));
    assert_eq!(prod::price(Some(3), false), (2, 1), "half a resource is paid in full");
    assert_eq!(prod::price(Some(8), false), (4, 1));
    assert_eq!(prod::price(Some(24), true), (12, 1));
    assert_eq!(prod::price(None, false), (0, 1));
    assert_eq!(prod::price(Some(255), false).0, 64, "capped like the string loop");
}

#[test]
fn the_price_of_a_printed_unit_follows_its_cost_and_the_conscription_law() {
    let (s, k) = table().build();
    let c = cx(&s, &k);
    assert_eq!(prod::price_of(&c, Unit::FighterI), (1, 2));
    assert_eq!(prod::price_of(&c, Unit::DreadnoughtI), (4, 1));
    assert_eq!(prod::price_of(&c, Unit::CarrierI), (3, 1));
    assert_eq!(prod::price_of(&c, Unit::WarSun), (12, 1));
    let mut law = s.clone();
    law.agenda_mut().laws.insert(Agenda::RegulatedConscription, Outcome::Empty);
    let c = cx(&law, &k);
    assert_eq!(prod::price_of(&c, Unit::FighterI), (1, 1), "Regulated Conscription: one unit for one resource");
    assert_eq!(prod::price_of(&c, Unit::DreadnoughtI), (4, 1));
    assert_eq!(prod::price_of(&Cx::new(&law, &k, ALL_SOURCES, true), Unit::FighterI), (1, 2), "blanked laws");
}

#[test]
fn structure_limits_are_one_dock_and_two_pds_per_planet() {
    assert_eq!(prod::structure_limit(UnitBaseType::Spacedock), Some(1));
    assert_eq!(prod::structure_limit(UnitBaseType::Pds), Some(2));
    assert_eq!(prod::structure_limit(UnitBaseType::Infantry), None);
    assert_eq!(prod::structure_limit(UnitBaseType::Mech), None);
}

#[test]
fn a_structure_is_allowed_until_the_limit_the_homeland_defense_act_lifts_the_pds_cap_and_a_demilitarized_planet_takes_none() {
    let systems = plain_systems();
    let planet = systems[1].data().planets[0];
    let k = kernel_all();
    let none = table().state();
    let c = cx(&none, &k);
    assert!(prod::structure_allowed(&c, Seat(0), planet, UnitBaseType::Spacedock));
    let docked = table().unit(systems[1], Some(planet), Unit::SpaceDockI, 0, 1).unit(systems[1], Some(planet), Unit::PDSI, 0, 2).state();
    let c = cx(&docked, &k);
    assert_eq!(prod::structures_on(&c, Seat(0), planet, UnitBaseType::Spacedock), 1);
    assert_eq!(prod::structures_on(&c, Seat(1), planet, UnitBaseType::Spacedock), 0);
    assert!(!prod::structure_allowed(&c, Seat(0), planet, UnitBaseType::Spacedock), "one dock per planet");
    assert!(!prod::structure_allowed(&c, Seat(0), planet, UnitBaseType::Pds), "two PDS per planet");
    assert!(prod::structure_allowed(&c, Seat(1), planet, UnitBaseType::Pds), "limits are per owner");
    let mut lifted = docked.clone();
    lifted.agenda_mut().laws.insert(Agenda::HomelandDefenseAct, Outcome::Empty);
    assert!(prod::structure_allowed(&cx(&lifted, &k), Seat(0), planet, UnitBaseType::Pds), "Homeland Defense Act");
    assert!(!prod::structure_allowed(&cx(&lifted, &k), Seat(0), planet, UnitBaseType::Spacedock), "the dock limit stays");
    let mut dmz = none.clone();
    dmz.agenda_mut().laws.insert(Agenda::DemilitarizedZone, Outcome::Planet(planet));
    assert!(!prod::structure_allowed(&cx(&dmz, &k), Seat(0), planet, UnitBaseType::Pds), "a demilitarized zone takes no structure");
    assert!(!prod::structure_allowed(&cx(&dmz, &k), Seat(0), planet, UnitBaseType::Spacedock));
    assert!(prod::structure_allowed(&Cx::new(&dmz, &k, ALL_SOURCES, true), Seat(0), planet, UnitBaseType::Pds), "blanked laws");
}

#[test]
fn the_producers_of_a_system_are_our_docks_in_space_and_on_its_planets() {
    let systems = plain_systems();
    let planet = systems[1].data().planets[0];
    let k = kernel_all();
    let s = table().unit(systems[1], Some(planet), Unit::SpaceDockI, 0, 1).unit(systems[1], Some(planet), Unit::InfantryI, 0, 3).unit(systems[1], Some(planet), Unit::SpaceDockI, 1, 1).unit(systems[1], None, Unit::CruiserI, 0, 2).state();
    let c = cx(&s, &k);
    let found = prod::producers(&c, Seat(0), systems[1]);
    assert_eq!(found.len(), 1);
    assert_eq!(found.as_slice()[0].0.kind(), Unit::SpaceDockI);
    assert_eq!(found.as_slice()[0].1, Some(planet));
    assert_eq!(prod::producers(&c, Seat(1), systems[1]).len(), 1, "each seat sees its own");
    assert!(prod::producers(&c, Seat(2), systems[1]).is_empty());
    assert!(prod::producers(&c, Seat(0), systems[0]).is_empty(), "another system");
}

#[test]
fn a_space_dock_produces_two_plus_the_resources_of_its_planet_and_war_machine_adds_five_for_the_activation() {
    let systems = plain_systems();
    let planet = systems[1].data().planets.iter().copied().max_by_key(|p| p.data().resources).unwrap();
    let k = kernel_all();
    let mut s = table().control(planet, 0).unit(systems[1], Some(planet), Unit::SpaceDockI, 0, 1).state();
    let c = cx(&s, &k);
    let r = i64::from(planet.data().resources);
    assert_eq!(prod::capacity(&c, Seat(0), systems[1]), 2 + r);
    assert_eq!(prod::capacity(&c, Seat(1), systems[1]), 0);
    assert_eq!(prod::war_machine_bonus(&s, Seat(0)), 0);
    s.meta_mut().activation_seq = 3;
    s.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::WarMachine, 3);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::WarMachine, 2);
    assert_eq!(prod::war_machine_bonus(&s, Seat(0)), 5, "only the mark of this activation counts");
    assert_eq!(prod::capacity(&cx(&s, &k), Seat(0), systems[1]), 2 + r + 5);
    let twice = {
        let mut t = s.clone();
        t.players_mut().by_seat_mut(Seat(0)).unwrap().round_marks.push(RoundKind::WarMachine, 3);
        t
    };
    assert_eq!(prod::war_machine_bonus(&twice, Seat(0)), 10);
}

#[test]
fn a_planets_worth_for_spending_is_its_printed_value_plus_attachments() {
    let systems = plain_systems();
    let planet = systems[1].data().planets[0];
    let k = kernel_all();
    let s = table().control(planet, 0).state();
    let c = cx(&s, &k);
    let want = i64::from(planet.data().resources);
    assert_eq!(prod::planet_value(&c, Seat(0), planet, ti4_engine_core::query::economy::Spend::Resources), want);
    let out_of_scope = Cx::new(&s, &k, Scope(0), false);
    assert_eq!(prod::planet_value(&out_of_scope, Seat(0), planet, ti4_engine_core::query::economy::Spend::Resources), 0);
    assert_eq!(prod::mobile_docks(&c, Seat(0), systems[1]).len(), 0);
}

#[test]
fn a_floating_factory_in_space_is_a_mobile_dock_and_a_producer() {
    let systems = plain_systems();
    let k = kernel_all();
    let s = Scenario::new(&[Faction::TheClanOfSaar, Faction::TheArborec]).map_ring(7).unit(systems[2], None, Unit::FloatingFactoryI, 0, 1).state();
    let c = cx(&s, &k);
    assert_eq!(prod::mobile_docks(&c, Seat(0), systems[2]).len(), 1);
    assert_eq!(prod::mobile_docks(&c, Seat(1), systems[2]).len(), 0);
    assert_eq!(prod::producers(&c, Seat(0), systems[2]).len(), 1);
    assert!(prod::capacity(&c, Seat(0), systems[2]) >= 4, "the printed production value");
}
