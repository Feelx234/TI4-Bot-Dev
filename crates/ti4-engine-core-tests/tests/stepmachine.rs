//! stepmachine-1 (`docs/STEPMACHINE1_RESUME.md`): the strategy card's ability (`driver::windows::strategic::perform`) parks its child loop at
//! a question, and an answer in the same position resumes the operation there instead of replaying it from its start. The conversion must be
//! exact: these tests fork the position at EVERY question of a converted path (a follower's secondary, a primary) and continue the original, a
//! copy and a third copy answered by the replay (`driver::set_parking(false)`) with the same answers: the three must reach the same position
//! and the same outputs at every step. A copy answered differently must leave the original untouched and equal its own replay. A pick that is
//! no option is refused with the position untouched, and the park stays usable. (Debug builds also check every parked answer against the
//! replay inside the core, `live::verify_parked`.)

use std::rc::Rc;

use ti4_engine_core::choice::strategy::Opt as SOpt;
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{advance, Env, Live, LiveError, Out};
use ti4_engine_core::driver::turn::TurnFlowWin;
use ti4_engine_core::driver::windows::strategic::{advance_secondary, begin::begin_strategic, secondary_of};
use ti4_engine_core::driver::{park_stats, set_parking, NoSeeds};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::state::Window as OpenWindow;
use ti4_engine_core_tests::prelude::*;

fn env(k: &Kernel) -> Env<'_> {
    use ti4_engine_core::kernel::ability::{PlayFacts, PlaysProvider, SharedPlay};
    let roster: &'static ti4_engine_core::query::tech::UnitRoster = Box::leak(Box::default());
    let mut table = AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    table.facts.plays_provider = Some(PlaysProvider(std::sync::Arc::new(move |core, _| {
        Some(PlayFacts {
            scope: 0,
            blanked: false,
            action_card: ti4_engine_core::state::Code(0),
            supply_base: ti4_engine_core::state::Code(0),
            note_order: |a, b| a.cmp(&b),
            seats: Vec::new(),
            name_group: Vec::new(),
            has_effect: Vec::new(),
            skimmer: false,
            roster: Some(roster),
            rules_version: core.meta().rules_version,
            tech_order: |a, b| a.cmp(&b),
            shared: SharedPlay::default(),
        })
    })));
    Env { kernel: k, registry: Cow::new(Registry::default()), table, seeds: Rc::new(NoSeeds) }
}

/// Which arm answers: the follower window, or the strategic job (the primary).
#[derive(Clone, Copy)]
enum Arm {
    Secondary,
    Job,
}

type Step = Result<(Live, Out), LiveError>;

fn answer(arm: Arm, s: &mut CoreState, e: &Env<'_>, pick: Pick) -> Step {
    match arm {
        Arm::Secondary => advance_secondary(s, e, Some(pick)),
        Arm::Job => advance::<TurnFlowWin>(s, e, Some(pick)),
    }
}

/// The same answer by the replay (the park switched off for this call; the suspension it leaves keeps no park).
fn replayed(arm: Arm, s: &mut CoreState, e: &Env<'_>, pick: Pick) -> Step {
    set_parking(false);
    let r = answer(arm, s, e, pick);
    set_parking(true);
    r
}

/// The `n`-th answer of the policy: the option `n` modulo the options (so that the questions are answered differently along the way).
fn policy(c: &Choice, n: usize) -> Pick {
    c.pick(n % c.opts.len().max(1)).expect("an option")
}

/// Walk the nested questions of one operation from `live` on. At every question: fork; the original and a fork answer with the policy, a
/// third copy answers by the replay; the three must agree in everything. A fourth copy takes another option and must equal ITS replay, and
/// the original must not see it. Returns the number of questions walked and the number of parked answers.
fn walk(arm: Arm, s: &mut CoreState, e: &Env<'_>, mut live: Live) -> (usize, u64) {
    let parked_before = park_stats().1;
    let mut n = 0;
    while let Live::Asks(asking) = live {
        if s.pending().nested.is_none() {
            break; // a stage question of the window: the operation is over
        }
        let pick = policy(&asking.choice, n);
        let mut fork = s.clone();
        let mut replay = s.clone();
        let held = s.clone();
        // Another option on a copy of its own: the copy and its replay agree, the original is untouched.
        if asking.choice.opts.len() > 1 {
            let other = policy(&asking.choice, n + 1);
            let (mut alt, mut alt_replay) = (s.clone(), s.clone());
            let a = answer(arm, &mut alt, e, other);
            let b = replayed(arm, &mut alt_replay, e, other);
            assert_eq!(a, b, "question {n}: another answer on a copy differs from its replay");
            assert_eq!(alt, alt_replay, "question {n}: another answer on a copy left another position than its replay");
            assert_eq!(*s, held, "question {n}: the copy changed the original");
        }
        let a = answer(arm, s, e, pick);
        let b = answer(arm, &mut fork, e, pick);
        let c = replayed(arm, &mut replay, e, pick);
        assert_eq!(a, b, "question {n}: the fork's answer differs");
        assert_eq!(a, c, "question {n}: the parked answer differs from the replay's");
        assert_eq!(*s, fork, "question {n}: the fork reached another position");
        assert_eq!(*s, replay, "question {n}: the parked answer reached another position than the replay");
        live = a.expect("the answer is taken").0;
        n += 1;
    }
    (n, park_stats().1 - parked_before)
}

/// Leadership's follower: the purchase and its payment are asked from inside the follower's answer.
fn leadership_followers() -> (CoreState, Kernel) {
    let (mut s, k) = Scenario::lobby(3).phase(Phase::Action).active(Some(0)).goods(1, 6).goods(2, 6).build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().strat.push(StrategyCard::Leadership);
    s.pending_mut().open(OpenWindow::StrategySecondary(secondary_of(Seat(0), StrategyCard::Leadership, &[Seat(1), Seat(2)], true)));
    (s, k)
}

#[test]
fn a_followers_ability_forked_at_every_question_continues_identically_and_equals_the_replay() {
    let (mut s, k) = leadership_followers();
    let e = env(&k);
    let mut walked = (0, 0);
    // Each follower follows; every nested question of its ability is forked.
    for _ in 0..2 {
        let (live, _) = advance_secondary(&mut s, &e, Some(Pick::Opt(Opt::Strategy(SOpt::Follow)))).unwrap();
        let w = walk(Arm::Secondary, &mut s, &e, live);
        walked = (walked.0 + w.0, walked.1 + w.1);
        if !matches!(s.pending().window(ti4_engine_core::state::WindowKind::StrategySecondary), Some(_)) {
            break;
        }
    }
    eprintln!("secondary: {walked:?} (questions, parked answers)");
    assert!(walked.0 >= 2, "the abilities asked from inside the answers: {walked:?}");
    assert!(walked.1 >= 1, "answers resumed a parked operation: {walked:?}");
}

#[test]
fn a_primary_forked_at_every_question_continues_identically_and_equals_the_replay() {
    for card in [StrategyCard::Leadership, StrategyCard::Trade, StrategyCard::Politics] {
        let (mut s, k) = Scenario::lobby(4).phase(Phase::Action).active(Some(0)).goods(0, 6).build();
        s.players_mut().by_seat_mut(Seat(0)).unwrap().strat.push(card);
        let e = env(&k);
        let (live, _) = begin_strategic(&mut s, &e, card).unwrap();
        let (n, parked) = walk(Arm::Job, &mut s, &e, live);
        eprintln!("{card:?} primary: {n} questions, {parked} parked answers");
        assert!(n >= 1, "{card:?}: the primary asks from inside the step");
        assert!(parked >= 1 || n == 1, "{card:?}: a later question resumed a parked operation ({n} questions, {parked} parked)");
    }
}

#[test]
fn an_illegal_pick_to_a_parked_question_is_refused_and_the_park_stays_usable() {
    let (mut s, k) = leadership_followers();
    let e = env(&k);
    let (mut live, _) = advance_secondary(&mut s, &e, Some(Pick::Opt(Opt::Strategy(SOpt::Follow)))).unwrap();
    // Go to a question the follower's ability parked at (the first one comes from the answer's own run, which parks too).
    let Live::Asks(asking) = &live else { panic!("the ability asks") };
    assert!(s.pending().nested.is_some());
    let held = s.clone();
    for wrong in [Pick::Number(99), Pick::Opt(Opt::Strategy(SOpt::PrimaryInstead)), Pick::Opt(Opt::Done)] {
        if asking.choice.opts.iter().any(|o| Pick::Opt(*o) == wrong) {
            continue;
        }
        assert_eq!(advance_secondary(&mut s, &e, Some(wrong)), Err(LiveError::Illegal), "{wrong:?} is no option");
        assert_eq!(s, held, "the position is untouched");
    }
    let before = park_stats().1;
    let pick = asking.choice.pick(0).unwrap();
    live = advance_secondary(&mut s, &e, Some(pick)).unwrap().0;
    assert_eq!(park_stats().1, before + 1, "the legal pick resumed the parked operation");
    let mut replay = held.clone();
    let r = replayed(Arm::Secondary, &mut replay, &e, pick).unwrap().0;
    assert_eq!(live, r);
    assert_eq!(s, replay);
}

#[test]
fn a_park_is_shared_by_a_clone_and_each_copy_resumes_on_its_own() {
    let (mut s, k) = leadership_followers();
    let e = env(&k);
    let (live, _) = advance_secondary(&mut s, &e, Some(Pick::Opt(Opt::Strategy(SOpt::Follow)))).unwrap();
    let Live::Asks(asking) = live else { panic!("the ability asks") };
    let copies: Vec<CoreState> = (0..asking.choice.opts.len().min(3)).map(|_| s.clone()).collect();
    let before = park_stats().1;
    for (i, mut copy) in copies.into_iter().enumerate() {
        let pick = asking.choice.pick(i).unwrap();
        let a = advance_secondary(&mut copy, &e, Some(pick));
        let mut replay = s.clone();
        let b = replayed(Arm::Secondary, &mut replay, &e, pick);
        assert_eq!(a, b, "copy {i}");
        assert_eq!(copy, replay, "copy {i}");
    }
    assert!(park_stats().1 > before, "the copies resumed the shared park");
}

/// The inline sizes of a park and its parts (printed with `--nocapture`; the heap a park holds is measured by `perf_kernel` with `perf-count`).
#[test]
fn the_size_of_a_park() {
    use std::mem::size_of;
    use ti4_engine_core::driver::{Park, Resume};
    eprintln!(
        "Park {} bytes: CoreState {}, Resolver {}, Resume {}, Memo {}",
        size_of::<Park>(),
        size_of::<CoreState>(),
        size_of::<ti4_engine_core::kernel::Resolver>(),
        size_of::<Resume>(),
        size_of::<ti4_engine_core::state::Memo>()
    );
    assert!(size_of::<ti4_engine_core::state::Memo>() <= 16, "a position carries a park as one reference count");
}
