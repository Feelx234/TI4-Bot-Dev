//! D4-2c: `kernel::fx` (the state-changing primitives every rule area shares): economy, victory points, units, tokens, planets, laws, staging.

use ti4_engine_core::choice::strategy::Pool;
use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{Agenda, PublicObjective, SecretObjective, StrategyCard};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::state::{Code, EventKind, LedgerReason, NoteKey, ObjectiveRef, Outcome, PlanetFlags, SysFlags};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{first_planet, plain_systems};

fn table() -> Scenario {
    Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheArborec, Faction::TheYinBrotherhood]).map_ring(7)
}

fn econ(s: &CoreState, seat: u8) -> ti4_engine_core::state::PlayerEcon {
    s.players().by_seat(Seat(seat)).unwrap().econ
}

// ---- economy ----------------------------------------------------------------------------------

#[test]
fn trade_goods_are_gained_in_full_spent_only_when_there_are_enough_and_never_below_zero() {
    let mut s = table().goods(0, 3).state();
    assert_eq!(fx::gain_trade_goods(&mut s, Seat(0), 4), 4);
    assert_eq!(econ(&s, 0).trade_goods, 7);
    assert_eq!(fx::gain_trade_goods(&mut s, Seat(0), 0), 0);
    assert_eq!(fx::gain_trade_goods(&mut s, Seat(0), -3), 0, "a negative gain is none");
    assert_eq!(fx::gain_trade_goods(&mut s, Seat(9), 3), 0, "no such seat");
    assert!(fx::spend_trade_goods(&mut s, Seat(0), 7));
    assert_eq!(econ(&s, 0).trade_goods, 0);
    assert!(!fx::spend_trade_goods(&mut s, Seat(0), 1), "none left");
    assert!(fx::spend_trade_goods(&mut s, Seat(0), 0) && fx::spend_trade_goods(&mut s, Seat(0), -2), "spending nothing always works");
    assert!(!fx::spend_trade_goods(&mut s, Seat(9), 1));
    assert_eq!(econ(&s, 0).trade_goods, 0);
}

#[test]
fn trade_goods_saturate_at_the_counter_limit() {
    let mut s = table().goods(0, i16::MAX - 1).state();
    assert_eq!(fx::gain_trade_goods(&mut s, Seat(0), 10), 1, "only one more fits");
    assert_eq!(econ(&s, 0).trade_goods, i16::MAX);
}

#[test]
fn replenishing_fills_the_commodities_to_the_factions_limit_and_reports_what_was_added() {
    let limit = i16::from(Faction::TheEmiratesOfHacan.data().commodities);
    let mut s = table().commodities(0, 1).state();
    assert_eq!(fx::replenish_commodities(&mut s, Seat(0)), i32::from(limit - 1));
    assert_eq!(econ(&s, 0).commodities, limit);
    assert_eq!(fx::replenish_commodities(&mut s, Seat(0)), 0, "already full");
    assert_eq!(fx::replenish_commodities(&mut s, Seat(9)), 0);
}

#[test]
fn an_announced_gain_carries_an_event_only_when_something_was_gained() {
    let mut s = table().state();
    let (n, rec) = fx::gain_trade_goods_announced(&mut s, Seat(0), 2, Code(5));
    assert_eq!(n, 2);
    let rec = rec.expect("an event");
    assert_eq!(rec.kind, EventKind::TradeGoodsGained);
    assert!(!fx::has_staged(&s));
    let (n, rec) = fx::gain_trade_goods_announced(&mut s, Seat(0), 0, Code(5));
    assert_eq!((n, rec), (0, None));
    assert_eq!(fx::gain_trade_goods_staged(&mut s, Seat(1), 3, Code(5)), 3);
    assert!(fx::has_staged(&s));
    assert_eq!(fx::gain_trade_goods_staged(&mut s, Seat(1), 0, Code(5)), 0);
    let drained = fx::drain_staged(&mut s);
    assert_eq!(drained.len(), 1, "the zero gain staged nothing");
    assert!(!fx::has_staged(&s) && fx::drain_staged(&mut s).is_empty());
}

// ---- victory points ---------------------------------------------------------------------------

#[test]
fn adjusted_points_stop_at_the_target_and_at_zero_and_the_ledger_records_what_happened() {
    let mut s = table().state();
    assert_eq!(fx::VICTORY_TARGET, 10);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.victory_points = 8;
    assert_eq!(fx::adjust_victory_points(&mut s, Seat(0), 5, LedgerReason::Objective), 2, "capped at ten");
    assert_eq!(econ(&s, 0).victory_points, 10);
    assert_eq!(fx::adjust_victory_points(&mut s, Seat(0), 1, LedgerReason::Objective), 0, "a row of zero is still written");
    assert_eq!(fx::adjust_victory_points(&mut s, Seat(0), -4, LedgerReason::Objective), -4);
    assert_eq!(fx::adjust_victory_points(&mut s, Seat(0), -9, LedgerReason::Objective), -6, "floored at zero");
    assert_eq!(econ(&s, 0).victory_points, 0);
    let rows: Vec<i32> = s.ledger.vp.iter().map(|r| r.delta).collect();
    assert_eq!(rows, vec![2, 0, -4, -6]);
    assert_eq!(fx::adjust_victory_points(&mut s, Seat(9), 3, LedgerReason::Objective), 0);
    assert_eq!(s.ledger.vp.len(), 4, "no row for a seat that is not there");
}

#[test]
fn finishing_needs_a_player_at_the_target() {
    let mut s = table().state();
    fx::finish_if_won(&mut s);
    assert!(!s.meta().finished);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().econ.victory_points = 9;
    fx::finish_if_won(&mut s);
    assert!(!s.meta().finished);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().econ.victory_points = 10;
    fx::finish_if_won(&mut s);
    assert!(s.meta().finished);
}

#[test]
fn scoring_records_the_objective_for_the_seat_and_pays_its_points() {
    let o = PublicObjective::ALL.into_iter().find(|o| ti4_engine_core::query::objectives::points_of(*o) == 2).expect("a two point objective");
    let mut s = table().state();
    assert_eq!(fx::score_public(&mut s, Seat(1), ObjectiveRef::Public(o)), 2);
    assert!(s.objectives().scored[1].public.contains(o) && s.objectives().scored_present.contains(Seat(1)));
    assert!(!s.objectives().scored[0].public.contains(o));
    assert_eq!(econ(&s, 1).victory_points, 2);
    let secret = SecretObjective::ALL[0];
    s.players_mut().by_seat_mut(Seat(1)).unwrap().cards.secrets.push(secret);
    assert!(fx::score_secret(&mut s, Seat(1), secret));
    assert!(s.players().by_seat(Seat(1)).unwrap().cards.secrets.is_empty() && s.objectives().scored[1].secret.contains(secret));
    assert!(!fx::score_secret(&mut s, Seat(1), secret), "no longer in the hand");
    assert!(!fx::score_secret(&mut s, Seat(0), SecretObjective::ALL[1]));
}

#[test]
fn an_occurrence_is_recorded_once_per_seat() {
    let mut s = table().state();
    fx::record_occurrence_score(&mut s, Seat(0), 4);
    fx::record_occurrence_score(&mut s, Seat(0), 4);
    fx::record_occurrence_score(&mut s, Seat(1), 4);
    assert_eq!(s.objectives().scored_feats.len(), 2);
}

#[test]
fn secrets_are_drawn_from_the_top_returned_to_the_bottom_and_made_public_once() {
    let mut s = table().state();
    s.decks_mut().secret = Deck::new(vec![SecretObjective::ALL[0], SecretObjective::ALL[1]]);
    assert_eq!(fx::draw_secret(&mut s, Seat(0)), Some(SecretObjective::ALL[0]));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.secrets.as_slice(), &[SecretObjective::ALL[0]]);
    fx::return_secret(&mut s, Seat(0), SecretObjective::ALL[0]);
    assert!(s.players().by_seat(Seat(0)).unwrap().cards.secrets.is_empty());
    assert_eq!(s.decks().secret.cards(), &[SecretObjective::ALL[1], SecretObjective::ALL[0]]);
    assert!(fx::make_public(&mut s, SecretObjective::ALL[1]));
    assert!(!fx::make_public(&mut s, SecretObjective::ALL[1]), "already public");
    let mut empty = table().state();
    assert_eq!(fx::draw_secret(&mut empty, Seat(0)), None);
}

#[test]
fn a_stage_card_comes_from_the_objective_deck_in_the_stage_asked_and_is_revealed() {
    let one = PublicObjective::ALL.into_iter().find(|o| ti4_engine_core::query::objectives::stage_of(*o) == Some(1)).unwrap();
    let two = PublicObjective::ALL.into_iter().find(|o| ti4_engine_core::query::objectives::stage_of(*o) == Some(2)).unwrap();
    let mut s = table().state();
    s.decks_mut().objective = Deck::new(vec![two, one]);
    assert_eq!(fx::reveal_stage(&mut s, 1), Some(one));
    assert_eq!(s.decks().objective.cards(), &[two]);
    assert_eq!(s.objectives().revealed.as_slice(), &[ObjectiveRef::Public(one)]);
    assert_eq!(fx::reveal_stage(&mut s, 1), None, "no stage one left");
    assert_eq!(fx::reveal_stage(&mut s, 2), Some(two));
}

// ---- units and tokens -------------------------------------------------------------------------

#[test]
fn placing_a_unit_marks_the_system_and_a_planet_entry_and_removing_takes_the_first_of_the_kind() {
    let (system, planet) = first_planet();
    let mut s = table().state();
    fx::place_unit(&mut s, Seat(0), system, None, Unit::CruiserI);
    assert!(s.board().sys(system).flags.has(SysFlags::TOUCHED));
    fx::place_unit(&mut s, Seat(0), system, Some(planet), Unit::InfantryI);
    fx::place_unit(&mut s, Seat(1), system, Some(planet), Unit::InfantryI);
    fx::place_unit(&mut s, Seat(0), system, Some(planet), Unit::InfantryI);
    assert!(s.board().planet(planet).flags.has(PlanetFlags::UNITS_ENTRY));
    assert_eq!(s.board().planet(planet).units.len(), 3);
    assert!(fx::remove_unit(&mut s, Seat(0), system, Some(planet), Unit::InfantryI));
    let left: Vec<u8> = s.board().planet(planet).units.as_slice().iter().map(|p| p.owner().0).collect();
    assert_eq!(left, vec![1, 0], "the first infantry of seat 0 went");
    assert!(!fx::remove_unit(&mut s, Seat(0), system, None, Unit::DestroyerI), "none of that kind");
    assert!(fx::remove_unit(&mut s, Seat(0), system, None, Unit::CruiserI));
    assert!(s.board().sys(system).units.is_empty());
    assert!(!fx::remove_unit(&mut s, Seat(2), system, Some(planet), Unit::InfantryI), "another seat's unit only");
}

#[test]
fn a_command_token_is_placed_once_per_system_and_removal_reports_whether_it_was_there() {
    let systems = plain_systems();
    let mut s = table().state();
    assert!(fx::place_command_token(&mut s, Seat(0), systems[0]));
    assert!(!fx::place_command_token(&mut s, Seat(0), systems[0]), "already there");
    assert!(fx::place_command_token(&mut s, Seat(1), systems[0]));
    assert!(s.board().sys(systems[0]).flags.has(SysFlags::TOUCHED));
    assert!(fx::remove_command_token(&mut s, Seat(0), systems[0]));
    assert!(!fx::remove_command_token(&mut s, Seat(0), systems[0]));
    assert!(s.board().sys(systems[0]).cmd.contains(Seat(1)), "the other seat's token stays");
}

#[test]
fn tokens_are_gained_into_a_pool_up_to_what_is_in_the_reinforcements() {
    let systems = plain_systems();
    let mut s = table().tokens(0, 4, 4, 4).command_token(systems[0], 0).state();
    assert_eq!(fx::tokens_in_reinforcements(&s, Seat(0)), 16 - 12 - 1);
    assert_eq!(fx::gain_token(&mut s, Seat(0), Pool::Fleet, 2), 2);
    assert_eq!(econ(&s, 0).fleet_tokens, 6);
    assert_eq!(fx::gain_token(&mut s, Seat(0), Pool::Strategic, 5), 1, "only one left");
    assert_eq!(econ(&s, 0).strategic_tokens, 5);
    assert_eq!(fx::gain_token(&mut s, Seat(0), Pool::Tactic, 1), 0, "none left");
    assert_eq!(econ(&s, 0).tactic_tokens, 4);
    assert_eq!(fx::gain_token(&mut s, Seat(0), Pool::Tactic, -2), 0);
    assert_eq!(fx::tokens_in_reinforcements(&s, Seat(1)), 16);
}

#[test]
fn a_strategic_token_is_spent_only_from_the_strategic_pool() {
    let mut s = table().tokens(0, 3, 3, 1).state();
    assert!(fx::spend_strategic_token(&mut s, Seat(0)));
    assert!(!fx::spend_strategic_token(&mut s, Seat(0)));
    assert_eq!(econ(&s, 0).tactic_tokens, 3);
    assert!(!fx::spend_strategic_token(&mut s, Seat(9)));
}

#[test]
fn drawn_action_cards_join_the_hand_in_deck_order_and_the_deck_may_run_out() {
    use ti4_engine_core::generated::ActionCard;
    let mut s = table().state();
    s.decks_mut().action = Deck::new(vec![ActionCard::ALL[0], ActionCard::ALL[1], ActionCard::ALL[2]]);
    assert_eq!(fx::draw_action_cards(&mut s, Seat(0), 2, false), 2);
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), &[ActionCard::ALL[0], ActionCard::ALL[1]]);
    assert_eq!(fx::draw_action_cards(&mut s, Seat(0), 5, false), 1, "one card left");
    assert_eq!(fx::draw_action_cards(&mut s, Seat(0), 1, false), 0);
    let mut unofficial = table().state();
    unofficial.decks_mut().action = Deck::new(vec![ActionCard::Veto3, ActionCard::ALL[0], ActionCard::Veto4]);
    assert_eq!(fx::draw_action_cards(&mut unofficial, Seat(1), 3, true), 1, "the unofficial cards leave the deck first");
    assert_eq!(unofficial.players().by_seat(Seat(1)).unwrap().cards.hand.as_slice(), &[ActionCard::ALL[0]]);
}

#[test]
fn a_technology_is_granted_once() {
    let mut s = table().state();
    assert!(fx::grant_technology(&mut s, Seat(0), Technology::GravityDrive));
    assert!(!fx::grant_technology(&mut s, Seat(0), Technology::GravityDrive));
    assert!(!fx::grant_technology(&mut s, Seat(9), Technology::GravityDrive));
}

// ---- planets ----------------------------------------------------------------------------------

#[test]
fn exhausting_and_readying_report_whether_the_state_changed() {
    let (_, planet) = first_planet();
    let mut s = table().state();
    assert!(fx::exhaust_planet(&mut s, planet));
    assert!(!fx::exhaust_planet(&mut s, planet), "already exhausted");
    assert!(fx::ready_planet(&mut s, planet));
    assert!(!fx::ready_planet(&mut s, planet));
}

// ---- laws, strategy cards, notes --------------------------------------------------------------

#[test]
fn repealing_a_law_removes_it_and_political_censure_costs_its_holder_a_point() {
    let mut s = table().state();
    s.agenda_mut().laws.insert(Agenda::FleetRegulations, Outcome::Empty);
    s.agenda_mut().laws.insert(Agenda::PoliticalCensure, Outcome::Seat(Seat(1)));
    s.players_mut().by_seat_mut(Seat(1)).unwrap().econ.victory_points = 3;
    assert!(fx::repeal_law(&mut s, Agenda::FleetRegulations));
    assert!(!fx::repeal_law(&mut s, Agenda::FleetRegulations), "not in play any more");
    assert_eq!(econ(&s, 1).victory_points, 3);
    assert!(fx::repeal_law(&mut s, Agenda::PoliticalCensure));
    assert_eq!(econ(&s, 1).victory_points, 2, "the censured player loses the point");
    assert!(s.agenda().laws.is_empty());
}

#[test]
fn a_dealt_strategy_card_joins_the_holding_in_initiative_order() {
    let mut s = table().state();
    let (a, b, c) = (StrategyCard::ALL[0], StrategyCard::ALL[1], StrategyCard::ALL[2]);
    for (i, card) in [a, b, c].into_iter().enumerate() {
        s.strategy_mut().initiative.insert(card, i as i32 + 1);
    }
    assert!(fx::deal_strategy_card(&mut s, Seat(0), c));
    assert!(fx::deal_strategy_card(&mut s, Seat(0), a));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().strat.as_slice(), &[a, c]);
    assert!(!fx::deal_strategy_card(&mut s, Seat(9), b));
}

#[test]
fn a_note_goes_home_to_its_owner_unplayed() {
    let mut s = table().state();
    let key = NoteKey { note: ti4_engine_core::generated::PromissoryNote::Ceasefire, faction: Faction::TheArborec };
    s.cards_mut().promissory.edit().insert(key, Seat(2));
    s.cards_mut().promissory_faceup.edit().insert(key, ());
    fx::give_back_note(&mut s, key);
    assert_eq!(s.cards().promissory.get(&key), Some(&Seat(1)));
    assert!(!s.cards().promissory_faceup.contains_key(&key));
    let before = s.clone();
    fx::give_back_note(&mut s, key);
    assert!(s == before, "already with its owner");
    let stranger = NoteKey { note: ti4_engine_core::generated::PromissoryNote::Ceasefire, faction: Faction::TheNomad };
    fx::give_back_note(&mut s, stranger);
    assert!(s == before, "nobody plays that faction");
}
