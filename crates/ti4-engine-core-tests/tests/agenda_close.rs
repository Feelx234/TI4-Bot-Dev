//! D4-2a: `rules::agenda::close::Close` (the close of a vote: log, enactment, redirect, the riders paid for correct predictions).

use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt, Pool};
use ti4_engine_core::keys::{Prediction, PredictionTag, VoteBar};
use ti4_engine_core::rules::agenda::close::{Close, Facts};
use ti4_engine_core::state::{Code, Outcome};
use ti4_engine_core_tests::prelude::*;

const COUNTS: [(Outcome, i64); 2] = [(Outcome::For, 3), (Outcome::Against, 1)];

fn facts() -> Facts {
    Facts { trade_rider: Code(1), keleres_rider: Code(2), drop_unofficial: false, blanked: false, hooked: 0, research: None }
}

fn law() -> Agenda {
    Agenda::ALL.into_iter().find(|a| a.data().kind == AgendaType::Law && a.data().elects == AgendaElects::ForAgainst).expect("a for/against law")
}

fn directive() -> Agenda {
    Agenda::ALL.into_iter().find(|a| a.data().kind == AgendaType::Directive && a.data().elects == AgendaElects::ForAgainst).expect("a for/against directive")
}

fn close(agenda: Agenda, winner: Option<Outcome>, predictions: &[(u8, Prediction)], rules: u32) -> Rig<Close> {
    let mut s = Scenario::lobby(3).round(2).rules(rules).state();
    for (seat, p) in predictions {
        s.agenda_mut().predictions.insert(Seat(*seat), *p);
    }
    let window = Close::new(facts(), agenda, None, winner, &[(Seat(0), Outcome::For), (Seat(1), Outcome::Against)], &COUNTS, &[Outcome::For, Outcome::Against]);
    Rig::from_parts(s, kernel_all(), window)
}

fn tokens_of(rig: &Rig<Close>, seat: u8) -> i16 {
    let e = &rig.state.players().by_seat(Seat(seat)).unwrap().econ;
    e.tactic_tokens + e.fleet_tokens + e.strategic_tokens
}

#[test]
fn a_vote_without_a_winner_is_logged_and_the_predictions_are_dropped() {
    covers!("rules::agenda::close::Close");
    let mut rig = close(law(), None, &[(1, Prediction::Plain(Outcome::For))], LATEST_RULES);
    rig.run();
    assert!(rig.is_done());
    let rec = rig.state.agenda().log.last().expect("logged");
    assert_eq!((rec.outcome, rec.enacted, rec.discarded), (None, false, false));
    assert!(rig.state.agenda().predictions.is_empty());
    assert!(rig.state.agenda().laws.is_empty());
    assert_eq!(rig.driver.window.effective(), None);
}

#[test]
fn a_law_that_wins_for_is_enacted_and_one_that_loses_is_not_and_a_directive_never() {
    covers!("rules::agenda::close::Close");
    let mut won = close(law(), Some(Outcome::For), &[], LATEST_RULES);
    won.run();
    assert!(won.is_done());
    assert_eq!(won.state.agenda().laws.get(&law()), Some(&Outcome::For));
    assert!(won.state.agenda().log.last().unwrap().enacted);
    assert_eq!(won.driver.window.effective(), Some(Outcome::For));
    let mut lost = close(law(), Some(Outcome::Against), &[], LATEST_RULES);
    lost.run();
    assert!(lost.state.agenda().laws.is_empty());
    assert!(!lost.state.agenda().log.last().unwrap().enacted);
    let mut dir = close(directive(), Some(Outcome::For), &[], LATEST_RULES);
    dir.run();
    assert!(dir.state.agenda().laws.is_empty());
    ti4_engine_core_tests::assert_snapshot("agenda_close/law_won", &won.snapshot());
}

#[test]
fn a_redirect_names_the_seat_the_agenda_elects_and_is_spent() {
    covers!("rules::agenda::close::Close");
    let elector = Agenda::ALL.into_iter().find(|a| a.data().elects == AgendaElects::Player).expect("a player election");
    let mut s = Scenario::lobby(3).round(2).state();
    s.agenda_mut().elected_override = Some(Seat(2));
    let window = Close::new(facts(), elector, None, Some(Outcome::Seat(Seat(0))), &[(Seat(1), Outcome::Seat(Seat(0)))], &[(Outcome::Seat(Seat(0)), 2)], &[Outcome::Seat(Seat(0)), Outcome::Seat(Seat(2))]);
    let mut rig = Rig::from_parts(s, kernel_all(), window);
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.effective(), Some(Outcome::Seat(Seat(2))));
    assert_eq!(rig.driver.window.elected(), Some(Seat(2)));
    assert_eq!(rig.state.agenda().elected_override, None);
}

#[test]
fn the_imperial_rider_pays_a_victory_point_the_trade_rider_five_goods_and_a_wrong_guess_nothing() {
    covers!("rules::agenda::close::Close");
    let preds = [
        (0, Prediction::Plain(Outcome::For)),
        (1, Prediction::Tagged(Outcome::For, PredictionTag::TradeRider)),
        (2, Prediction::Tagged(Outcome::Against, PredictionTag::TradeRider)),
    ];
    let mut rig = close(directive(), Some(Outcome::For), &preds, LATEST_RULES);
    let vp0 = rig.state.players().by_seat(Seat(0)).unwrap().econ.victory_points;
    let goods = |r: &Rig<Close>, s: u8| r.state.players().by_seat(Seat(s)).unwrap().econ.trade_goods;
    let (g1, g2) = (goods(&rig, 1), goods(&rig, 2));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.victory_points, vp0 + 1);
    assert_eq!(goods(&rig, 1), g1 + 5);
    assert_eq!(goods(&rig, 2), g2, "a wrong prediction pays nothing");
    assert!(rig.state.agenda().predictions.is_empty());
    assert!(!rig.driver.window.unsupported());
}

#[test]
fn a_barred_vote_and_the_sanction_pay_nothing() {
    covers!("rules::agenda::close::Close");
    let preds = [(0, Prediction::Barred(VoteBar::PoliticalSecret)), (1, Prediction::Tagged(Outcome::For, PredictionTag::Sanction))];
    let mut rig = close(directive(), Some(Outcome::For), &preds, LATEST_RULES);
    let before = rig.state.clone();
    rig.run();
    assert!(rig.is_done());
    for seat in 0..3 {
        let (a, b) = (&before.players().by_seat(Seat(seat)).unwrap().econ, &rig.state.players().by_seat(Seat(seat)).unwrap().econ);
        assert_eq!((a.victory_points, a.trade_goods), (b.victory_points, b.trade_goods));
    }
}

#[test]
fn the_leadership_rider_asks_the_pool_of_each_of_three_tokens() {
    covers!("rules::agenda::close::Close");
    let mut rig = close(directive(), Some(Outcome::For), &[(1, Prediction::Tagged(Outcome::For, PredictionTag::LeadRider))], LATEST_RULES);
    let before = tokens_of(&rig, 1);
    rig.run();
    for left in [3u8, 2, 1] {
        assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::GainPool { left: l }) if l == left), "{:?}", rig.asked().ask);
        assert_eq!(rig.asked().actor, Seat(1));
        rig.assert_options_sound();
        rig.pick_opt(Opt::Strategy(SOpt::Pool(Pool::Fleet)));
    }
    assert!(rig.is_done());
    assert_eq!(tokens_of(&rig, 1), before + 3);
    let again = rig.replay();
    assert!(again.state == rig.state, "replay reproduces the run");
}

#[test]
fn riders_that_ask_their_own_questions_and_old_rules_end_the_window_unsupported() {
    covers!("rules::agenda::close::Close");
    let mut tech = close(directive(), Some(Outcome::For), &[(1, Prediction::Tagged(Outcome::For, PredictionTag::TechRider))], LATEST_RULES);
    tech.run();
    assert!(tech.is_done());
    assert!(tech.driver.window.unsupported());
    let mut old = close(directive(), Some(Outcome::For), &[(0, Prediction::Plain(Outcome::For))], 11);
    old.run();
    assert!(old.driver.window.unsupported(), "below the pool question the string result stands");
}

#[test]
fn the_close_survives_every_answer_policy() {
    covers!("rules::agenda::close::Close");
    smoke_quiet(|| close(directive(), Some(Outcome::For), &[(1, Prediction::Tagged(Outcome::For, PredictionTag::LeadRider)), (0, Prediction::Plain(Outcome::For))], LATEST_RULES));
}
