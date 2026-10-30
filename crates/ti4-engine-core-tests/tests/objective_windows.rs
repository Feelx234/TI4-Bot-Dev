//! D4-2a: `rules::objective_windows` (score an objective, the secret objective hand limit) and `rules::strategy_windows` (Trade primary).

use ti4_engine_core::choice::objectives::{Offer, Opt as OOpt};
use ti4_engine_core::generated::{PublicObjective, SecretObjective};
use ti4_engine_core::query::objectives as q;
use ti4_engine_core::rules::objective_windows::{HandLimit, ScoreOne};
use ti4_engine_core::rules::strategy_windows::{TradePrimary, TRADE_PRIMARY_GOODS};
use ti4_engine_core::state::ObjectiveRef;
use ti4_engine_core_tests::prelude::*;

fn table() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec, Faction::TheBaronyOfLetnev])
}

fn free_public() -> PublicObjective {
    PublicObjective::ALL.into_iter().find(|o| q::cost_of(*o).is_none() && q::points_of(*o) >= 1).expect("an unpriced objective")
}

fn free_secrets(n: usize) -> Vec<SecretObjective> {
    SecretObjective::ALL.into_iter().filter(|s| !matches!(s, SecretObjective::DestroyHereticalWorks | SecretObjective::FormASpyNetwork)).take(n).collect()
}

fn vp(s: &CoreState, seat: u8) -> i16 {
    s.players().by_seat(Seat(seat)).unwrap().econ.victory_points
}

fn score_rig(status: bool, offers: &[Offer], prepare: impl Fn(&mut CoreState)) -> Rig<ScoreOne> {
    let mut s = table().state();
    prepare(&mut s);
    Rig::from_parts(s, ti4_engine_core_tests::kernel_all(), ScoreOne::new(Seat(0), status, offers, None))
}

// ---- ScoreOne ---------------------------------------------------------------------------------

#[test]
fn scoring_a_public_objective_adds_its_points_and_records_the_score() {
    covers!("rules::objective_windows::ScoreOne");
    let o = free_public();
    let offer = Offer::Objective(ObjectiveRef::Public(o));
    let mut rig = score_rig(true, &[offer], |s| s.objectives_mut().revealed.push(ObjectiveRef::Public(o)));
    rig.run();
    assert_eq!(rig.opts(), vec![Opt::Objectives(OOpt::Score(ObjectiveRef::Public(o))), Opt::Decline]);
    rig.assert_options_sound();
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.scored, Some(ObjectiveRef::Public(o)));
    assert_eq!(vp(&rig.state, 0), i16::from(q::points_of(o)));
    assert!(rig.state.objectives().scored[0].public.contains(o) && !rig.state.objectives().scored[1].public.contains(o));
    assert_eq!(rig.state.ledger.vp.len(), 1);
    assert!(!rig.state.meta().finished);
    ti4_engine_core_tests::assert_snapshot("objective_windows/score_public", &rig.snapshot());
}

#[test]
fn declining_to_score_changes_nothing() {
    covers!("rules::objective_windows::ScoreOne");
    let o = free_public();
    let mut rig = score_rig(false, &[Offer::Objective(ObjectiveRef::Public(o))], |s| s.objectives_mut().revealed.push(ObjectiveRef::Public(o)));
    rig.run().decline();
    assert!(rig.is_done() && rig.driver.window.scored.is_none());
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

#[test]
fn an_objective_that_was_not_offered_cannot_be_scored() {
    covers!("rules::objective_windows::ScoreOne");
    let o = free_public();
    let other = PublicObjective::ALL.into_iter().find(|x| *x != o && q::cost_of(*x).is_none()).unwrap();
    let mut rig = score_rig(true, &[Offer::Objective(ObjectiveRef::Public(o))], |_| {});
    rig.run();
    rig.expect_illegal(Pick::Opt(Opt::Objectives(OOpt::Score(ObjectiveRef::Public(other)))));
    rig.expect_illegal(Pick::Opt(Opt::Objectives(OOpt::Plot { secret: SecretObjective::ALL[0], token: Seat(0) })));
}

#[test]
fn the_tenth_point_ends_the_game_and_points_never_pass_ten() {
    covers!("rules::objective_windows::ScoreOne");
    let o = PublicObjective::ALL.into_iter().filter(|x| q::cost_of(*x).is_none()).max_by_key(|x| q::points_of(*x)).unwrap();
    let mut rig = score_rig(true, &[Offer::Objective(ObjectiveRef::Public(o))], |s| {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.victory_points = 9;
    });
    rig.run().pick(0);
    assert_eq!(vp(&rig.state, 0), 10, "capped at the target");
    assert!(rig.state.meta().finished, "reaching ten finishes the game");
    let row = rig.state.ledger.vp.last().cloned().unwrap();
    assert_eq!(row.delta, 1, "the ledger row carries what was applied, not the printed points");
}

#[test]
fn a_secret_objective_in_hand_is_scored_and_leaves_the_hand_a_stale_offer_is_refused() {
    covers!("rules::objective_windows::ScoreOne");
    let secret = free_secrets(2);
    let offer = Offer::Objective(ObjectiveRef::Secret(secret[0]));
    let mut rig = score_rig(false, &[offer], |s| {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(secret[0]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(secret[1]);
    });
    rig.run().pick(0);
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.secrets.as_slice(), &[secret[1]]);
    assert!(rig.state.objectives().scored[0].secret.contains(secret[0]));
    assert_eq!(vp(&rig.state, 0), i16::from(q::secret_points(secret[0])));
    let mut stale = score_rig(false, &[offer], |_| {});
    stale.run();
    assert_eq!(stale.try_answer(Pick::Opt(Opt::Objectives(OOpt::Score(ObjectiveRef::Secret(secret[0]))))).map(|_| ()), Err(DriverError::Illegal), "not in the hand");
    assert_eq!(delta(stale.start_state(), &stale.state), "");
}

#[test]
fn a_secret_with_a_price_is_not_scored_without_a_price_environment() {
    covers!("rules::objective_windows::ScoreOne");
    let priced = SecretObjective::DestroyHereticalWorks;
    let offer = Offer::Objective(ObjectiveRef::Secret(priced));
    let mut rig = score_rig(false, &[offer], |s| s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(priced));
    rig.run();
    assert_eq!(rig.try_answer(Pick::Opt(Opt::Objectives(OOpt::Score(ObjectiveRef::Secret(priced))))).map(|_| ()), Err(DriverError::Illegal));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.secrets.len(), 1, "still in hand");
}

#[test]
fn scoring_smoke() {
    let o = free_public();
    let s = free_secrets(1)[0];
    smoke("objective_windows/score_smoke", || {
        score_rig(true, &[Offer::Objective(ObjectiveRef::Public(o)), Offer::Objective(ObjectiveRef::Secret(s))], |st| {
            st.objectives_mut().revealed.push(ObjectiveRef::Public(o));
            st.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(s);
        })
    });
}

// ---- HandLimit --------------------------------------------------------------------------------

fn hand_rig(n: usize, scored: usize) -> Rig<HandLimit> {
    let secrets = free_secrets(n + scored);
    let mut s = table().state();
    for x in &secrets[..n] {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(*x);
    }
    for x in &secrets[n..] {
        s.objectives_mut().scored[0].secret.insert(*x);
    }
    Rig::from_parts(s, ti4_engine_core_tests::kernel_all(), HandLimit::new(Seat(0)))
}

#[test]
fn a_hand_of_three_secrets_is_within_the_limit_and_asks_nothing() {
    covers!("rules::objective_windows::HandLimit");
    let mut rig = hand_rig(3, 0);
    rig.run();
    assert!(rig.is_done());
    assert_eq!(delta(rig.start_state(), &rig.state), "");
    let mut with_scored = hand_rig(2, 1);
    with_scored.run();
    assert!(with_scored.is_done(), "scored secrets count: two in hand and one scored is three");
}

#[test]
fn four_secrets_ask_which_goes_back_to_the_bottom_of_the_deck() {
    covers!("rules::objective_windows::HandLimit");
    let mut rig = hand_rig(4, 0);
    rig.run();
    assert_eq!(rig.asked().opts.len(), 4);
    rig.assert_options_sound();
    let first = rig.opts()[0];
    rig.expect_illegal(Pick::Opt(Opt::Objectives(OOpt::Return(SecretObjective::ALL[SecretObjective::ALL.len() - 1]))));
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.secrets.len(), 3);
    let Opt::Objectives(OOpt::Return(returned)) = first else { panic!("a return option") };
    assert_eq!(rig.state.decks().secret.cards().last(), Some(&returned), "to the bottom");
    ti4_engine_core_tests::assert_snapshot("objective_windows/hand_limit_four", &rig.snapshot());
}

#[test]
fn two_over_the_limit_asks_twice_and_one_unscored_secret_over_it_returns_without_a_question() {
    covers!("rules::objective_windows::HandLimit");
    let mut rig = hand_rig(5, 0);
    rig.run().pick(0);
    assert!(!rig.is_done(), "still over");
    rig.pick(0);
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().cards.secrets.len(), 3);
    let mut single = hand_rig(1, 3);
    single.run();
    assert!(single.is_done(), "one unscored secret over the limit goes back at once");
    assert!(single.state.players().by_seat(Seat(0)).unwrap().cards.secrets.is_empty());
    assert_eq!(single.state.decks().secret.len(), 1);
}

#[test]
fn hand_limit_smoke() {
    smoke_quiet(|| hand_rig(5, 0));
}

// ---- Trade primary ----------------------------------------------------------------------------

fn trade_table() -> Scenario {
    Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheArborec, Faction::TheYinBrotherhood, Faction::TheEmbersOfMuaat]).commodities(0, 0).commodities(1, 0).commodities(3, 0)
}

#[test]
fn trade_primary_pays_three_goods_replenishes_the_holder_and_offers_those_who_need_commodities() {
    covers!("rules::strategy_windows::TradePrimary");
    let sc = trade_table().commodities(2, i16::from(Faction::TheYinBrotherhood.data().commodities));
    let limit = |seat: u8, sc: &Scenario| i16::from(sc.faction(seat).data().commodities);
    let mut rig = Rig::new(sc.clone(), TradePrimary::new(Seat(0)));
    rig.run();
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, TRADE_PRIMARY_GOODS as i16);
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.commodities, limit(0, &sc));
    assert_eq!(rig.opts(), vec![Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Replenish(Seat(1))), Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Replenish(Seat(3))), Opt::Done], "Yin is full, the holder is not offered");
    rig.expect_illegal(Pick::Opt(Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Replenish(Seat(2)))));
    rig.expect_illegal(Pick::Opt(Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Replenish(Seat(0)))));
    rig.pick(0);
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().econ.commodities, limit(1, &sc));
    assert!(!rig.is_done());
    assert_eq!(rig.opts(), vec![Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Replenish(Seat(3))), Opt::Done], "a replenished seat is no longer offered");
    rig.pick_opt(Opt::Done);
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(3)).unwrap().econ.commodities, 0, "stopping leaves the rest as they are");
}

#[test]
fn trade_primary_ends_by_itself_when_nobody_else_needs_commodities() {
    covers!("rules::strategy_windows::TradePrimary");
    let mut sc = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheArborec]);
    sc = sc.commodities(1, i16::from(Faction::TheArborec.data().commodities));
    let mut rig = Rig::new(sc, TradePrimary::new(Seat(0)));
    rig.run();
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 3);
}

#[test]
fn trade_primary_smoke() {
    smoke("objective_windows/trade_primary", || Rig::new(trade_table(), TradePrimary::new(Seat(0))));
}
