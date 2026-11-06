//! Lane B, KP2: the end of a turn as a live job (`driver::turn::end`). Every test runs the job the way the live loop does: a held
//! window on the stack, `advance` to a question or the end, a clone of the position standing for a checkpoint.


use ti4_engine_core::choice::turn::{Ask as TAsk, Opt as TOpt};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{advance, ask, is_open, Env, Live, LiveError};
use ti4_engine_core::driver::turn::{advance_job, TurnFlowWin};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::state::{Emit, Outcome, PlainArg, PlainKind, PlainRec, TransientFlags, TurnJob};
use ti4_engine_core_tests::prelude::*;

fn env(k: &Kernel) -> Env<'_> {
    let mut table = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    Env { kernel: k, registry: Cow::new(Registry::default()), table }
}

fn plain_kinds(s: &CoreState) -> Vec<PlainKind> {
    s.ledger.events.iter().filter_map(|e| e.plain().map(|p| p.kind)).collect()
}

fn end_turn(s: &mut CoreState, k: &Kernel) -> Live {
    advance_job(TurnJob::End, s, &env(k), None).expect("the job runs").0
}

/// A three-seat table in the action phase, seat 0 active, nobody passed.
fn table() -> (CoreState, Kernel) {
    Scenario::lobby(3).phase(Phase::Action).active(Some(0)).build()
}

#[test]
fn a_plain_turn_passes_to_the_next_seat_and_announces_the_end_of_the_one_before() {
    let (mut s, k) = table();
    assert_eq!(end_turn(&mut s, &k), Live::Done);
    assert_eq!(s.meta().active, Some(Seat(1)));
    assert_eq!(s.meta().turn_seq, 1);
    assert!(!is_open::<TurnFlowWin>(&s), "the job left the stack");
    // The plain line, then the typed event the resolver emitted (mirrored into the queue).
    assert_eq!(plain_kinds(&s), [PlainKind::TurnPassed]);
    assert!(matches!(s.ledger.events.iter().nth(1), Some(Emit::Typed(e)) if e.kind == ti4_engine_core::state::EventKind::TurnPassed));
    assert_eq!(s.ledger.events.len(), 2);
}

#[test]
fn an_additional_action_keeps_the_turn_and_spends_the_flag() {
    let (mut s, k) = table();
    s.transient_mut().misc.flags.set(TransientFlags::ADDITIONAL_ACTION);
    s.pending_mut().actions_this_turn = 3;
    s.pending_mut().turn_closing = Some(Seat(0));
    assert_eq!(end_turn(&mut s, &k), Live::Done);
    assert_eq!((s.meta().active, s.meta().turn_seq), (Some(Seat(0)), 0), "the same turn goes on");
    assert!(!s.transient().misc.flags.has(TransientFlags::ADDITIONAL_ACTION));
    assert_eq!(plain_kinds(&s), [PlainKind::TurnRetained]);
    assert_eq!(s.pending().turn_closing, None, "the closing menu is over");
    assert_eq!(s.pending().actions_this_turn, 3, "a retained turn does not reset the count of its actions");
}

#[test]
fn the_last_seat_to_pass_still_ends_its_turn_with_nobody_to_pass_it_to() {
    let (mut s, k) = table();
    for i in 0..3 {
        s.players_mut().edit(i).econ.passed = true;
    }
    assert_eq!(end_turn(&mut s, &k), Live::Done);
    assert_eq!(s.meta().active, None);
    assert_eq!(plain_kinds(&s), [PlainKind::TurnPassed]);
}

#[test]
fn crisis_skips_the_seat_the_turn_arrived_at_and_a_puppet_grant_gives_the_passer_a_fresh_turn() {
    let (mut s, k) = table();
    s.transient_mut().misc.flags.set(TransientFlags::SKIP_NEXT_TURN);
    assert_eq!(end_turn(&mut s, &k), Live::Done);
    assert_eq!(s.meta().active, Some(Seat(2)));
    assert_eq!(plain_kinds(&s), [PlainKind::TurnPassed, PlainKind::TurnSkipped]);
    assert_eq!(s.ledger.events.iter().filter_map(Emit::plain).last().copied(), Some(PlainRec::one(PlainKind::TurnSkipped, PlainArg::Seat(Seat(1)))));
    let (mut s, k) = table();
    s.players_mut().edit(0).econ.passed = true;
    s.transient_mut().misc.flags.set(TransientFlags::PUPPET_ACTION);
    let seq = s.meta().turn_seq;
    assert_eq!(end_turn(&mut s, &k), Live::Done);
    // The turn goes to seat 1, then the grant gives the passer (seat 0, still passed) a FRESH turn: a new turn counter.
    assert_eq!((s.meta().active, s.meta().turn_seq), (Some(Seat(0)), seq + 2));
    assert!(s.players().by_seat(Seat(0)).unwrap().econ.passed, "the grant is one action, not a return from pass");
    assert_eq!(plain_kinds(&s), [PlainKind::TurnPassed, PlainKind::TurnPuppet]);
}

fn minister_table() -> (CoreState, Kernel) {
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).command_token(system, 0).tokens(0, 2, 3, 3).build();
    s.agenda_mut().laws.insert(Agenda::MinisterOfWar, Outcome::Seat(Seat(0)));
    (s, k)
}

fn pick(c: &Choice, i: usize) -> Pick {
    c.pick(i).expect("an option")
}

#[test]
fn the_minister_of_war_is_offered_the_discard_then_the_token_and_the_turn_is_retained() {
    let (mut s, k) = minister_table();
    let (system, _) = ti4_engine_core_tests::scenario::first_planet();
    let e = env(&k);
    let (live, _) = advance_job(TurnJob::End, &mut s, &e, None).unwrap();
    let Live::Asks(offer) = live else { panic!("the offer is a question") };
    assert_eq!((offer.choice.actor, matches!(offer.choice.ask, Ask::Agenda(_))), (Seat(0), true));
    assert!(offer.nested.is_none(), "a question of the job itself, not of an emission");
    assert!(s.pending().nested.is_some(), "the step is suspended on it");
    // A clone of the suspended position stands for a checkpoint; a dropped driver changes nothing.
    let mut fork = s.clone();
    assert_eq!(ask::<TurnFlowWin>(&fork, &e).unwrap().unwrap().choice, offer.choice, "a replay of the suspension asks the same question");
    let discard = pick(&offer.choice, 0);
    let Live::Asks(token) = ti4_engine_chance::resolved(advance::<TurnFlowWin>)(&mut s, &e, Some(discard)).unwrap().0 else { panic!("which token comes back") };
    assert_eq!(token.choice.ask, Ask::Turn(TAsk::MinisterToken));
    assert_eq!(token.choice.opts.as_slice(), &[Opt::Turn(TOpt::Token(system))]);
    assert_eq!(s.agenda().laws.get(&Agenda::MinisterOfWar), None.or(Some(&Outcome::Seat(Seat(0)))), "nothing is kept of a half-run job: the law is still on the table");
    let back = pick(&token.choice, 0);
    let (live, _) = ti4_engine_chance::resolved(advance::<TurnFlowWin>)(&mut s, &e, Some(back)).unwrap();
    assert_eq!(live, Live::Done);
    // Law discarded, the token back in the tactic pool, the turn retained and the flag spent.
    assert!(s.agenda().laws.get(&Agenda::MinisterOfWar).is_none());
    assert!(!s.board().sys(system).cmd.contains(Seat(0)));
    assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.tactic_tokens, 3);
    assert_eq!((s.meta().active, s.meta().turn_seq), (Some(Seat(0)), 0));
    assert_eq!(plain_kinds(&s), [PlainKind::TurnRetained]);
    assert!(!s.transient().misc.flags.has(TransientFlags::ADDITIONAL_ACTION));
    // The fork continues identically.
    ti4_engine_chance::resolved(advance::<TurnFlowWin>)(&mut fork, &e, Some(discard)).unwrap();
    ti4_engine_chance::resolved(advance::<TurnFlowWin>)(&mut fork, &e, Some(back)).unwrap();
    assert_eq!(fork, s, "a clone mid-window continues identically");
}

#[test]
fn declining_the_minister_of_war_passes_the_turn_on_and_the_law_stays() {
    let (mut s, k) = minister_table();
    let e = env(&k);
    let Live::Asks(offer) = advance_job(TurnJob::End, &mut s, &e, None).unwrap().0 else { panic!("the offer") };
    let (live, _) = ti4_engine_chance::resolved(advance::<TurnFlowWin>)(&mut s, &e, Some(Pick::Opt(Opt::Decline))).unwrap();
    assert_eq!(live, Live::Done);
    assert!(s.agenda().laws.get(&Agenda::MinisterOfWar).is_some());
    assert_eq!(s.meta().active, Some(Seat(1)));
    assert_eq!(offer.choice.opts.len(), 2);
}

#[test]
fn a_minister_of_war_with_no_token_on_the_board_is_not_asked() {
    let (mut s, k) = table();
    s.agenda_mut().laws.insert(Agenda::MinisterOfWar, Outcome::Seat(Seat(0)));
    assert_eq!(end_turn(&mut s, &k), Live::Done, "22.3: nothing to retrieve, nothing asked");
    assert_eq!(s.meta().active, Some(Seat(1)));
}

#[test]
fn an_illegal_answer_leaves_the_job_where_it_was() {
    let (mut s, k) = minister_table();
    let e = env(&k);
    advance_job(TurnJob::End, &mut s, &e, None).unwrap();
    let held = s.clone();
    assert_eq!(ti4_engine_chance::resolved(advance::<TurnFlowWin>)(&mut s, &e, Some(Pick::Number(9))), Err(LiveError::Illegal));
    assert_eq!(s, held);
}
