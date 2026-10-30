//! D4-2a: `rules::strategy_flow` (the mutation half of the strategy phase: deal, exhaust, swap, exchange, take from the mat, stock, lay out) and
//! `rules::objectives` (reveal).

use ti4_engine_core::rules::strategy_flow as flow;
use ti4_engine_core::state::{ObjectiveRef};
use ti4_engine_core_tests::prelude::*;

const A: StrategyCard = StrategyCard::Leadership;
const B: StrategyCard = StrategyCard::Politics;
const C: StrategyCard = StrategyCard::Trade;
const D: StrategyCard = StrategyCard::Warfare;

/// Three seats; the initiative of the cards is their order in `[A, B, C, D]` (1 to 4).
fn table() -> CoreState {
    let mut s = Scenario::lobby(3).state();
    for (i, c) in [A, B, C, D].into_iter().enumerate() {
        s.strategy_mut().initiative.insert(c, i as i32 + 1);
    }
    s
}

fn held(s: &CoreState, who: u8) -> Vec<StrategyCard> {
    s.players().by_seat(Seat(who)).unwrap().strat.as_slice().to_vec()
}

fn spent(s: &CoreState, who: u8, c: StrategyCard) -> bool {
    s.players().by_seat(Seat(who)).unwrap().strat_exhausted.contains(c)
}

#[test]
fn a_dealt_card_joins_the_holding_in_initiative_order_and_a_missing_seat_deals_nothing() {
    covers!("rules::strategy_flow");
    let mut s = table();
    assert!(flow::deal(&mut s, Seat(0), C));
    assert!(flow::deal(&mut s, Seat(0), A));
    assert_eq!(held(&s, 0), vec![A, C], "initiative order, not dealing order");
    assert!(!flow::deal(&mut s, Seat(9), B));
}

#[test]
fn exhausting_reports_whether_the_card_was_ready_and_swapping_hands_the_exhaustion_on() {
    covers!("rules::strategy_flow");
    let mut s = table();
    flow::deal(&mut s, Seat(0), A);
    flow::deal(&mut s, Seat(0), C);
    assert!(flow::exhaust(&mut s, Seat(0), A));
    assert!(!flow::exhaust(&mut s, Seat(0), A), "already exhausted");
    assert!(!flow::exhaust(&mut s, Seat(9), A));
    assert!(flow::swap(&mut s, Seat(0), A, B));
    assert_eq!(held(&s, 0), vec![B, C]);
    assert!(spent(&s, 0, B) && !spent(&s, 0, A), "the exhaustion goes with the swap");
    assert!(!spent(&s, 0, C));
    assert!(!flow::swap(&mut s, Seat(9), A, B));
}

#[test]
fn clearing_drops_the_cards_and_their_exhaustion() {
    covers!("rules::strategy_flow");
    let mut s = table();
    flow::deal(&mut s, Seat(1), B);
    flow::exhaust(&mut s, Seat(1), B);
    flow::clear(&mut s, Seat(1));
    assert!(held(&s, 1).is_empty() && !spent(&s, 1, B));
}

#[test]
fn an_exchange_swaps_two_cards_keeping_each_cards_own_exhaustion() {
    covers!("rules::strategy_flow");
    let mut s = table();
    flow::deal(&mut s, Seat(0), A);
    flow::deal(&mut s, Seat(1), D);
    flow::exhaust(&mut s, Seat(0), A);
    assert!(flow::exchange(&mut s, Seat(0), A, Seat(1), D));
    assert_eq!((held(&s, 0), held(&s, 1)), (vec![D], vec![A]));
    assert!(spent(&s, 1, A) && !spent(&s, 0, D), "A stays exhausted wherever it is");
    assert!(!spent(&s, 0, A) && !spent(&s, 1, D));
}

#[test]
fn an_exchange_that_cannot_be_made_changes_nothing() {
    covers!("rules::strategy_flow");
    let mut s = table();
    flow::deal(&mut s, Seat(0), A);
    flow::deal(&mut s, Seat(1), D);
    let before = s.clone();
    assert!(!flow::exchange(&mut s, Seat(0), A, Seat(0), A), "the same seat");
    assert!(!flow::exchange(&mut s, Seat(0), A, Seat(1), A), "the same card");
    assert!(!flow::exchange(&mut s, Seat(0), B, Seat(1), D), "a card the seat does not hold");
    assert!(!flow::exchange(&mut s, Seat(0), A, Seat(1), B), "a card the partner does not hold");
    assert!(!flow::exchange(&mut s, Seat(0), A, Seat(9), D), "no such seat");
    assert!(s == before);
}

#[test]
fn taking_a_card_from_the_mat_pays_the_goods_on_it_and_a_card_not_on_the_mat_is_refused() {
    covers!("rules::strategy_flow");
    let mut s = table();
    s.strategy_mut().unclaimed = [A, B].into_iter().collect();
    s.strategy_mut().goods.insert(A, 3);
    let goods = s.players().by_seat(Seat(2)).unwrap().econ.trade_goods;
    assert_eq!(flow::take_card(&mut s, Seat(2), A), Some(()));
    assert_eq!(s.players().by_seat(Seat(2)).unwrap().econ.trade_goods, goods + 3);
    assert_eq!(held(&s, 2), vec![A]);
    assert_eq!(s.strategy().unclaimed.as_slice(), &[B]);
    assert!(s.strategy().goods.get(&A).is_none(), "the goods left the card");
    assert_eq!(flow::take_card(&mut s, Seat(2), B), Some(()));
    assert_eq!(s.players().by_seat(Seat(2)).unwrap().econ.trade_goods, goods + 3, "no goods on B");
    assert_eq!(flow::take_card(&mut s, Seat(2), C), None, "C is not on the mat");
    assert_eq!(flow::take_card(&mut s, Seat(9), C), None);
}

#[test]
fn stocking_puts_one_good_on_each_card_still_on_the_mat() {
    covers!("rules::strategy_flow");
    let mut s = table();
    s.strategy_mut().unclaimed = [A, C].into_iter().collect();
    s.strategy_mut().goods.insert(C, 2);
    flow::stock_unclaimed(&mut s);
    assert_eq!((s.strategy().goods.get(&A).copied(), s.strategy().goods.get(&C).copied()), (Some(1), Some(3)));
    assert!(s.strategy().goods.get(&B).is_none(), "B is not on the mat");
}

#[test]
fn the_mat_is_laid_out_without_the_cards_a_stable_seat_keeps_and_the_overrides_lapse() {
    covers!("rules::strategy_flow");
    let mut s = table();
    flow::deal(&mut s, Seat(1), B);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().econ.stability = true;
    flow::deal(&mut s, Seat(2), C);
    s.strategy_mut().initiative_overrides.insert(Seat(2), 0);
    flow::lay_out_mat(&mut s, &[A, B, C, D]);
    assert_eq!(s.strategy().unclaimed.as_slice(), &[A, C, D], "only the stable seat's card stays off the mat");
    assert!(s.strategy().initiative_overrides.is_empty());
}

#[test]
fn the_objective_deck_reveals_the_top_card_and_the_first_card_of_a_stage() {
    covers!("rules::objectives");
    use ti4_engine_core::containers::Deck;
    use ti4_engine_core::query::objectives::stage_of;
    use ti4_engine_core::rules::objectives as obj;
    let stage = |n: u8| PublicObjective::ALL.into_iter().filter(|o| stage_of(*o) == Some(n)).take(2).collect::<Vec<_>>();
    let (ones, twos) = (stage(1), stage(2));
    let mut s = Scenario::lobby(2).state();
    s.decks_mut().objective = Deck::new(vec![twos[0], ones[0], twos[1], ones[1]]);
    assert_eq!(obj::reveal_stage(&mut s, 1), Some(ones[0]), "the first stage-one card, wherever it lies");
    assert_eq!(s.decks().objective.cards(), &[twos[0], twos[1], ones[1]]);
    assert_eq!(obj::reveal_top(&mut s), Some(twos[0]));
    assert_eq!(s.objectives().revealed.as_slice(), &[ObjectiveRef::Public(ones[0]), ObjectiveRef::Public(twos[0])]);
    assert_eq!(obj::reveal_stage(&mut s, 2), Some(twos[1]));
    assert_eq!(obj::reveal_stage(&mut s, 2), None, "no stage-two card left");
    assert_eq!(obj::reveal_top(&mut s), Some(ones[1]));
    assert_eq!(obj::reveal_top(&mut s), None, "an empty deck");
    assert_eq!(s.objectives().revealed.len(), 4);
}
