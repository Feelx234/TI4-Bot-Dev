//! D4-2a: `rules::explore` (the writes of exploration, fragments and relics).

use ti4_engine_core::containers::Deck;
use ti4_engine_core::rules::explore as ex;
use ti4_engine_core::state::{Code, LedgerReason, PlanetFlags};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{first_planet, plain_systems};

fn table() -> Scenario {
    let (system, planet) = first_planet();
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[system]).control(planet, 0)
}

fn mech() -> Unit {
    Unit::FactionlessMech
}

fn fragments(s: &mut CoreState, who: u8, counts: &[(ExploreType, i16)]) {
    let p = s.players_mut().by_seat_mut(Seat(who)).unwrap();
    for (k, n) in counts {
        p.cards.fragments.set(*k, *n);
    }
}

fn frag(s: &CoreState, who: u8, kind: ExploreType) -> Option<i16> {
    s.players().by_seat(Seat(who)).unwrap().cards.fragments.get(kind)
}

#[test]
fn three_matching_fragments_buy_the_top_relic_and_log_it_with_no_points() {
    covers!("rules::explore");
    let mut s = table().round(3).state();
    s.decks_mut().relic = Deck::new(vec![Relic::ShardOfTheThrone, Relic::TheCrownOfEmphidia]);
    fragments(&mut s, 0, &[(ExploreType::Cultural, 4)]);
    assert_eq!(ex::purge_for_relic(&mut s, Seat(0), ExploreType::Cultural), Some(Relic::ShardOfTheThrone));
    assert_eq!(frag(&s, 0, ExploreType::Cultural), Some(1));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.relics.as_slice(), &[Relic::ShardOfTheThrone]);
    let row = s.ledger().relics.last().unwrap();
    assert_eq!((row.round, row.player, row.relic, row.victory_points), (3, Seat(0), Relic::ShardOfTheThrone, 0), "copied: the Shard scores only through relic_gain");
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.victory_points, 0);
}

#[test]
fn frontier_fragments_make_up_a_shortfall_and_too_few_or_an_empty_deck_change_nothing() {
    covers!("rules::explore");
    let mut s = table().state();
    s.decks_mut().relic = Deck::new(vec![Relic::TheCrownOfEmphidia]);
    fragments(&mut s, 0, &[(ExploreType::Hazardous, 2), (ExploreType::Frontier, 1)]);
    assert_eq!(ex::purge_for_relic(&mut s, Seat(0), ExploreType::Hazardous), Some(Relic::TheCrownOfEmphidia));
    assert_eq!((frag(&s, 0, ExploreType::Hazardous), frag(&s, 0, ExploreType::Frontier)), (Some(0), Some(0)), "two matching and one frontier");
    let mut few = table().state();
    few.decks_mut().relic = Deck::new(vec![Relic::TheCrownOfEmphidia]);
    fragments(&mut few, 0, &[(ExploreType::Cultural, 2)]);
    let before = few.clone();
    assert_eq!(ex::purge_for_relic(&mut few, Seat(0), ExploreType::Cultural), None);
    assert!(few == before, "two fragments are not enough");
    let mut empty = table().state();
    fragments(&mut empty, 0, &[(ExploreType::Cultural, 3)]);
    assert_eq!(ex::purge_for_relic(&mut empty, Seat(0), ExploreType::Cultural), None, "no relic left");
    assert_eq!(frag(&empty, 0, ExploreType::Cultural), Some(3), "the fragments stay");
    assert_eq!(ex::purge_for_relic(&mut empty, Seat(9), ExploreType::Cultural), None);
}

#[test]
fn asking_for_the_frontier_trait_counts_the_pile_twice_and_can_leave_it_negative() {
    covers!("rules::explore");
    // Copied string behaviour (docs/ENGINE_KNOWN_ISSUES.md): two frontier fragments pass the check, then 3 are taken from a pile of 2.
    let mut s = table().state();
    s.decks_mut().relic = Deck::new(vec![Relic::TheCrownOfEmphidia]);
    fragments(&mut s, 0, &[(ExploreType::Frontier, 2)]);
    assert_eq!(ex::purge_for_relic(&mut s, Seat(0), ExploreType::Frontier), Some(Relic::TheCrownOfEmphidia));
    assert_eq!(frag(&s, 0, ExploreType::Frontier), Some(-1));
}

#[test]
fn a_relic_is_exhausted_once_and_only_when_held_and_purging_keeps_the_exhausted_mark() {
    covers!("rules::explore");
    let mut s = table().state();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(Relic::TheCrownOfEmphidia);
    assert!(!ex::exhaust_relic(&mut s, Seat(0), Relic::BookOfLatvinia), "not held");
    assert!(ex::exhaust_relic(&mut s, Seat(0), Relic::TheCrownOfEmphidia));
    assert!(!ex::exhaust_relic(&mut s, Seat(0), Relic::TheCrownOfEmphidia), "already exhausted");
    assert!(!ex::exhaust_relic(&mut s, Seat(9), Relic::TheCrownOfEmphidia));
    ex::purge_relic(&mut s, Seat(0), Relic::TheCrownOfEmphidia);
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert!(p.cards.relics.is_empty() && p.cards.relics_exhausted.contains(Relic::TheCrownOfEmphidia), "as the string state keeps it");
    ex::ready_relics_and_legendary(&mut s);
    assert!(!s.players().by_seat(Seat(0)).unwrap().cards.relics_exhausted.contains(Relic::TheCrownOfEmphidia));
}

#[test]
fn an_attachment_goes_on_the_planet_and_marks_its_entry() {
    covers!("rules::explore");
    let (_, planet) = first_planet();
    let mut s = table().state();
    ex::attach(&mut s, planet, Attachment::TombOfEmphidia);
    let cell = s.board().planet(planet);
    assert_eq!(cell.attach.as_slice(), &[Attachment::TombOfEmphidia]);
    assert!(cell.flags.has(PlanetFlags::ATTACH_ENTRY));
}

#[test]
fn the_frontier_tokens_become_exactly_the_given_systems_and_the_ion_storm_is_placed() {
    covers!("rules::explore");
    let systems = plain_systems();
    let mut s = table().state();
    ex::place_frontier_tokens(&mut s, &[systems[0], systems[1]]);
    ex::place_frontier_tokens(&mut s, &[systems[2]]);
    assert_eq!(s.explore().frontier.len(), 1);
    assert!(s.explore().frontier.contains(systems[2]) && !s.explore().frontier.contains(systems[0]));
    ex::place_ion_storm(&mut s, systems[1], Wormhole::ALL[0]);
    assert_eq!(s.meta().ion_storm, Some((systems[1], Wormhole::ALL[0])));
}

#[test]
fn the_crown_scores_when_held_with_the_tomb_on_a_planet_of_its_holder() {
    covers!("rules::explore");
    let (_, planet) = first_planet();
    let mut s = table().state();
    assert!(!ex::crown_point(&mut s, Seat(0)), "no crown");
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(Relic::TheCrownOfEmphidia);
    assert!(!ex::crown_point(&mut s, Seat(0)), "no tomb");
    ex::attach(&mut s, planet, Attachment::TombOfEmphidia);
    assert!(ex::crown_point(&mut s, Seat(0)));
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert!(p.cards.relics.is_empty() && p.econ.victory_points == 1);
    assert_eq!(s.ledger().vp.last().map(|r| (r.player, r.delta, r.reason)), Some((Seat(0), 1, LedgerReason::Relic(Relic::TheCrownOfEmphidia))));
    let mut other = table().state();
    other.players_mut().by_seat_mut(Seat(1)).unwrap().cards.relics.push(Relic::TheCrownOfEmphidia);
    ex::attach(&mut other, planet, Attachment::TombOfEmphidia);
    assert!(!ex::crown_point(&mut other, Seat(1)), "the tomb is on a planet of another seat");
}

#[test]
fn a_card_leaves_the_discard_pile_for_the_hand_once_per_occurrence() {
    covers!("rules::explore");
    let mut s = table().state();
    s.cards_mut().discarded_action.edit().push(ActionCard::Spy);
    s.cards_mut().discarded_action.edit().push(ActionCard::Sabotage1);
    s.cards_mut().discarded_action.edit().push(ActionCard::Spy);
    assert!(ex::take_discarded_action_card(&mut s, Seat(0), ActionCard::Spy));
    assert_eq!(s.cards().discarded_action.as_slice(), &[ActionCard::Sabotage1, ActionCard::Spy], "the first occurrence went");
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[ActionCard::Spy]);
    assert!(!ex::take_discarded_action_card(&mut s, Seat(0), ActionCard::Sabotage2));
    assert!(!ex::take_discarded_action_card(&mut s, Seat(9), ActionCard::Sabotage1));
}

#[test]
fn a_purged_planet_loses_everything_and_is_marked_and_a_legendary_card_is_spent_until_readied() {
    covers!("rules::explore");
    let (system, planet) = first_planet();
    let mut s = table().unit(system, Some(planet), Unit::InfantryI, 0, 2).state();
    ex::attach(&mut s, planet, Attachment::TombOfEmphidia);
    ex::purge_planet(&mut s, planet);
    let cell = s.board().planet(planet);
    assert!(cell.units.as_slice().is_empty() && cell.control.is_none() && cell.attach.as_slice().is_empty());
    assert!(cell.flags.has(PlanetFlags::PURGED) && !cell.flags.has(PlanetFlags::UNITS_ENTRY) && !cell.flags.has(PlanetFlags::ATTACH_ENTRY));
    ex::exhaust_legendary(&mut s, Seat(0), planet);
    assert!(s.players().by_seat(Seat(0)).unwrap().cards.legendary_exhausted.contains_key(&planet));
    ex::ready_relics_and_legendary(&mut s);
    assert!(s.players().by_seat(Seat(0)).unwrap().cards.legendary_exhausted.is_empty());
}

#[test]
fn maw_of_worlds_exhausts_every_planet_the_seat_controls() {
    covers!("rules::explore");
    let (system, planet) = first_planet();
    let mut s = table().state();
    ex::exhaust_controlled_planets(&mut s, Seat(0));
    assert!(s.board().planet(planet).flags.has(PlanetFlags::EXHAUSTED));
    let _ = system;
}

#[test]
fn the_silver_flame_scores_on_a_ten_and_otherwise_consumes_the_home_system_and_bars_public_objectives() {
    covers!("rules::explore");
    let mut s = table().state();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(Relic::TheSilverFlame);
    ex::silver_flame(&mut s, Seat(0), true);
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert_eq!((p.econ.victory_points, p.cards.relics.len(), p.econ.public_objectives_forbidden), (1, 0, false));
    let mut bad = table().state();
    bad.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(Relic::TheSilverFlame);
    let home = bad.players().by_seat(Seat(0)).unwrap().faction.data().home_system.expect("a home system");
    ex::silver_flame(&mut bad, Seat(0), false);
    let p = bad.players().by_seat(Seat(0)).unwrap();
    assert_eq!((p.econ.victory_points, p.cards.relics.len(), p.econ.public_objectives_forbidden), (0, 0, true));
    let cell = bad.board().sys(home);
    assert!(cell.flags.has(ti4_engine_core::state::SysFlags::PURGED) && cell.units.as_slice().is_empty());
}

#[test]
fn one_unit_leaves_a_planet_by_index_and_an_index_past_the_list_is_ignored() {
    covers!("rules::explore");
    let (system, planet) = first_planet();
    let mut s = table().unit(system, Some(planet), Unit::InfantryI, 0, 1).unit(system, Some(planet), mech(), 0, 1).state();
    ex::remove_unit_at(&mut s, planet, 9);
    assert_eq!(s.board().planet(planet).units.as_slice().len(), 2);
    ex::remove_unit_at(&mut s, planet, 0);
    assert_eq!(s.board().planet(planet).units.as_slice().len(), 1);
}

#[test]
fn the_record_of_an_exploration_is_opened_closed_and_given_its_system() {
    covers!("rules::explore");
    let (system, planet) = first_planet();
    let mut s = table().round(2).state();
    ex::open_record(&mut s, Seat(0), ExploreType::Cultural, Explore::ALL[0], Some(planet), None, Code(1));
    assert_eq!(s.explore().log.len(), 1);
    let closing = ex::Closing { outcome: Code(7), result: Some(Code(8)), gains: vec![(Code(9), 2)], placed: vec![Code(10)] };
    ex::close_record(&mut s, 0, &closing);
    ex::set_record_system(&mut s, 0, system);
    let rec = s.explore().log.get(0).expect("the record");
    assert_eq!((rec.round, rec.player, rec.outcome, rec.result, rec.system), (2, Seat(0), Code(7), Some(Code(8)), Some(system)));
    assert_eq!(rec.gains.get(&Code(9)), Some(&2));
    assert_eq!(rec.placed.as_slice(), &[Code(10)]);
    let before = s.clone();
    ex::close_record(&mut s, 5, &closing);
    ex::set_record_system(&mut s, 5, system);
    assert!(s == before, "an index past the log is ignored");
}

#[test]
fn mirage_is_placed_once_under_the_seats_control_and_readied() {
    covers!("rules::explore");
    let systems = plain_systems();
    let mut s = table().state();
    assert!(ex::place_planet(&mut s, Seat(1), systems[1], Planet::Mirage));
    let cell = s.board().planet(Planet::Mirage);
    assert_eq!((cell.placed_in, cell.control), (Some(systems[1]), Some(Seat(1))));
    assert!(!cell.flags.has(PlanetFlags::EXHAUSTED));
    assert!(!ex::place_planet(&mut s, Seat(0), systems[2], Planet::Mirage), "already on the board");
    assert_eq!(s.board().planet(Planet::Mirage).control, Some(Seat(1)));
}

#[test]
fn demilitarizing_sends_the_structures_home_and_the_ground_forces_to_space() {
    covers!("rules::explore");
    let (system, planet) = first_planet();
    let sc = table().unit(system, Some(planet), Unit::SpaceDockI, 0, 1).unit(system, Some(planet), Unit::InfantryI, 0, 2).unit(system, Some(planet), mech(), 1, 1);
    let mut s = sc.state();
    ex::demilitarize(&mut s, ti4_engine_core_tests::inv::ALL_SOURCES, planet);
    assert!(s.board().planet(planet).units.as_slice().is_empty(), "nothing stays on the planet");
    let space: Vec<Unit> = s.board().sys(system).units.as_slice().iter().map(|p| p.kind()).collect();
    assert_eq!(space.iter().filter(|u| **u == Unit::InfantryI).count(), 2);
    assert!(space.contains(&mech()) && !space.contains(&Unit::SpaceDockI), "the dock goes back to the reinforcements");
}
