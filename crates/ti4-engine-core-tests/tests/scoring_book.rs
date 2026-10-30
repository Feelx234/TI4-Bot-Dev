//! D4-2a: `rules::scoring_book::ScoringBook`, the order in which a scoring window asks the players, against a fake table of offers.

use std::collections::BTreeMap;

use ti4_engine_core::choice::objectives::Offer;
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{PublicObjective, SecretObjective};
use ti4_engine_core::rules::scoring_book::{Offers, Outcome, ScoringBook};
use ti4_engine_core::state::ObjectiveRef;
use ti4_engine_core_tests::prelude::*;

#[derive(Default)]
struct Table {
    publics: BTreeMap<u8, Vec<PublicObjective>>,
    secrets: BTreeMap<u8, Vec<SecretObjective>>,
    plots: BTreeMap<u8, Vec<(SecretObjective, Seat)>>,
    scored_at_occurrence: Vec<u8>,
}

impl Offers for Table {
    fn publics(&mut self, seat: Seat) -> SmallList<ObjectiveRef, 24> {
        self.publics.get(&seat.0).into_iter().flatten().map(|o| ObjectiveRef::Public(*o)).collect()
    }
    fn secrets(&mut self, seat: Seat) -> SmallList<SecretObjective, 8> {
        self.secrets.get(&seat.0).into_iter().flatten().copied().collect()
    }
    fn plots(&mut self, seat: Seat) -> SmallList<(SecretObjective, Seat), 8> {
        self.plots.get(&seat.0).into_iter().flatten().copied().collect()
    }
    fn scored_at_occurrence(&mut self, seat: Seat) -> bool {
        self.scored_at_occurrence.contains(&seat.0)
    }
}

const ORDER: [Seat; 3] = [Seat(0), Seat(1), Seat(2)];

fn p(i: usize) -> PublicObjective {
    PublicObjective::ALL[i]
}

fn s(i: usize) -> SecretObjective {
    SecretObjective::ALL[i]
}

fn public(o: PublicObjective) -> Offer {
    Offer::Objective(ObjectiveRef::Public(o))
}

fn secret(o: SecretObjective) -> Offer {
    Offer::Objective(ObjectiveRef::Secret(o))
}

#[test]
fn players_are_asked_in_the_order_given_and_those_with_nothing_on_offer_are_passed_over() {
    let mut t = Table::default();
    t.publics.insert(1, vec![p(0)]);
    t.publics.insert(2, vec![p(1)]);
    let book = ScoringBook::new(&ORDER, true, false, false);
    assert_eq!(book.pending(), &[Seat(2), Seat(1), Seat(0)], "stored last first");
    let asked = book.asked(&mut t).expect("somebody has an offer");
    assert_eq!((asked.seat, asked.offset), (Seat(1), 1), "seat 0 has nothing: the first asked is seat 1, the second in the order");
    assert_eq!(asked.offers.as_slice(), &[public(p(0))]);
    assert!(!book.is_complete());
    let nobody = Table::default();
    let mut nobody = nobody;
    assert!(ScoringBook::new(&ORDER, true, false, false).asked(&mut nobody).is_none());
}

#[test]
fn a_status_phase_player_scores_one_public_and_one_secret_and_is_asked_again_while_something_is_left() {
    let mut t = Table::default();
    t.publics.insert(0, vec![p(0), p(1)]);
    t.secrets.insert(0, vec![s(0)]);
    let mut book = ScoringBook::new(&[Seat(0), Seat(1)], true, false, false);
    let first = book.asked(&mut t).unwrap();
    assert_eq!(first.offers.as_slice(), &[public(p(0)), public(p(1)), secret(s(0))], "publics first, then secrets");
    book.after(&first, Outcome::Scored { secret: false }, &mut t);
    assert!(book.public_used().contains(Seat(0)));
    let second = book.asked(&mut t).expect("the secret is still on offer");
    assert_eq!(second.seat, Seat(0));
    assert_eq!(second.offers.as_slice(), &[secret(s(0))], "the public cap is used");
    book.after(&second, Outcome::Scored { secret: true }, &mut t);
    assert!(book.secret_used().contains(Seat(0)));
    assert!(book.asked(&mut t).is_none(), "nothing left for seat 0 and seat 1 has nothing");
    assert!(book.pending().len() <= 1, "seat 0 is done");
}

#[test]
fn a_decline_passes_the_player_by_and_a_failed_attempt_too() {
    let mut t = Table::default();
    t.publics.insert(0, vec![p(0)]);
    t.publics.insert(1, vec![p(1)]);
    for outcome in [Outcome::Declined, Outcome::Failed] {
        let mut book = ScoringBook::new(&[Seat(0), Seat(1)], true, false, false);
        let first = book.asked(&mut t).unwrap();
        assert_eq!(first.seat, Seat(0));
        book.after(&first, outcome, &mut t);
        let next = book.asked(&mut t).unwrap();
        assert_eq!(next.seat, Seat(1), "{outcome:?}");
        book.after(&next, outcome, &mut t);
        assert!(book.is_complete(), "{outcome:?}: both passed");
    }
}

#[test]
fn outside_the_status_phase_a_player_stays_until_they_decline_unless_capped_to_one() {
    let mut t = Table::default();
    t.secrets.insert(0, vec![s(0), s(1)]);
    let mut open = ScoringBook::new(&[Seat(0)], false, false, false);
    let a = open.asked(&mut t).unwrap();
    open.after(&a, Outcome::Scored { secret: true }, &mut t);
    assert_eq!(open.asked(&mut t).map(|x| x.seat), Some(Seat(0)), "still in place");
    let b = open.asked(&mut t).unwrap();
    open.after(&b, Outcome::Declined, &mut t);
    assert!(open.is_complete());
    let mut capped = ScoringBook::new(&[Seat(0)], false, true, false);
    let a = capped.asked(&mut t).unwrap();
    capped.after(&a, Outcome::Scored { secret: true }, &mut t);
    assert!(capped.is_complete(), "one per player, and no plot on offer: done after one score");
}

#[test]
fn a_secret_already_scored_at_this_occurrence_is_not_offered_again_under_the_cap() {
    let mut t = Table::default();
    t.secrets.insert(0, vec![s(0)]);
    t.scored_at_occurrence.push(0);
    assert!(ScoringBook::new(&[Seat(0)], false, true, true).asked(&mut t).is_none(), "capped by the occurrence");
    assert!(ScoringBook::new(&[Seat(0)], false, true, false).asked(&mut t).is_some(), "no occurrence: no cap");
    assert!(ScoringBook::new(&[Seat(0)], false, false, true).asked(&mut t).is_some(), "not one-per-player: no cap");
}

#[test]
fn a_plot_scored_without_a_placed_card_keeps_the_window_open_until_the_card_is_placed() {
    let mut t = Table::default();
    t.plots.insert(0, vec![(s(0), Seat(0))]);
    let mut book = ScoringBook::new(&[Seat(0)], true, false, false);
    let a = book.asked(&mut t).unwrap();
    assert_eq!(a.offers.as_slice(), &[Offer::Plot { secret: s(0), token: Seat(0) }]);
    book.after(&a, Outcome::PlotScored { token: Seat(0), placed: false }, &mut t);
    assert_eq!(book.plot_card(), Some((Seat(0), Seat(0))));
    t.plots.clear();
    assert!(book.asked(&mut t).is_none());
    assert!(!book.is_complete(), "the card still has to be placed");
    book.card_placed();
    assert_eq!(book.plot_card(), None);
}

#[test]
fn a_restored_book_asks_what_the_stored_one_would() {
    let mut t = Table::default();
    t.publics.insert(1, vec![p(0)]);
    let live = ScoringBook::new(&ORDER, true, false, false);
    let restored = ScoringBook::restore(live.pending(), live.public_used(), live.secret_used(), live.plot_card(), true, false, false);
    assert_eq!(restored, live);
    assert_eq!(restored.asked(&mut t), live.asked(&mut t));
}
