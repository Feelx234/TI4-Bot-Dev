//! D4-1 low layer: `query/{seats,economy,laws,board}`.

use ti4_engine_core::generated::{Agenda, Attachment, Planet, StrategyCard, System, Unit, UnitBaseType};
use ti4_engine_core::query::{board, economy, economy::Spend, laws, seats};
use ti4_engine_core::state::{Outcome, PlanetFlags};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{first_planet, plain_systems, system_of};

fn four() -> Scenario {
    Scenario::lobby(4).map_ring(7)
}

// ---- seats ------------------------------------------------------------------------------------

#[test]
fn the_seating_order_rotates_from_any_seat_and_stays_unrotated_for_a_stranger() {
    let s = four().state();
    assert_eq!(seats::seating_order(&s).as_slice(), &[Seat(0), Seat(1), Seat(2), Seat(3)]);
    assert_eq!(seats::clockwise_from(&s, Seat(2)).as_slice(), &[Seat(2), Seat(3), Seat(0), Seat(1)]);
    assert_eq!(seats::clockwise_from(&s, Seat(0)).as_slice(), &[Seat(0), Seat(1), Seat(2), Seat(3)]);
    assert_eq!(seats::clockwise_from(&s, Seat(9)).as_slice(), &[Seat(0), Seat(1), Seat(2), Seat(3)]);
    assert_eq!(seats::position(&s, Seat(3)), Some(3));
    assert_eq!(seats::position(&s, Seat(9)), None);
}

#[test]
fn the_speaker_order_starts_at_the_speaker_and_next_after_wraps() {
    let s = four().speaker(2).state();
    assert_eq!(seats::speaker_order(&s).as_slice(), &[Seat(2), Seat(3), Seat(0), Seat(1)]);
    assert_eq!(seats::next_after(&s, Seat(3)), Seat(0));
    assert_eq!(seats::next_after(&s, Seat(0)), Seat(1));
    assert_eq!(seats::next_after(&s, Seat(9)), Seat(9), "not seated: itself");
    let alone = Scenario::lobby(1).state();
    assert_eq!(seats::next_after(&alone, Seat(0)), Seat(0));
    assert!(seats::is_player(&s, Seat(3)) && !seats::is_player(&s, Seat(4)));
}

#[test]
fn initiative_orders_by_the_lowest_card_with_overrides_and_seating_breaking_ties() {
    let mut s = four().state();
    let (leadership, diplomacy, politics) = (StrategyCard::ALL[0], StrategyCard::ALL[1], StrategyCard::ALL[2]);
    for (i, c) in [leadership, diplomacy, politics].into_iter().enumerate() {
        s.strategy_mut().initiative.insert(c, i as i32 + 1);
    }
    // Seat 3 holds the first card, seat 1 the second and the third, seat 0 and 2 hold none (99, tie by seating).
    s.players_mut().by_seat_mut(Seat(3)).unwrap().strat.push(leadership);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().strat.push(politics);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().strat.push(diplomacy);
    assert_eq!(seats::initiative_order(&s).as_slice(), &[Seat(3), Seat(1), Seat(0), Seat(2)]);
    s.strategy_mut().initiative_overrides.insert(Seat(2), 0);
    assert_eq!(seats::initiative_order(&s).as_slice(), &[Seat(2), Seat(3), Seat(1), Seat(0)], "an override replaces the printed number");
}

// ---- economy ----------------------------------------------------------------------------------

fn pair_with_values() -> (Planet, Planet) {
    let rich = Planet::ALL.into_iter().find(|p| p.data().resources >= 3 && p.data().influence == 0 && p.data().source == ti4_engine_core::generated::Source::Base).unwrap();
    let wise = Planet::ALL.into_iter().find(|p| p.data().influence >= 2 && p.data().resources == 0 && p.data().source == ti4_engine_core::generated::Source::Base).unwrap();
    (rich, wise)
}

#[test]
fn printed_values_are_zero_outside_the_scope() {
    let (rich, wise) = pair_with_values();
    assert_eq!(economy::printed_value(ALL_SOURCES, rich, Spend::Resources), i64::from(rich.data().resources));
    assert_eq!(economy::printed_value(ALL_SOURCES, wise, Spend::Influence), i64::from(wise.data().influence));
    assert_eq!(economy::printed_value(ALL_SOURCES, rich, Spend::Influence), 0);
    let none = ti4_engine_core::tables::Scope(0);
    assert_eq!(economy::printed_value(none, rich, Spend::Resources), 0);
}

#[test]
fn attachments_and_laws_add_to_the_printed_value() {
    let (rich, wise) = pair_with_values();
    let att = Attachment::ALL.into_iter().find(|a| a.data().resources.is_some_and(|r| r > 0)).expect("an attachment with a resource bonus");
    let bonus = i64::from(att.data().resources.unwrap_or(0));
    let mut s = four().state();
    assert_eq!(economy::attachment_bonus(&s, rich, Spend::Resources), 0);
    s.board_mut().planet_mut(rich).attach.push(att);
    assert_eq!(economy::attachment_bonus(&s, rich, Spend::Resources), bonus);
    assert_eq!(economy::value_now(&s, ALL_SOURCES, false, rich, Spend::Resources, 0), i64::from(rich.data().resources) + bonus);
    assert_eq!(economy::value_now(&s, ALL_SOURCES, false, rich, Spend::Resources, 5), i64::from(rich.data().resources) + bonus + 5, "the hook term");
    // Core Mining (+2 resources), Senate Sanctuary (+2 influence), Terraforming Initiative (+1 +1) onto a planet.
    s.agenda_mut().laws.insert(Agenda::CoreMining, Outcome::Planet(rich));
    s.agenda_mut().laws.insert(Agenda::SenateSanctuary, Outcome::Planet(wise));
    s.agenda_mut().laws.insert(Agenda::TerraformingInitiative, Outcome::Planet(rich));
    assert_eq!(laws::planet_value_bonus(&s, false, rich, Spend::Resources), 3);
    assert_eq!(laws::planet_value_bonus(&s, false, rich, Spend::Influence), 1);
    assert_eq!(laws::planet_value_bonus(&s, false, wise, Spend::Influence), 2);
    assert_eq!(laws::planet_value_bonus(&s, false, wise, Spend::Resources), 0);
    assert_eq!(laws::planet_value_bonus(&s, true, rich, Spend::Resources), 0, "blanked: no law in play");
    let srcs = laws::planet_value_bonus_sources(&s, false, rich);
    assert_eq!(srcs.len(), 2);
    assert_eq!(economy::value_now(&s, ALL_SOURCES, false, rich, Spend::Resources, 0), i64::from(rich.data().resources) + bonus + 3);
}

#[test]
fn only_controlled_ready_planets_are_spendable_and_they_are_summed() {
    let (rich, wise) = pair_with_values();
    let s = four().control(rich, 0).control(wise, 0).state();
    assert_eq!(economy::spendable_planets(&s, Seat(0)).len(), 2);
    let total_r = economy::spendable_total(&s, ALL_SOURCES, false, Seat(0), Spend::Resources);
    assert_eq!(total_r, i64::from(rich.data().resources) + i64::from(wise.data().resources));
    let s = four().control(rich, 0).control(wise, 0).exhaust(rich).state();
    let ready: Vec<Planet> = economy::spendable_planets(&s, Seat(0)).as_slice().iter().map(|(_, p)| *p).collect();
    assert_eq!(ready, vec![wise]);
    assert_eq!(economy::spendable_total(&s, ALL_SOURCES, false, Seat(0), Spend::Resources), i64::from(wise.data().resources));
    assert!(economy::spendable_planets(&s, Seat(1)).is_empty());
}

#[test]
fn reinforcements_are_sixteen_less_sheet_board_and_held_tokens_never_below_zero() {
    let systems = plain_systems();
    let s = four().tokens(0, 3, 3, 2).command_token(systems[0], 0).command_token(systems[1], 0).state();
    assert_eq!(economy::tokens_in_reinforcements(&s, Seat(0), 0), 16 - 8 - 2);
    assert_eq!(economy::tokens_in_reinforcements(&s, Seat(0), 3), 16 - 8 - 2 - 3, "tokens a Mahact holds");
    assert_eq!(economy::tokens_in_reinforcements(&s, Seat(0), 99), 0);
    assert_eq!(economy::tokens_in_reinforcements(&s, Seat(1), 0), 16);
    assert_eq!(economy::TOKENS_PER_FACTION, 16);
}

#[test]
fn a_command_token_goes_where_there_is_none_of_ours_and_stock_remains() {
    let systems = plain_systems();
    let s = four().tokens(0, 3, 3, 2).command_token(systems[0], 0).state();
    assert!(!economy::can_place_command_token(&s, Seat(0), systems[0], 0), "already there");
    assert!(economy::can_place_command_token(&s, Seat(0), systems[1], 0));
    assert!(economy::can_place_command_token(&s, Seat(1), systems[0], 0), "another seat's token does not block");
    assert!(!economy::can_place_command_token(&s, Seat(9), systems[1], 0), "no such player");
    let empty = four().tokens(0, 8, 8, 0).state();
    assert!(!economy::can_place_command_token(&empty, Seat(0), systems[1], 0), "none left in reinforcements");
    assert!(economy::can_place_command_token(&empty, Seat(0), systems[1], 0) == false);
}

// ---- laws -------------------------------------------------------------------------------------

#[test]
fn laws_are_in_play_unless_blanked_and_report_what_they_were_elected_onto() {
    let mut s = four().state();
    assert!(laws::in_play(&s, false).is_empty());
    s.agenda_mut().laws.insert(Agenda::FleetRegulations, Outcome::Empty);
    s.agenda_mut().laws.insert(Agenda::MinisterOfPolicy, Outcome::Seat(Seat(2)));
    assert!(laws::active(&s, false, laws::FLEET_REGULATIONS) && !laws::active(&s, true, laws::FLEET_REGULATIONS));
    assert_eq!(laws::in_play(&s, false).len(), 2);
    assert!(laws::in_play(&s, true).is_empty());
    assert_eq!(laws::elected(&s, false, laws::MINISTER_POLICY), Some(Outcome::Seat(Seat(2))));
    assert_eq!(laws::elected(&s, true, laws::MINISTER_POLICY), None);
    assert!(laws::holds_ministry(&s, false, Seat(2), laws::MINISTER_POLICY) && !laws::holds_ministry(&s, false, Seat(1), laws::MINISTER_POLICY));
    assert!(!laws::holds_ministry(&s, true, Seat(2), laws::MINISTER_POLICY));
}

#[test]
fn standing_laws_cap_and_forbid_what_they_say() {
    let (system, planet) = first_planet();
    let mut s = four().state();
    assert_eq!(laws::fleet_pool_cap(&s, false, 7), 7);
    assert_eq!(laws::action_card_limit(&s, false, 7), 7);
    assert!(!laws::nebulae_passable(&s, false) && !laws::single_unit_production(&s, false));
    assert!(!laws::structure_cap_lifted(&s, false, UnitBaseType::Pds) && !laws::censure_bars(&s, false, Seat(1)));
    for (law, outcome) in [(Agenda::FleetRegulations, Outcome::Empty), (Agenda::ExecutiveSanctions, Outcome::Empty), (Agenda::SharedResearch, Outcome::Empty), (Agenda::RegulatedConscription, Outcome::Empty), (Agenda::HomelandDefenseAct, Outcome::Empty), (Agenda::PoliticalCensure, Outcome::Seat(Seat(1))), (Agenda::HolyPlanetOfIxth, Outcome::Planet(planet))] {
        s.agenda_mut().laws.insert(law, outcome);
    }
    assert_eq!(laws::fleet_pool_cap(&s, false, 7), 4);
    assert_eq!(laws::fleet_pool_cap(&s, false, 3), 3, "a cap never raises");
    assert_eq!(laws::action_card_limit(&s, false, 7), 3);
    assert_eq!(laws::action_card_limit(&s, false, 2), 2);
    assert!(laws::nebulae_passable(&s, false) && laws::single_unit_production(&s, false));
    assert!(laws::structure_cap_lifted(&s, false, UnitBaseType::Pds) && !laws::structure_cap_lifted(&s, false, UnitBaseType::Spacedock));
    assert!(laws::censure_bars(&s, false, Seat(1)) && !laws::censure_bars(&s, false, Seat(0)));
    assert!(laws::production_forbidden_on(&s, false, planet) && !laws::production_forbidden_on(&s, true, planet));
    let _ = system;
}

#[test]
fn the_demilitarized_zone_is_an_elected_planet_or_since_version_34_an_attachment() {
    let (_, planet) = first_planet();
    let other = plain_systems()[1].data().planets[0];
    let mut s = four().rules(33).state();
    assert!(!laws::planet_is_demilitarized(&s, false, planet));
    s.agenda_mut().laws.insert(Agenda::DemilitarizedZone, Outcome::Planet(planet));
    assert!(laws::planet_is_demilitarized(&s, false, planet) && !laws::planet_is_demilitarized(&s, false, other));
    assert!(!laws::planet_is_demilitarized(&s, true, planet));
    s.board_mut().planet_mut(other).attach.push(Attachment::DemilitarizedZone);
    assert!(!laws::planet_is_demilitarized(&s, false, other), "before version 34 the card's attachment is not a zone");
    s.meta_mut().rules_version = laws::RULES_DMZ_EXPLORATION;
    assert!(laws::planet_is_demilitarized(&s, false, other));
}

#[test]
fn conventions_of_war_forbid_bombardment_only_of_cultural_planets_in_scope() {
    let cultural = Planet::ALL.into_iter().find(|p| p.data().traits.contains(&ti4_engine_core::generated::PlanetTrait::Cultural)).unwrap();
    let hazardous = Planet::ALL.into_iter().find(|p| !p.data().traits.contains(&ti4_engine_core::generated::PlanetTrait::Cultural)).unwrap();
    let mut s = four().state();
    assert!(!laws::bombardment_forbidden(&s, false, ALL_SOURCES, cultural), "no law");
    s.agenda_mut().laws.insert(Agenda::ConventionsOfWar, Outcome::Empty);
    assert!(laws::bombardment_forbidden(&s, false, ALL_SOURCES, cultural));
    assert!(!laws::bombardment_forbidden(&s, false, ALL_SOURCES, hazardous));
    assert!(!laws::bombardment_forbidden(&s, true, ALL_SOURCES, cultural));
    assert!(!laws::bombardment_forbidden(&s, false, ti4_engine_core::tables::Scope(0), cultural), "out of scope");
}

// ---- board ------------------------------------------------------------------------------------

#[test]
fn control_exhaustion_and_counts_are_per_planet_and_per_seat() {
    let systems = plain_systems();
    let (p0, p1) = (systems[0].data().planets[0], systems[1].data().planets[0]);
    let s = four().control(p0, 1).control(p1, 1).exhaust(p1).state();
    assert_eq!(board::control(&s, p0), Some(Seat(1)));
    assert!(board::controls(&s, Seat(1), p0) && !board::controls(&s, Seat(0), p0));
    assert_eq!(board::controlled_count(&s, Seat(1)), 2);
    assert_eq!(board::controlled_count(&s, Seat(0)), 0);
    assert_eq!(board::controlled_planets(&s, Seat(1)).as_slice(), &[(systems[0], p0), (systems[1], p1)]);
    assert!(board::exhausted(&s, p1) && !board::exhausted(&s, p0));
    assert!(board::controls_a_planet_in(&s, systems[0], Seat(1)) && !board::controls_a_planet_in(&s, systems[0], Seat(2)));
    assert_eq!(board::printed_system(p0), Some(systems[0]));
    assert_eq!(board::home_system(&s, p0), Some(systems[0]));
}

#[test]
fn a_planet_placed_elsewhere_belongs_to_the_system_it_was_placed_in() {
    let systems = plain_systems();
    let (p0, other) = (systems[0].data().planets[0], systems[1]);
    let mut s = four().control(p0, 1).state();
    s.board_mut().planet_mut(p0).placed_in = Some(other);
    assert_eq!(board::home_system(&s, p0), Some(other));
    assert_eq!(board::system_of(&s, ALL_SOURCES, p0), Some(other));
    assert!(board::planets_of(&s, other).any(|p| p == p0));
    assert!(!board::planets_of(&s, systems[0]).any(|p| p == p0));
    assert!(board::planets_in(&s, ALL_SOURCES, other).as_slice().contains(&p0));
    assert_eq!(board::controlled_planets(&s, Seat(1)).as_slice(), &[(other, p0)]);
}

#[test]
fn a_planet_of_an_expansion_out_of_scope_is_found_only_when_it_has_an_entry() {
    let pok = Planet::ALL.into_iter().find(|p| p.data().source == ti4_engine_core::generated::Source::Pok && p.data().system.is_some()).unwrap();
    let base_only = ti4_engine_core::tables::Scope(ti4_engine_core::tables::source_bit(ti4_engine_core::generated::Source::Base));
    let s = four().state();
    assert_eq!(board::system_of(&s, base_only, pok), None);
    let mut c = s.clone();
    c.board_mut().planet_mut(pok).control = Some(Seat(0));
    assert_eq!(board::system_of(&c, base_only, pok), system_of(pok).into(), "controlled: it is in play whatever the scope");
    let mut e = s.clone();
    e.board_mut().planet_mut(pok).flags.set(PlanetFlags::UNITS_ENTRY, true);
    assert_eq!(board::system_of(&e, base_only, pok), Some(system_of(pok)));
    assert_eq!(board::system_of(&s, ALL_SOURCES, pok), Some(system_of(pok)));
    let sys = system_of(pok);
    assert!(!board::planets_in(&s, base_only, sys).as_slice().contains(&pok));
    assert!(board::planets_in(&s, ALL_SOURCES, sys).as_slice().contains(&pok));
}

#[test]
fn unit_and_token_presence_queries_see_space_and_planets_separately() {
    let systems = plain_systems();
    let (sys, planet) = (systems[2], systems[2].data().planets[0]);
    let s = four().unit(sys, None, Unit::CruiserI, 0, 2).unit(systems[3], None, Unit::DestroyerI, 1, 1).unit(sys, Some(planet), Unit::InfantryI, 2, 1).command_token(systems[4], 3).state();
    assert_eq!(board::space_units(&s, sys).len(), 2);
    assert_eq!(board::planet_units(&s, planet).len(), 1);
    assert!(board::has_space_units_of(&s, sys, Seat(0)) && !board::has_space_units_of(&s, sys, Seat(2)), "infantry on the planet is not in space");
    assert!(board::has_token(&s, systems[4], Seat(3)) && !board::has_token(&s, systems[4], Seat(0)));
    assert_eq!(board::systems_with_token(&s, Seat(3)).iter().collect::<Vec<_>>(), vec![systems[4].index()]);
    assert_eq!(board::systems_with_units_of(&s, Seat(0)).iter().collect::<Vec<_>>(), vec![sys.index()]);
    assert_eq!(board::systems_with_units_of(&s, Seat(2)).len(), 0, "the lists of systems look at space only");
    assert_eq!(board::systems_with_units_other_than(&s, Seat(0)).iter().collect::<Vec<_>>(), vec![systems[3].index()]);
}

#[test]
fn coexistence_counts_only_in_the_planets_home_system() {
    let systems = plain_systems();
    let (home, planet) = (systems[0], systems[0].data().planets[0]);
    let mut s = four().control(planet, 0).state();
    assert!(!board::in_coexistence(&s, home, planet));
    s.board_mut().planet_mut(planet).coexist.insert(Seat(2));
    assert!(board::in_coexistence(&s, home, planet) && board::is_coexisting(&s, home, planet, Seat(2)) && !board::is_coexisting(&s, home, planet, Seat(1)));
    assert_eq!(board::coexisters(&s, home, planet).len(), 1);
    assert!(!board::in_coexistence(&s, systems[1], planet), "another system");
    assert!(board::controls_for_scoring(&s, home, planet, Seat(0)), "the controller");
    assert!(board::controls_for_scoring(&s, home, planet, Seat(2)), "a coexisting seat");
    assert!(!board::controls_for_scoring(&s, home, planet, Seat(1)));
    assert!(!board::controls_for_scoring(&s, systems[1], planet, Seat(0)), "scored in the wrong system");
}

#[test]
fn neighbours_come_from_the_map_slice_and_a_state_without_a_map_has_none() {
    let s = four().state();
    let systems = plain_systems();
    assert_eq!(board::neighbours(&s, systems[0]).map(|n| n.len()), Some(6));
    assert!(board::adjacency(&s).is_some());
    let mut bare = s.clone();
    bare.map = Default::default();
    assert_eq!(board::neighbours(&bare, systems[0]), None);
    assert!(board::adjacency(&bare).is_none());
    let _ = System::COUNT;
}
