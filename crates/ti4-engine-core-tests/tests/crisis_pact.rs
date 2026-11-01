//! Lane E2: Galactic Crisis Pact's window (`rules::crisis_pact`), one player's question about the secondary of the elected card without a command token.
//! The driver composition (the speaker's order, the eligibility, the ability as a child) is checked against the string by `ti4-engine`'s
//! `agenda_live_tests`.

use ti4_engine_core::choice::generic::Rule;
use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt, Subtype as SSub};
use ti4_engine_core::choice::{Origin, Subtype};
use ti4_engine_core::rules::crisis_pact::CrisisPact;
use ti4_engine_core_tests::prelude::*;

fn table() -> Scenario {
    Scenario::lobby(3)
}

fn opt(o: SOpt) -> Opt {
    Opt::Strategy(o)
}

#[test]
fn a_seat_that_could_do_nothing_with_the_secondary_is_not_asked() {
    covers!("rules::crisis_pact::CrisisPact");
    let mut rig = Rig::new(table(), CrisisPact::new(Seat(1), StrategyCard::Trade, false));
    rig.run();
    assert!(rig.is_done() && !rig.driver.window.accepted);
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

#[test]
fn the_question_is_the_followers_without_a_token_and_a_yes_accepts_a_no_declines() {
    covers!("rules::crisis_pact::CrisisPact");
    let mut rig = Rig::new(table(), CrisisPact::new(Seat(1), StrategyCard::Trade, true));
    rig.run();
    assert_eq!(rig.asked().actor, Seat(1));
    assert_eq!(rig.asked().ask, Ask::Strategy(SAsk::CrisisPact { card: StrategyCard::Trade }));
    assert_eq!(rig.opts(), vec![opt(SOpt::Skip), opt(SOpt::Follow)]);
    assert_eq!(rig.asked().ctx.subtype, Subtype::Strategy(SSub::FollowSecondary), "the card's own follower question");
    assert!(matches!(rig.asked().ctx.origin, Origin::StrategyCard { card: StrategyCard::Trade, secondary: true }));
    rig.assert_options_sound();
    rig.pick_opt(opt(SOpt::Follow));
    assert!(rig.is_done() && rig.driver.window.accepted);
    assert_eq!(delta(rig.start_state(), &rig.state), "", "the ability is the driver's: the window changes nothing itself");
    let mut declined = Rig::new(table(), CrisisPact::new(Seat(1), StrategyCard::Trade, true));
    declined.run().pick_opt(opt(SOpt::Skip));
    assert!(declined.is_done() && !declined.driver.window.accepted);
}

#[test]
fn leadership_asks_the_purchase_question_itself() {
    covers!("rules::crisis_pact::CrisisPact");
    let mut rig = Rig::new(table(), CrisisPact::new(Seat(0), StrategyCard::Leadership, true));
    rig.run();
    assert_eq!(rig.asked().ctx.subtype, Subtype::Strategy(SSub::BuyTokenWithInfluence));
    assert!(matches!(rig.asked().ctx.origin, Origin::Rule(Rule::R52_3)));
}

#[test]
fn an_option_outside_the_question_is_illegal() {
    covers!("rules::crisis_pact::CrisisPact");
    let mut rig = Rig::new(table(), CrisisPact::new(Seat(1), StrategyCard::Trade, true));
    rig.run();
    rig.expect_illegal(Pick::Opt(opt(SOpt::PrimaryInstead)));
    rig.expect_illegal(Pick::Opt(Opt::Decline));
    smoke_quiet(|| Rig::new(table(), CrisisPact::new(Seat(1), StrategyCard::Trade, true)));
}
