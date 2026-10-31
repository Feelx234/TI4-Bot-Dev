//! D4-2d: the transitions of a space combat between windows (`rules::combat::flow::Flow`) and the announcement of its end (`EndAnnounce`).
//! `Flow` is a function from (facts, step) to the next step: every arm of the string `settle_stages` it ports is one row below.

use ti4_engine_core::rules::combat::end_announce::{Codes, EndAnnounce, Facts as EndFacts};
use ti4_engine_core::rules::combat::flow::{Facts, Flow, Out, Pend, Phase, Step, MAX_ROUNDS};
use ti4_engine_core::state::{Code, EventKind, Key, List};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn sys() -> System {
    plain_systems()[0]
}

fn arena() -> Scenario {
    Scenario::new(&[Faction::TheEmbersOfMuaat, Faction::TheArborec]).map_from(&[sys(), plain_systems()[1]])
}

fn facts() -> Facts {
    Facts {
        attacker: Seat(0),
        defender: Seat(1),
        system: sys(),
        announce: true,
        over: false,
        asking_can_retreat: false,
        leaver_destinations: 1,
        front_alive: 2,
        offered_before: false,
        sustainers: false,
        pending_retreats: vec![],
        phase: Phase::Round,
        resuming: false,
    }
}

fn run(f: Facts, step: Step) -> (Out, Vec<EventKind>, CoreState) {
    let mut rig = Rig::new(arena(), Flow::new(f, step));
    rig.run();
    assert!(rig.is_done(), "a transition never asks");
    let out = rig.driver.window.out.clone().expect("a transition leaves its outcome");
    let kinds = rig.driver.emitted.iter().map(|e| e.kind).collect();
    (out, kinds, rig.state)
}

fn pend(player: u8, hits: u32) -> Pend {
    Pend { player: Seat(player), hits, producer: Seat(1 - player), non_fighters_only: false, producer_assigns: false }
}

// ---- Announcing ------------------------------------------------------------------------------

#[test]
fn announcing_the_defender_comes_first_then_the_attacker_then_the_roll() {
    covers!("rules::combat::flow::Flow");
    let at_defender = Step::Announcing { round: 1, asking: Seat(1), announced: vec![] };
    let (out, kinds, _) = run(facts(), at_defender.clone());
    assert_eq!(out.next, Step::Announcing { round: 1, asking: Seat(0), announced: vec![] }, "nobody to ask at the defender: the attacker next");
    assert!(!out.returned);
    assert_eq!(kinds, vec![EventKind::RetreatStepStarted], "the retreat step opens with the defender's turn to announce");
    // The defender has somewhere to go: the question is pending.
    let (out, _, _) = run(Facts { asking_can_retreat: true, ..facts() }, at_defender);
    assert!(out.returned);
    assert_eq!(out.next, Step::Announcing { round: 1, asking: Seat(1), announced: vec![] }, "the step stays while the question is open");
    // At the attacker, nothing to ask: on to the roll with the announced retreats pending.
    let (out, kinds, _) = run(facts(), Step::Announcing { round: 1, asking: Seat(0), announced: vec![Seat(1)] });
    assert_eq!(out.next, Step::Rolling { round: 1 });
    assert_eq!(out.pending_retreats, vec![Seat(1)], "the retreat announced is carried to the end of the round");
    assert!(kinds.is_empty(), "the step-start event is for the defender's first turn only");
}

#[test]
fn a_defender_who_already_announced_does_not_get_a_second_turn_and_a_finished_fight_ends() {
    covers!("rules::combat::flow::Flow");
    let (out, _, _) = run(facts(), Step::Announcing { round: 2, asking: Seat(1), announced: vec![Seat(1)] });
    assert_eq!(out.next, Step::Rolling { round: 2 });
    assert_eq!(out.pending_retreats, vec![Seat(1)]);
    let (out, kinds, _) = run(Facts { over: true, ..facts() }, Step::Announcing { round: 1, asking: Seat(1), announced: vec![] });
    assert_eq!(out.next, Step::Done);
    assert!(out.returned, "the fight is over: the settle stops");
    assert_eq!(kinds, vec![EventKind::RetreatStepStarted]);
    let (_, quiet, _) = run(Facts { announce: false, ..facts() }, Step::Announcing { round: 1, asking: Seat(1), announced: vec![] });
    assert!(quiet.is_empty(), "without the timing machinery nothing is announced");
}

// ---- Retreating ------------------------------------------------------------------------------

#[test]
fn the_end_of_a_round_announces_it_and_opens_the_next_unless_the_fight_is_over_or_at_the_limit() {
    covers!("rules::combat::flow::Flow");
    let (out, kinds, _) = run(facts(), Step::Retreating { round: 3, leaving: vec![] });
    assert_eq!(out.next, Step::Opening { round: 4 });
    assert_eq!(out.phase, Phase::Round);
    assert!(!out.returned);
    assert_eq!(kinds, vec![EventKind::SpaceCombatRoundEnded]);
    let (out, _, _) = run(Facts { over: true, ..facts() }, Step::Retreating { round: 3, leaving: vec![] });
    assert_eq!(out.next, Step::Done);
    let (out, _, _) = run(facts(), Step::Retreating { round: MAX_ROUNDS, leaving: vec![] });
    assert_eq!(out.next, Step::Done, "fifty rounds is the limit");
    let (out, _, _) = run(facts(), Step::Retreating { round: MAX_ROUNDS - 1, leaving: vec![] });
    assert_eq!(out.next, Step::Opening { round: MAX_ROUNDS });
}

#[test]
fn a_leaver_with_nowhere_to_go_is_skipped_and_one_with_choices_is_a_pending_question() {
    covers!("rules::combat::flow::Flow");
    let (out, kinds, _) = run(Facts { leaver_destinations: 0, ..facts() }, Step::Retreating { round: 1, leaving: vec![Seat(1), Seat(0)] });
    assert_eq!(out.next, Step::Retreating { round: 1, leaving: vec![Seat(0)] });
    assert!(!out.returned && kinds.is_empty());
    let (out, _, _) = run(facts(), Step::Retreating { round: 1, leaving: vec![Seat(1)] });
    assert!(out.returned);
    assert_eq!(out.next, Step::Retreating { round: 1, leaving: vec![Seat(1)] });
}

// ---- Queue ---------------------------------------------------------------------------------

#[test]
fn an_empty_queue_continues_by_the_phase_it_was_opened_in() {
    covers!("rules::combat::flow::Flow");
    let queue = |round| Step::Queue { assigning: false, round, queue: vec![] };
    // Hits of the round: on to the retreats announced, which are consumed.
    let (out, _, _) = run(Facts { pending_retreats: vec![Seat(1)], ..facts() }, queue(2));
    assert_eq!(out.next, Step::Retreating { round: 2, leaving: vec![Seat(1)] });
    assert!(out.pending_retreats.is_empty());
    // The fight is over after the round's hits: the round ends and the fight is done.
    let (out, kinds, _) = run(Facts { over: true, ..facts() }, queue(2));
    assert_eq!((out.next, out.returned), (Step::Done, true));
    assert_eq!(kinds, vec![EventKind::SpaceCombatRoundEnded]);
    let (out, _, _) = run(facts(), queue(MAX_ROUNDS));
    assert_eq!(out.next, Step::Done, "the round limit ends it too");
    // Hits at the start of the combat: the first round opens, marked as resuming.
    let (out, _, _) = run(Facts { phase: Phase::CombatStart, ..facts() }, queue(1));
    assert_eq!(out.next, Step::Opening { round: 1 });
    assert!(out.resuming);
    assert_eq!(out.phase, Phase::Round);
    let (out, kinds, _) = run(Facts { phase: Phase::CombatStart, over: true, ..facts() }, queue(1));
    assert_eq!((out.next, out.returned), (Step::Done, true));
    assert!(kinds.is_empty(), "no round was played");
    // Hits at the end of a round: the round ends and the next opens.
    let (out, kinds, _) = run(Facts { phase: Phase::RoundEnd, ..facts() }, queue(2));
    assert_eq!(out.next, Step::Opening { round: 3 });
    assert_eq!(kinds, vec![EventKind::SpaceCombatRoundEnded]);
}

#[test]
fn a_queue_entry_without_hits_or_without_ships_is_dropped() {
    covers!("rules::combat::flow::Flow");
    let q = vec![pend(0, 0), pend(1, 2)];
    let (out, _, _) = run(facts(), Step::Queue { assigning: true, round: 1, queue: q.clone() });
    assert_eq!(out.next, Step::Queue { assigning: false, round: 1, queue: vec![pend(1, 2)] });
    assert!(!out.returned);
    let (out, _, _) = run(Facts { front_alive: 0, ..facts() }, Step::Queue { assigning: false, round: 1, queue: vec![pend(1, 3)] });
    assert_eq!(out.next, Step::Queue { assigning: false, round: 1, queue: vec![] }, "the player has no ship left: the hits have no effect");
}

#[test]
fn the_hits_to_assign_event_is_made_once_per_round_for_the_front_player() {
    covers!("rules::combat::flow::Flow");
    let step = Step::Queue { assigning: false, round: 2, queue: vec![pend(1, 2)] };
    let (out, kinds, _) = run(facts(), step.clone());
    assert!(out.offered_now);
    assert_eq!(kinds, vec![EventKind::HitsToAssign]);
    assert_eq!(out.next, Step::Queue { assigning: true, round: 2, queue: vec![pend(1, 2)] }, "nothing can sustain: straight to the assignment");
    assert!(!out.returned);
    // Something can sustain: the decision is the sustain offer, pending.
    let (out, _, _) = run(Facts { sustainers: true, ..facts() }, step.clone());
    assert!(out.returned && out.offered_now);
    assert_eq!(out.next, step, "the entry stays at the front");
    // Already announced this round: no second event.
    let (out, kinds, _) = run(Facts { offered_before: true, ..facts() }, step.clone());
    assert!(!out.offered_now && kinds.is_empty());
    // Already assigning: the assignment is pending, no event.
    let (out, kinds, _) = run(facts(), Step::Queue { assigning: true, round: 2, queue: vec![pend(1, 2)] });
    assert!(out.returned && kinds.is_empty());
    // Without the timing machinery the flag is still set but nothing is emitted.
    let (out, kinds, _) = run(Facts { announce: false, ..facts() }, step);
    assert!(out.offered_now && kinds.is_empty());
}

#[test]
fn cancellations_granted_in_the_hits_window_are_spent_before_a_sustain_or_a_casualty() {
    covers!("rules::combat::flow::Flow");
    let (mut state, kernel) = arena().build();
    state.meta_mut().combat_round_seq = 3;
    state.players_mut().by_seat_mut(Seat(1)).unwrap().card_timers.cancel_hits_round = Some((3, 2));
    let step = Step::Queue { assigning: false, round: 1, queue: vec![pend(1, 3)] };
    let mut rig = Rig::from_parts(state.clone(), kernel.clone(), Flow::new(facts(), step.clone()));
    rig.run();
    let out = rig.driver.window.out.clone().unwrap();
    assert_eq!(out.next, Step::Queue { assigning: false, round: 1, queue: vec![pend(1, 1)] }, "two of three hits cancelled");
    assert!(!out.returned);
    assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().card_timers.cancel_hits_round, Some((3, 0)));
    // A grant of another round is not valid now.
    state.players_mut().by_seat_mut(Seat(1)).unwrap().card_timers.cancel_hits_round = Some((2, 5));
    let mut stale = Rig::from_parts(state, kernel, Flow::new(facts(), step));
    stale.run();
    assert_eq!(stale.driver.window.out.clone().unwrap().next, Step::Queue { assigning: true, round: 1, queue: vec![pend(1, 3)] });
    assert_eq!(stale.state.players().by_seat(Seat(1)).unwrap().card_timers.cancel_hits_round, Some((2, 5)));
}

#[test]
fn the_other_steps_are_decisions_or_belong_to_other_windows() {
    covers!("rules::combat::flow::Flow");
    for step in [Step::Opening { round: 1 }, Step::Rolling { round: 1 }, Step::AfterBarrage { round: 1 }, Step::Done] {
        let (out, kinds, _) = run(facts(), step.clone());
        assert!(out.returned && out.next == step && kinds.is_empty(), "{step:?}");
    }
}

// ---- EndAnnounce -----------------------------------------------------------------------------

fn end_facts(winner: Option<Seat>, declared_draw: bool, announce: bool) -> EndFacts {
    EndFacts {
        attacker: Seat(0),
        defender: Seat(1),
        system: sys(),
        rounds: 3,
        winner,
        declared_draw,
        fled: vec![],
        at_start: vec![(Seat(0), vec![Unit::CruiserI]), (Seat(1), vec![Unit::DestroyerI, Unit::FighterI])],
        codes: Codes { losers: Code(1), retreated: Code(2), reasons: [Code(10), Code(11), Code(12), Code(13)] },
        announce,
    }
}

#[test]
fn the_end_of_a_combat_reports_winner_losers_reason_and_the_ships_lost() {
    covers!("rules::combat::end_announce::EndAnnounce");
    let mut rig = Rig::new(arena().unit(sys(), None, Unit::CruiserI, 0, 1), EndAnnounce::new(end_facts(Some(Seat(0)), false, true)));
    rig.run();
    assert!(rig.is_done());
    let e = &rig.driver.emitted;
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].kind, EventKind::SpaceCombatEnded);
    let p = &e[0].payload;
    assert_eq!(p.system(Key::System), Some(sys()));
    assert_eq!((p.seat(Key::Attacker), p.seat(Key::Defender), p.seat(Key::Winner)), (Some(Seat(0)), Some(Seat(1)), Some(Seat(0))));
    assert_eq!(p.int(Key::Rounds), Some(3));
    assert_eq!(p.code(Key::Reason), Some(Code(12)), "destruction");
    let Some(List::Seats(losers)) = p.list(Key::Other(Code(1))) else { panic!("losers list") };
    assert_eq!(losers.as_slice(), &[Seat(1)]);
    let Some(List::Seats(left)) = p.list(Key::Other(Code(2))) else { panic!("retreated list") };
    assert!(left.is_empty());
    let Some(List::Units(lost)) = p.list(Key::Destroyed) else { panic!("destroyed list") };
    assert_eq!(lost.as_slice(), &[(Seat(1), Unit::DestroyerI), (Seat(1), Unit::FighterI)]);
}

#[test]
fn a_draw_has_no_winner_key_and_each_reason_has_its_own_text() {
    covers!("rules::combat::end_announce::EndAnnounce");
    let mut draw = Rig::new(arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::DestroyerI, 1, 1), EndAnnounce::new(end_facts(None, true, true)));
    draw.run();
    let p = &draw.driver.emitted[0].payload;
    assert_eq!(p.seat(Key::Winner), None);
    assert_eq!(p.code(Key::Reason), Some(Code(11)), "draw");
    let mut limit = Rig::new(arena().unit(sys(), None, Unit::CruiserI, 0, 1).unit(sys(), None, Unit::DestroyerI, 1, 1), EndAnnounce::new(end_facts(None, false, true)));
    limit.run();
    assert_eq!(limit.driver.emitted[0].payload.code(Key::Reason), Some(Code(13)), "round limit");
    let mut facts = end_facts(Some(Seat(0)), false, true);
    facts.fled = vec![(Seat(1), vec![Unit::DestroyerI])];
    let mut retreat = Rig::new(arena().unit(sys(), None, Unit::CruiserI, 0, 1), EndAnnounce::new(facts));
    retreat.run();
    let p = &retreat.driver.emitted[0].payload;
    assert_eq!(p.code(Key::Reason), Some(Code(10)), "retreat");
    let Some(List::Seats(left)) = p.list(Key::Other(Code(2))) else { panic!("retreated list") };
    assert_eq!(left.as_slice(), &[Seat(1)]);
    let Some(List::Units(lost)) = p.list(Key::Destroyed) else { panic!("destroyed list") };
    assert_eq!(lost.as_slice(), &[(Seat(1), Unit::FighterI)], "the destroyer arrived with the retreat");
}

#[test]
fn without_the_timing_machinery_nothing_is_announced() {
    covers!("rules::combat::end_announce::EndAnnounce");
    let mut rig = Rig::new(arena().unit(sys(), None, Unit::CruiserI, 0, 1), EndAnnounce::new(end_facts(Some(Seat(0)), false, false)));
    rig.run();
    assert!(rig.is_done() && rig.driver.emitted.is_empty());
    assert_eq!(delta(rig.start_state(), &rig.state), "");
}

#[test]
fn hits_a_card_cancelled_earlier_in_the_round_are_spent_first_without_the_timing_machinery_too() {
    covers!("rules::combat::flow::Flow");
    for announce in [true, false] {
        let (mut state, kernel) = arena().build();
        state.meta_mut().combat_round_seq = 3;
        state.players_mut().by_seat_mut(Seat(1)).unwrap().card_timers.cancel_hits_round = Some((3, 2));
        let queue = vec![pend(1, 3), pend(0, 1)];
        for assigning in [false, true] {
            let step = Step::Queue { assigning, round: 1, queue: queue.clone() };
            let mut rig = Rig::from_parts(state.clone(), kernel.clone(), Flow::new(Facts { announce, ..facts() }, step));
            rig.run();
            let out = rig.driver.window.out.clone().unwrap();
            assert_eq!(out.next, Step::Queue { assigning, round: 1, queue: vec![pend(1, 1), pend(0, 1)] }, "announce {announce} assigning {assigning}: two of the three hits go, the stage stays");
            assert!(!out.returned && !out.offered_now, "the announcement of the hits waits for the next transition");
            assert!(rig.driver.emitted.is_empty(), "nothing is announced before the pool is spent");
            assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().card_timers.cancel_hits_round, Some((3, 0)));
        }
        // More cancelled than hits: the queue head empties and the rest of the pool stays.
        state.players_mut().by_seat_mut(Seat(1)).unwrap().card_timers.cancel_hits_round = Some((3, 5));
        let step = Step::Queue { assigning: false, round: 1, queue: vec![pend(1, 2)] };
        let mut rig = Rig::from_parts(state, kernel, Flow::new(Facts { announce, ..facts() }, step));
        rig.run();
        assert_eq!(rig.driver.window.out.clone().unwrap().next, Step::Queue { assigning: false, round: 1, queue: vec![pend(1, 0)] });
        assert_eq!(rig.state.players().by_seat(Seat(1)).unwrap().card_timers.cancel_hits_round, Some((3, 3)));
    }
}
