//! D4-2a: `rules::strategy_secondary` (one step of the follower window of a strategic action) and the pure helpers on the held window.

use ti4_engine_core::choice::strategy::{Ask as SAsk, Opt as SOpt};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::rules::strategy_secondary::{advance, is_complete, plain_events, FollowerFacts, Outcome, Resolution, SecondaryStep, Waiver, WaiverEffect};
use ti4_engine_core::state::{Code, PlainKind, PlainRec, Secondary, SecondaryHow};
use ti4_engine_core_tests::prelude::*;

const CARD: StrategyCard = StrategyCard::Politics;

fn follower(who: u8, eligible: bool, costs_token: bool) -> FollowerFacts {
    FollowerFacts { who: Seat(who), eligible, costs_token, primary_instead: false, waivers: Vec::new() }
}

fn step(followers: &[FollowerFacts], total: usize, tokens: i16) -> Rig<SecondaryStep> {
    let sc = Scenario::lobby(3).tokens(1, 3, 3, tokens).tokens(2, 3, 3, tokens);
    Rig::new(sc, SecondaryStep::new(Seat(0), CARD, true, total, followers, Code(5)))
}

fn strat(o: SOpt) -> Opt {
    Opt::Strategy(o)
}

fn exhausted(rig: &Rig<SecondaryStep>) -> bool {
    rig.state.players().by_seat(Seat(0)).unwrap().strat_exhausted.contains(CARD)
}

#[test]
fn with_nobody_eligible_nothing_is_asked_everyone_is_skipped_and_the_card_is_spent() {
    covers!("rules::strategy_secondary::SecondaryStep");
    let mut rig = step(&[follower(1, false, true), follower(2, false, true)], 2, 2);
    rig.run();
    assert!(rig.is_done());
    let out = rig.driver.window.outcome();
    assert_eq!((out.skipped.as_slice(), out.answered, out.complete), (&[Seat(1), Seat(2)][..], None, true));
    assert!(exhausted(&rig));
}

#[test]
fn the_first_eligible_follower_is_asked_and_the_ones_before_are_recorded_as_ineligible() {
    covers!("rules::strategy_secondary::SecondaryStep");
    let mut rig = step(&[follower(1, false, true), follower(2, true, true)], 2, 2);
    rig.run();
    assert_eq!(rig.asked().actor, Seat(2));
    assert!(matches!(rig.asked().ask, Ask::Strategy(SAsk::Follow { card: CARD, primary: Seat(0), costs_token: true })));
    assert_eq!(rig.opts(), vec![strat(SOpt::Skip), strat(SOpt::Follow)]);
    rig.assert_options_sound();
    rig.pick_opt(strat(SOpt::Follow));
    assert!(rig.is_done());
    let out = rig.driver.window.outcome();
    assert_eq!(out.skipped.as_slice(), &[Seat(1)]);
    assert_eq!(out.answered, Some((Seat(2), Resolution::Followed)));
    assert!(out.complete, "the asked follower was the last of two");
    assert!(exhausted(&rig));
    assert_eq!(rig.state.players().by_seat(Seat(2)).unwrap().econ.strategic_tokens, 1, "a strategy token paid");
    ti4_engine_core_tests::assert_snapshot("strategy_secondary/follow", &rig.snapshot());
}

#[test]
fn declining_keeps_the_token_and_the_card_stays_while_followers_remain() {
    covers!("rules::strategy_secondary::SecondaryStep");
    let mut rig = step(&[follower(1, true, true), follower(2, true, true)], 2, 2);
    rig.run().pick_opt(strat(SOpt::Skip));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.outcome().answered, Some((Seat(1), Resolution::Declined)));
    assert!(!rig.driver.window.outcome().complete, "seat 2 is still to be asked");
    assert!(!exhausted(&rig));
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 2);
}

#[test]
fn a_follow_that_costs_a_token_the_follower_lacks_is_illegal() {
    covers!("rules::strategy_secondary::SecondaryStep");
    let mut rig = step(&[follower(1, true, true)], 1, 0);
    rig.run();
    rig.expect_illegal(Pick::Opt(strat(SOpt::Follow)));
    rig.pick_opt(strat(SOpt::Skip));
    assert!(rig.is_done());
}

#[test]
fn a_free_follow_costs_no_token() {
    covers!("rules::strategy_secondary::SecondaryStep");
    let mut rig = step(&[follower(1, true, false)], 1, 0);
    rig.run().pick_opt(strat(SOpt::Follow));
    assert!(rig.is_done());
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 0);
}

#[test]
fn a_tokenless_follower_with_a_waiver_is_offered_the_waiver_instead_of_the_follow() {
    covers!("rules::strategy_secondary::SecondaryStep");
    let mut f = follower(1, true, true);
    f.waivers.push(Waiver { opt: SOpt::Waive(Code(9)), effect: WaiverEffect::Free });
    let mut rig = step(&[f.clone()], 1, 0);
    rig.run();
    assert_eq!(rig.opts(), vec![strat(SOpt::Skip), strat(SOpt::Waive(Code(9)))]);
    rig.expect_illegal(Pick::Opt(strat(SOpt::Waive(Code(8)))));
    rig.expect_illegal(Pick::Opt(strat(SOpt::Follow)));
    rig.pick_opt(strat(SOpt::Waive(Code(9))));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.outcome().answered, Some((Seat(1), Resolution::Followed)));
    let mut rich = step(&[f], 1, 1);
    rich.run();
    assert_eq!(rich.opts(), vec![strat(SOpt::Skip), strat(SOpt::Follow), strat(SOpt::Waive(Code(9)))], "with a token the follow is offered too");
}

#[test]
fn an_unported_waiver_is_refused() {
    covers!("rules::strategy_secondary::SecondaryStep");
    let mut f = follower(1, true, true);
    f.waivers.push(Waiver { opt: SOpt::Waive(Code(9)), effect: WaiverEffect::Unported });
    let mut rig = step(&[f], 1, 0);
    rig.run();
    rig.expect_illegal(Pick::Opt(strat(SOpt::Waive(Code(9)))));
}

#[test]
fn the_primary_may_be_performed_instead_only_when_the_follow_costs_nothing() {
    covers!("rules::strategy_secondary::SecondaryStep");
    let mut f = follower(1, true, false);
    f.primary_instead = true;
    let mut rig = step(&[f.clone()], 1, 0);
    rig.run();
    assert_eq!(rig.opts(), vec![strat(SOpt::Skip), strat(SOpt::Follow), strat(SOpt::PrimaryInstead)]);
    rig.pick_opt(strat(SOpt::PrimaryInstead));
    assert!(rig.is_done());
    assert_eq!(rig.driver.window.outcome().answered, Some((Seat(1), Resolution::Followed)));
    let mut pays = follower(1, true, true);
    pays.primary_instead = true;
    let mut dear = step(&[pays], 1, 2);
    dear.run();
    assert!(!dear.opts().contains(&strat(SOpt::PrimaryInstead)));
}

#[test]
fn the_step_survives_every_answer_policy() {
    covers!("rules::strategy_secondary::SecondaryStep");
    smoke_quiet(|| step(&[follower(1, false, true), follower(2, true, true)], 3, 1));
    smoke_quiet(|| step(&[follower(1, true, false), follower(2, true, true)], 2, 0));
}

fn held(followers: &[u8]) -> Secondary {
    Secondary { primary: Seat(0), card: CARD, followers: followers.iter().map(|s| Seat(*s)).collect(), next: 0, resolved: SmallList::new(), exhausts_card: true, preview: false }
}

#[test]
fn the_held_window_advances_by_the_skipped_followers_and_the_answer() {
    covers!("rules::strategy_secondary::advance");
    let mut w = held(&[1, 2, 3]);
    assert!(!is_complete(&w));
    let mut skipped = SmallList::new();
    skipped.push(Seat(1));
    advance(&mut w, &Outcome { skipped, answered: Some((Seat(2), Resolution::Declined)), complete: false });
    assert_eq!(w.next, 2);
    assert_eq!(w.resolved.as_slice(), &[(Seat(1), SecondaryHow::Ineligible), (Seat(2), SecondaryHow::Declined)]);
    assert!(!is_complete(&w));
    advance(&mut w, &Outcome { skipped: SmallList::new(), answered: Some((Seat(3), Resolution::Followed)), complete: true });
    assert!(is_complete(&w));
    assert_eq!(w.resolved.as_slice().last(), Some(&(Seat(3), SecondaryHow::Followed)));
}

#[test]
fn the_plain_events_of_a_step_name_the_answer_and_the_end_of_the_action() {
    covers!("rules::strategy_secondary::plain_events");
    let out = |answered, complete| Outcome { skipped: SmallList::new(), answered, complete };
    assert!(plain_events(&out(None, false)).is_empty());
    assert_eq!(plain_events(&out(Some((Seat(1), Resolution::Declined)), false)).as_slice(), &[PlainRec::bare(PlainKind::StrategySecondaryDeclined)]);
    assert_eq!(
        plain_events(&out(Some((Seat(1), Resolution::Followed)), true)).as_slice(),
        &[PlainRec::bare(PlainKind::StrategySecondaryFollowed), PlainRec::bare(PlainKind::StrategicActionComplete)]
    );
    assert_eq!(plain_events(&out(None, true)).as_slice(), &[PlainRec::bare(PlainKind::StrategicActionComplete)]);
}
