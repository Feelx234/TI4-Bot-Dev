//! D4-2a: `rules::trade` (the writes of a transaction: take, give, exchange, the per-turn records, promissory notes changing hands, Support for the
//! Throne, Trade Agreement).

use ti4_engine_core::query::trade::{NoteFacts, NoteMove, Offer, Terms};
use ti4_engine_core::rules::trade as t;
use ti4_engine_core::state::{LedgerReason, NoteKey};
use ti4_engine_core_tests::prelude::*;

fn table() -> CoreState {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheEmiratesOfHacan]).goods(0, 5).commodities(0, 2).goods(1, 1).state()
}

fn goods(s: &CoreState, who: u8) -> i16 {
    s.players().by_seat(Seat(who)).unwrap().econ.trade_goods
}

fn terms(goods: i32, commodities: i32) -> Terms {
    Terms { trade_goods: goods, commodities, ..Terms::default() }
}

#[test]
fn taking_removes_goods_commodities_the_first_matching_card_and_secret_and_spent_fragments() {
    covers!("rules::trade");
    let mut s = table();
    {
        let p = s.players_mut().by_seat_mut(Seat(0)).unwrap();
        p.cards.hand.push(ActionCard::Sabotage1);
        p.cards.hand.push(ActionCard::Sabotage1);
        p.cards.secrets.push(SecretObjective::ALL[0]);
        p.cards.fragments.set(ExploreType::Cultural, 2);
        p.cards.fragments.set(ExploreType::Hazardous, 1);
    }
    let mut give = terms(3, 1);
    give.action_card = Some(ActionCard::Sabotage1);
    give.secret = Some(SecretObjective::ALL[0]);
    give.fragments[ExploreType::Cultural as usize] = 1;
    give.fragments[ExploreType::Hazardous as usize] = 1;
    t::take(&mut s, Seat(0), &give);
    let p = s.players().by_seat(Seat(0)).unwrap();
    assert_eq!((p.econ.trade_goods, p.econ.commodities), (2, 1));
    assert_eq!(p.cards.hand.as_slice(), &[ActionCard::Sabotage1], "only the first matching card");
    assert!(p.cards.secrets.is_empty());
    assert_eq!(p.cards.fragments.get(ExploreType::Cultural), Some(1));
    assert_eq!(p.cards.fragments.get(ExploreType::Hazardous), None, "a spent kind is gone, not zero");
    t::take(&mut s, Seat(9), &give);
}

#[test]
fn giving_adds_the_cards_and_fragments_and_a_commodity_arrives_as_a_trade_good() {
    covers!("rules::trade");
    let mut s = table();
    let mut terms = terms(2, 3);
    terms.action_card = Some(ActionCard::Spy);
    terms.fragments[ExploreType::Industrial as usize] = 2;
    t::give(&mut s, Seat(1), &terms);
    let p = s.players().by_seat(Seat(1)).unwrap();
    assert_eq!(p.econ.trade_goods, 1 + 2 + 3, "commodities count as trade goods on arrival");
    assert_eq!(p.econ.commodities, 0);
    assert_eq!(p.cards.hand.as_slice(), &[ActionCard::Spy]);
    assert_eq!(p.cards.fragments.get(ExploreType::Industrial), Some(2));
}

#[test]
fn an_exchange_takes_both_sides_before_either_is_given() {
    covers!("rules::trade");
    let mut s = table();
    let offer = Offer { proposer: Seat(0), partner: Seat(1), given: terms(4, 0), received: terms(1, 0) };
    assert_eq!(t::exchange(&mut s, &offer), Some(()));
    assert_eq!((goods(&s, 0), goods(&s, 1)), (5 - 4 + 1, 1 - 1 + 4));
}

#[test]
fn a_support_for_the_throne_leg_naming_a_faction_two_seats_play_stops_the_exchange_untouched() {
    covers!("rules::trade");
    let mut s = Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheArborec]).goods(0, 5).state();
    let support = NoteFacts { payable: true, receivable: true, mv: Some(NoteMove::Support(Faction::TheArborec)) };
    let offer = Offer { proposer: Seat(0), partner: Seat(1), given: Terms { trade_goods: 2, note: Some(support), ..Terms::default() }, received: Terms::default() };
    let before = s.clone();
    assert_eq!(t::exchange(&mut s, &offer), None);
    assert!(s == before, "nothing written");
}

#[test]
fn support_for_the_throne_is_lent_once_scores_a_point_and_the_loan_ends_with_it() {
    covers!("rules::trade");
    let mut s = table();
    assert_eq!(t::receive_support(&mut s, Seat(1), Faction::TheEmbersOfMuaat), Some(true));
    assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.victory_points, 1);
    assert_eq!(s.ledger().vp.last().map(|r| (r.player, r.delta, r.reason)), Some((Seat(1), 1, LedgerReason::SupportForTheThrone)));
    assert_eq!(t::receive_support(&mut s, Seat(2), Faction::TheEmbersOfMuaat), Some(false), "already lent");
    assert_eq!(t::receive_support(&mut s, Seat(0), Faction::TheEmbersOfMuaat), Some(false), "one's own");
    assert_eq!(t::receive_support(&mut s, Seat(1), Faction::TheWinnu), Some(false), "nobody plays the faction");
    assert!(t::return_support(&mut s, Seat(0)));
    assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.victory_points, 0, "the point goes back with the card");
    assert_eq!(s.ledger().vp.last().map(|r| r.delta), Some(-1));
    assert!(!t::return_support(&mut s, Seat(0)), "no loan left");
    let rows = s.ledger().vp.len();
    s.players_mut().by_seat_mut(Seat(1)).unwrap().econ.victory_points = 0;
    t::receive_support(&mut s, Seat(1), Faction::TheEmbersOfMuaat);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().econ.victory_points = 0;
    t::return_support(&mut s, Seat(0));
    assert_eq!(s.ledger().vp.len(), rows + 1, "a return that moves no points writes no row");
}

#[test]
fn a_note_changes_hands_and_goes_home() {
    covers!("rules::trade");
    let mut s = table();
    let key = NoteKey { note: PromissoryNote::TradeAgreement, faction: Faction::TheEmbersOfMuaat };
    t::take_note(&mut s, Seat(1), key);
    assert_eq!(s.cards().promissory.get(&key), Some(&Seat(1)));
    assert_eq!(t::give_back(&mut s, key), Some(()));
    assert_eq!(s.cards().promissory.get(&key), Some(&Seat(0)), "home to its owner");
    assert!(!s.cards().promissory_faceup.contains_key(&key));
    let nobody = NoteKey { note: PromissoryNote::TradeAgreement, faction: Faction::TheWinnu };
    t::take_note(&mut s, Seat(1), nobody);
    assert_eq!(t::give_back(&mut s, nobody), Some(()), "a faction nobody plays: nothing happens");
    assert_eq!(s.cards().promissory.get(&nobody), Some(&Seat(1)));
    let twice = Scenario::new(&[Faction::TheArborec, Faction::TheArborec, Faction::TheEmbersOfMuaat]).state();
    let mut twice = twice;
    assert_eq!(t::give_back(&mut twice, NoteKey { note: PromissoryNote::TradeAgreement, faction: Faction::TheArborec }), None, "two seats play the owner's faction");
}

#[test]
fn trade_agreement_turns_the_owners_commodities_into_the_holders_trade_goods_and_goes_home() {
    covers!("rules::trade");
    let mut s = table();
    let key = NoteKey { note: PromissoryNote::TradeAgreement, faction: Faction::TheEmbersOfMuaat };
    assert_eq!(t::trade_agreement(&mut s, Seat(0)), Some(None), "nobody holds the note");
    t::take_note(&mut s, Seat(1), key);
    assert_eq!(t::trade_agreement(&mut s, Seat(0)), Some(Some((Seat(1), 2))));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.commodities, 0);
    assert_eq!(goods(&s, 1), 1 + 2);
    assert_eq!(s.cards().promissory.get(&key), Some(&Seat(0)));
    t::take_note(&mut s, Seat(1), key);
    assert_eq!(t::trade_agreement(&mut s, Seat(0)), Some(None), "no commodities to turn");
}

#[test]
fn every_foreign_copy_of_a_note_goes_home() {
    covers!("rules::trade");
    let mut s = table();
    let a = NoteKey { note: PromissoryNote::TradeAgreement, faction: Faction::TheEmbersOfMuaat };
    let b = NoteKey { note: PromissoryNote::TradeAgreement, faction: Faction::TheArborec };
    let other = NoteKey { note: PromissoryNote::Ceasefire, faction: Faction::TheEmbersOfMuaat };
    t::take_note(&mut s, Seat(1), a);
    t::take_note(&mut s, Seat(0), b);
    t::take_note(&mut s, Seat(1), other);
    assert_eq!(t::return_all_foreign(&mut s, PromissoryNote::TradeAgreement), Some(()));
    assert_eq!(s.cards().promissory.get(&a), Some(&Seat(0)));
    assert_eq!(s.cards().promissory.get(&b), Some(&Seat(1)));
    assert_eq!(s.cards().promissory.get(&other), Some(&Seat(1)), "another note is not touched");
}

#[test]
fn the_per_turn_and_per_round_records_are_kept_and_restored() {
    covers!("rules::trade");
    let mut s = table();
    t::record_transaction(&mut s, Seat(0), Seat(1));
    assert!(s.trade().transactions_this_turn.get(&Seat(0)).is_some_and(|p| p.contains(Seat(1))));
    assert!(s.trade().transactions_this_turn.get(&Seat(1)).is_some_and(|p| p.contains(Seat(0))), "both sides remember it");
    t::note_negotiation(&mut s, Seat(2));
    t::note_negotiation(&mut s, Seat(2));
    assert_eq!(s.trade().negotiations_this_action.get(&Seat(2)), Some(&2));
    assert_eq!(s.trade().negotiations_this_round.get(&Seat(2)), Some(&2));
    t::record_resolved(&mut s, Seat(0), Seat(1));
    t::record_resolved(&mut s, Seat(0), Seat(1));
    assert_eq!(s.trade().this_round.len(), 2, "duplicates are kept");
    t::clear_turn_transactions(&mut s);
    assert!(s.trade().transactions_this_turn.is_empty());
    t::clear_action_negotiations(&mut s);
    assert!(s.trade().negotiations_this_action.is_empty() && !s.trade().negotiations_this_round.is_empty());
    t::reset_round(&mut s);
    assert!(s.trade().this_round.is_empty() && s.trade().negotiations_this_round.is_empty());
}
