//! D4-2a: `rules::trade_window` (propose, answer, counter) over plain goods and commodities deals.

use std::cmp::Ordering;

use ti4_engine_core::choice::trade::{Ask as TAsk, Opt as TOpt, Shape};
use ti4_engine_core::query::trade_deals::DealFacts;
use ti4_engine_core::rules::trade_window::{Outcome, TradeWindow};
use ti4_engine_core_tests::prelude::*;

fn facts(a: u8, b: u8) -> DealFacts {
    DealFacts {
        note_order: |x, y| x.cmp(&y),
        card_order: |x, y| x.cmp(&y),
        unlocked: 0,
        pair: (Seat(a), Seat(b)),
        may: [true, true],
        arbiters: [false, false],
        planet_values: &[],
        free: [None, None],
    }
}

fn table() -> Scenario {
    Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheArborec, Faction::TheYinBrotherhood]).goods(0, 2).commodities(0, 2).goods(1, 1).commodities(1, 1)
}

fn goods(s: &CoreState, seat: u8) -> i16 {
    s.players().by_seat(Seat(seat)).unwrap().econ.trade_goods
}

fn comm(s: &CoreState, seat: u8) -> i16 {
    s.players().by_seat(Seat(seat)).unwrap().econ.commodities
}

fn deal(shape: Shape) -> Opt {
    Opt::Trade(TOpt::Deal(shape))
}

#[test]
fn the_proposer_is_offered_the_deals_that_both_purses_allow_and_a_way_out() {
    covers!("rules::trade_window::TradeWindow");
    let mut rig = Rig::new(table(), TradeWindow::open(Seat(0), Seat(1), facts(0, 1)));
    rig.run();
    let opts = rig.opts();
    assert_eq!(rig.asked().actor, Seat(0));
    assert!(matches!(rig.asked().ask, Ask::Trade(TAsk::Propose { proposer: Seat(0), partner: Seat(1), .. })));
    assert_eq!(opts.last(), Some(&Opt::Decline));
    for want in [Shape::Goods { give: 1, want: 1 }, Shape::Goods { give: 2, want: 0 }, Shape::Goods { give: 0, want: 1 }, Shape::Swap { n: 1 }, Shape::GiftCommodities { n: 2 }] {
        assert!(opts.contains(&deal(want)), "{want:?} is among the deals");
    }
    assert!(!opts.contains(&deal(Shape::Goods { give: 0, want: 2 })), "the partner has one trade good only");
    assert!(!opts.contains(&deal(Shape::Goods { give: 0, want: 0 })), "an empty deal is not a deal");
    rig.assert_options_sound();
}

#[test]
fn an_accepted_goods_deal_moves_the_goods_and_is_recorded_as_resolved() {
    covers!("rules::trade_window::TradeWindow");
    let mut rig = Rig::new(table(), TradeWindow::open(Seat(0), Seat(1), facts(0, 1)));
    rig.run().pick_opt(deal(Shape::Goods { give: 2, want: 1 }));
    assert_eq!(rig.asked().actor, Seat(1), "the partner answers");
    assert_eq!(rig.opts(), vec![Opt::Trade(TOpt::Accept), Opt::Trade(TOpt::Refuse), Opt::Trade(TOpt::Counter)]);
    assert_eq!((goods(&rig.state, 0), goods(&rig.state, 1)), (2, 1), "nothing moves before the answer");
    rig.pick_opt(Opt::Trade(TOpt::Accept));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.outcome, Some(Outcome::Resolved));
    assert_eq!((goods(&rig.state, 0), goods(&rig.state, 1)), (1, 2), "two went over, one came back");
    assert_eq!(rig.state.trade().this_round.as_slice(), &[(Seat(0), Seat(1))]);
    ti4_engine_core_tests::assert_snapshot("trade_window/goods_accepted", &rig.snapshot());
}

#[test]
fn a_commodity_swap_gives_each_side_the_others_commodity_as_a_trade_good() {
    covers!("rules::trade_window::TradeWindow");
    let mut rig = Rig::new(table(), TradeWindow::open(Seat(0), Seat(1), facts(0, 1)));
    rig.run().pick_opt(deal(Shape::Swap { n: 1 })).pick_opt(Opt::Trade(TOpt::Accept));
    assert!(rig.is_done());
    assert_eq!((comm(&rig.state, 0), comm(&rig.state, 1)), (1, 0), "each paid one commodity");
    assert_eq!((goods(&rig.state, 0), goods(&rig.state, 1)), (3, 2), "and received the other's as a trade good");
}

#[test]
fn refusing_ends_the_negotiation_and_declining_to_propose_offers_nothing() {
    covers!("rules::trade_window::TradeWindow");
    let mut rig = Rig::new(table(), TradeWindow::open(Seat(0), Seat(1), facts(0, 1)));
    rig.run().pick_opt(deal(Shape::Goods { give: 1, want: 1 })).pick_opt(Opt::Trade(TOpt::Refuse));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.outcome, Some(Outcome::Refused));
    assert_eq!(rig.state.players().by_seat(Seat(0)).unwrap().econ, table().state().players().by_seat(Seat(0)).unwrap().econ);
    let mut none = Rig::new(table(), TradeWindow::open(Seat(0), Seat(1), facts(0, 1)));
    none.run().decline();
    assert!(none.is_done());
    assert_eq!(none.driver.window.outcome, Some(Outcome::NothingOffered));
    assert_eq!(delta(none.start_state(), &none.state), "");
}

#[test]
fn a_counter_swaps_the_chairs_and_two_counters_end_the_haggling() {
    covers!("rules::trade_window::TradeWindow");
    let mut rig = Rig::new(table(), TradeWindow::open(Seat(0), Seat(1), facts(0, 1)));
    rig.run().pick_opt(deal(Shape::Goods { give: 1, want: 1 })).pick_opt(Opt::Trade(TOpt::Counter));
    assert!(!rig.is_done());
    assert_eq!(rig.driver.window.outcome, Some(Outcome::Countered));
    assert_eq!(rig.driver.window.parties(), (Seat(1), Seat(0)), "the counter is the same negotiation from the other chair");
    assert_eq!(rig.asked().actor, Seat(1));
    rig.pick_opt(deal(Shape::Goods { give: 1, want: 0 })).pick_opt(Opt::Trade(TOpt::Counter));
    assert!(rig.is_done(), "haggling is bounded to two rounds");
    assert_eq!(rig.driver.window.outcome, Some(Outcome::NothingOffered));
}

#[test]
fn a_deal_that_is_not_among_the_proposals_or_an_answer_before_the_proposal_is_illegal() {
    covers!("rules::trade_window::TradeWindow");
    let mut rig = Rig::new(table(), TradeWindow::open(Seat(0), Seat(1), facts(0, 1)));
    rig.run();
    rig.expect_illegal(Pick::Opt(deal(Shape::Goods { give: 3, want: 3 })));
    rig.expect_illegal(Pick::Opt(Opt::Trade(TOpt::Accept)));
    rig.expect_illegal(Pick::Opt(Opt::Done));
}

#[test]
fn a_pair_with_no_goods_or_commodities_is_offered_only_deals_in_promissory_notes() {
    covers!("rules::trade_window::TradeWindow");
    let mut rig = Rig::new(Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheArborec]), TradeWindow::open(Seat(0), Seat(1), facts(0, 1)));
    rig.run();
    let opts = rig.opts();
    assert!(opts.contains(&Opt::Decline));
    for o in &opts {
        let ok = matches!(o, Opt::Decline | Opt::Trade(TOpt::Deal(Shape::Sell { .. } | Shape::SupportSwap { .. })));
        assert!(ok, "without goods or commodities only notes can change hands, not {o:?}");
    }
    assert!(opts.len() > 1, "the notes in hand still make deals");
}

#[test]
fn an_answer_window_resumes_at_the_partners_question_and_the_single_flag_stops_after_one_outcome() {
    covers!("rules::trade_window::TradeWindow");
    let from = Faction::TheEmiratesOfHacan;
    let w = TradeWindow::at(Seat(0), Seat(1), Some(Shape::Goods { give: 1, want: 0 }), 2, facts(0, 1));
    let mut rig = Rig::new(table(), w);
    rig.run();
    assert!(matches!(rig.asked().ask, Ask::Trade(TAsk::Answer { responder: Seat(1), proposer: Seat(0), from: f, .. }) if f == from));
    rig.pick_opt(Opt::Trade(TOpt::Accept));
    assert_eq!((goods(&rig.state, 0), goods(&rig.state, 1)), (1, 2));
    let single = TradeWindow::open(Seat(0), Seat(1), facts(0, 1)).single();
    let mut rig = Rig::new(table(), single);
    rig.run().pick_opt(deal(Shape::Goods { give: 1, want: 1 }));
    assert!(rig.is_done(), "a single window ends with the proposal; the caller continues with the answer");
    assert_eq!(rig.driver.window.outcome, Some(Outcome::Offered));
    assert_eq!((goods(&rig.state, 0), goods(&rig.state, 1)), (2, 1), "nothing moved yet");
    let _ = Ordering::Equal;
}

#[test]
fn trade_window_smoke() {
    smoke("trade_window/smoke", || Rig::new(table(), TradeWindow::open(Seat(0), Seat(1), facts(0, 1))));
}
