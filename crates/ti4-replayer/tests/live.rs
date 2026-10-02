//! Engine-driven tests for one live branch: a real game on one thread, parked at real decisions.
//!
//! These run the reviewer's own `LiveReview` through [`LiveBranch`], so a pause is the engine paused
//! inside `Table::ask` and an answer is validated by `Table::settle`. The inputs are the committed
//! example bundle and map pool, so there is no skip path.

// Every test here drives a live branch, which only the `host` build has.
#![cfg(feature = "host")]

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ti4_model::id::PlayerId;
use ti4_replayer::live::{AdvanceGoal, LiveBranch, LiveError, LiveEvent, LiveState};
use ti4_replayer::{
    ManualSubmission, ModeEffect, PendingManualChoice, Provenance, SeatControl, SeatMode,
    SubmitOutcome,
};
use ti4_review::{ProfileTable, SimulationConfig};

const CHECKPOINT: &str = "examples/reviewer/checkpoint-473312/slots.json";
const MAP_POOL: &str = "examples/reviewer/full_np8_12_holdout.json";
const SEED: u64 = 4_242;
const ROTATION: usize = 1;
const TEMPERATURE: f64 = 0.5;
const ALL_SEATS: [&str; 6] = ["seat0", "seat1", "seat2", "seat3", "seat4", "seat5"];
/// A parked branch is waiting for the test, not for the CPU, so this is generous -- but it is
/// not infinite, because a pause that never happened should fail in minutes, not hours.
const WAIT: Duration = Duration::from_secs(120);

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits directly under the workspace root")
        .to_path_buf()
}

fn env_path(name: &str, default: &str) -> PathBuf {
    env::var_os(name).map_or_else(|| workspace_root().join(default), PathBuf::from)
}

fn config() -> SimulationConfig {
    SimulationConfig {
        checkpoint: env_path("TI4_REPLAYER_CHECKPOINT", CHECKPOINT),
        map_pool: env_path("TI4_REPLAYER_POOL", MAP_POOL),
        seed: SEED,
        rotation: ROTATION,
        table: ProfileTable::Learner,
        temperature: TEMPERATURE,
        diplomacy: false,
    }
}

fn seats_with_manual(names: &[&str]) -> SeatControl {
    let mut seats = SeatControl::all_auto();
    for name in names {
        seats.set_mode(&PlayerId::new(*name), SeatMode::Manual);
    }
    seats
}

/// Start one live branch with the named seats on Manual and the rest on Auto.
fn start(manual: &[&str]) -> LiveBranch {
    let config = config();
    for path in [&config.checkpoint, &config.map_pool] {
        assert!(
            path.is_file(),
            "live-branch input {} is missing from the checkout",
            path.display()
        );
    }
    let branch = LiveBranch::start(config, seats_with_manual(manual)).expect("spawn the branch");
    // The thread builds the game before it will accept an advance; wait for that rather than
    // racing it, because a setup failure must surface as a Failed state, not a lost command.
    let deadline = Instant::now() + WAIT;
    loop {
        match branch.gate().state() {
            LiveState::Ready => break,
            LiveState::Failed => {
                let reason = branch
                    .drain_events()
                    .into_iter()
                    .find_map(|event| match event {
                        LiveEvent::Failed(reason) => Some(reason),
                        _ => None,
                    })
                    .unwrap_or_else(|| "no reason reported".to_owned());
                panic!("the branch failed to start: {reason}");
            }
            _ if Instant::now() >= deadline => panic!("the branch never became ready"),
            _ => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    branch.drain_events();
    branch
}

/// Everything one driven advance showed us.
#[derive(Debug, Default)]
struct Driven {
    /// Engine frames the branch reported.
    frames: usize,
    /// Every offer the branch parked on, in order.
    parks: Vec<PendingManualChoice>,
}

/// Run a goal, answering every manual offer with its first option until the goal comes to rest.
fn drive(branch: &LiveBranch, goal: AdvanceGoal) -> Driven {
    let mut out = Driven::default();
    branch.gate().run(goal).expect("an advance is accepted");
    let deadline = Instant::now() + WAIT;
    loop {
        out.frames += branch
            .drain_events()
            .into_iter()
            .filter(|event| matches!(event, LiveEvent::Frame(_)))
            .count();
        if let Some(pending) = branch.gate().pending() {
            let _ = branch.drain_events();
            let option_id = pending
                .options
                .first()
                .expect("a panel always offers something")
                .id
                .clone();
            let outcome = branch.gate().submit(&ManualSubmission {
                offer: pending.offer,
                fingerprint: pending.fingerprint.clone(),
                option_id: option_id.clone(),
            });
            assert_eq!(
                outcome,
                SubmitOutcome::Accepted { option_id },
                "clicking an option on the offer currently on screen must be accepted"
            );
            out.parks.push(pending);
            continue;
        }
        if branch.wait_until_idle(Duration::from_millis(5)) {
            // Drain once more: a frame published just before the goal came to rest would otherwise
            // be counted as missing, which is a test artefact and not a lost step.
            out.frames += branch
                .drain_events()
                .into_iter()
                .filter(|event| matches!(event, LiveEvent::Frame(_)))
                .count();
            return out;
        }
        assert!(
            Instant::now() < deadline,
            "the branch never came to rest; state {:?}",
            branch.gate().state()
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Wait for the branch to park on a manual offer.
fn park(branch: &LiveBranch) -> PendingManualChoice {
    branch
        .wait_until_parked(WAIT)
        .expect("the branch parks on a manual decision")
}

/// Wait for the log entry recording how this seat answered this exact offer.
///
/// The entry is written by the simulation thread after the panel closes, so reading it immediately
/// would race in both directions: too early and it is missing, too late and the seat has answered
/// something else. Matching on the fingerprint waits for the fact rather than for a sleep.
fn answer_to(
    branch: &LiveBranch,
    actor: &PlayerId,
    fingerprint: &ti4_replayer::ChoiceFingerprint,
) -> ti4_replayer::AnsweredDecision {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(entry) = branch
            .gate()
            .log()
            .entries()
            .into_iter()
            .find(|entry| &entry.actor == actor && &entry.fingerprint == fingerprint)
        {
            return entry;
        }
        assert!(
            Instant::now() < deadline,
            "seat {actor} never answered the choice {fingerprint:?}"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_manual_seat_parks_before_any_mutation_and_play_resumes_when_answered() {
    let branch = start(&ALL_SEATS);
    branch
        .gate()
        .run(AdvanceGoal::Steps(1))
        .expect("one step is accepted");
    let pending = park(&branch);
    assert!(
        ALL_SEATS.contains(&pending.actor.as_str()),
        "the panel names a physical seat, got {}",
        pending.actor
    );
    assert!(!pending.options.is_empty());
    assert_eq!(branch.gate().state(), LiveState::WaitingForHuman);

    // Parked means parked: nothing advances and the offer does not drift.
    let frames_before = branch
        .drain_events()
        .into_iter()
        .filter(|event| matches!(event, LiveEvent::Frame(_)))
        .count();
    std::thread::sleep(Duration::from_millis(120));
    let after = branch.gate().pending().expect("still parked");
    assert_eq!(
        after.fingerprint, pending.fingerprint,
        "the panel must not drift"
    );
    assert_eq!(
        branch
            .drain_events()
            .into_iter()
            .filter(|event| matches!(event, LiveEvent::Frame(_)))
            .count(),
        frames_before,
        "a parked branch must not mutate state"
    );
    // Reading the gate must not answer it.
    let _ = branch.gate().snapshot();
    let _ = branch.gate().pending();
    assert!(branch.gate().pending().is_some());

    let answer = pending
        .options
        .first()
        .expect("a panel offers something")
        .id
        .clone();
    assert_eq!(
        branch.gate().submit(&ManualSubmission {
            offer: pending.offer,
            fingerprint: pending.fingerprint.clone(),
            option_id: answer.clone(),
        }),
        SubmitOutcome::Accepted { option_id: answer },
        "the offered option for the choice on screen is accepted"
    );
    assert!(branch.gate().pending().is_none(), "the panel closes");
    assert!(branch.wait_until_idle(WAIT), "the engine resumes");
    assert!(
        branch
            .gate()
            .log()
            .provenance()
            .contains(&Provenance::Human),
        "the decision is recorded as one a person made"
    );
}

#[test]
fn nested_asks_inside_one_engine_step_each_get_their_own_panel() {
    // The measured reference session settles more than one decision on 6.1% of steps, so a probe at
    // the step boundary could never pause for the second one. With every seat manual, every ask in
    // the same step must still stop the engine and ask separately.
    let branch = start(&ALL_SEATS);
    let driven = drive(&branch, AdvanceGoal::Steps(120));
    let mut by_frame: BTreeMap<u64, Vec<&PendingManualChoice>> = BTreeMap::new();
    for pending in &driven.parks {
        by_frame.entry(pending.frame).or_default().push(pending);
    }
    let nested = by_frame
        .iter()
        .filter(|(_, asks)| asks.len() > 1)
        .max_by_key(|(_, asks)| asks.len())
        .expect("no engine step raised more than one manual ask in 120 steps");
    let (frame, asks) = (*nested.0, nested.1);
    assert!(
        asks.len() > 1,
        "frame {frame} should carry several asks, got {}",
        asks.len()
    );
    for (ordinal, pending) in asks.iter().enumerate() {
        assert_eq!(
            u64::from(pending.ask),
            ordinal as u64,
            "asks within frame {frame} are numbered from 0 in the order they were raised"
        );
    }
    // What must hold is that each ask got its *own* panel, and the ask ordinal is what says so: the
    // gate's clock increments once per ask within a frame, so a gate that republished one offer
    // several times instead of parking again would repeat an ordinal. That is checked above and
    // again here as a set.
    //
    // This deliberately does *not* assert that the fingerprints differ, which it used to. A
    // fingerprint binds actor, prompt, ordered options and context and excludes the frame and
    // ordinal on purpose, because that is what lets a rebuild match a recorded answer to the offer
    // it was made against. The engine legitimately raises asks that are identical in all of those
    // fields: a seat six units over capacity is asked "remove a unit: over capacity in 14" six times
    // in one step, every time with the single option `remove|0` and no context, so seven asks
    // collapsed to two distinct fingerprints and the old assertion failed on correct behaviour.
    // The real defect that hid behind it - the app suppressing every ask after the first, leaving a
    // parked engine with no panel on screen - is fixed in `ReplayApp` and pinned by
    // `app.rs::an_identical_looking_ask_at_the_next_ordinal_is_a_new_question`.
    let ordinals: std::collections::BTreeSet<u32> =
        asks.iter().map(|pending| pending.ask).collect();
    assert_eq!(
        ordinals.len(),
        asks.len(),
        "each ask in frame {frame} got its own panel, rather than one panel being republished"
    );
    assert!(
        asks.iter().all(|pending| pending.frame == frame),
        "every ask grouped under frame {frame} reports that frame"
    );
    assert!(driven.frames > 1, "the branch advanced through them");

    // R02-002's one uncovered path: an ask delivered *with* a bound seat observation. No test outside
    // the engine can build one (`SeatObservation::bind` is `pub(crate)`), so the proof is that a real
    // game delivered some, and that every one of them was still answered through the gate.
    let (viewless, bound) = branch.gate().delivery();
    assert!(
        viewless + bound >= driven.parks.len(),
        "every parked ask came from one of the two entry points: {} deliveries for {} parks",
        viewless + bound,
        driven.parks.len()
    );
    assert!(
        bound > 0,
        "no ask in {} engine steps was delivered with a bound observation (viewless {viewless})",
        driven.frames
    );
}

#[test]
fn a_stale_or_invented_answer_is_refused_and_the_panel_stays_up() {
    let branch = start(&ALL_SEATS);
    branch
        .gate()
        .run(AdvanceGoal::Steps(50))
        .expect("steps are accepted");
    let first = park(&branch);
    assert!(branch.gate().snapshot().pending.is_some());
    branch.gate().submit(&ManualSubmission {
        offer: first.offer,
        fingerprint: first.fingerprint.clone(),
        option_id: first.options.first().expect("an option").id.clone(),
    });
    let second = park(&branch);
    assert_ne!(
        second.fingerprint, first.fingerprint,
        "the second ask is a different choice"
    );

    // A click aimed at the panel that was already answered.
    assert_eq!(
        branch.gate().submit(&ManualSubmission {
            offer: first.offer,
            fingerprint: first.fingerprint.clone(),
            option_id: first.options.first().expect("an option").id.clone(),
        }),
        SubmitOutcome::Stale {
            current: second.fingerprint.clone()
        },
        "an old click is refused, and says what is current so the panel can redraw"
    );
    // An invented option on the current panel.
    let refused = branch.gate().submit(&ManualSubmission {
        offer: second.offer,
        fingerprint: second.fingerprint.clone(),
        option_id: "an-option-nobody-offered".to_owned(),
    });
    match refused {
        SubmitOutcome::NotOffered { offered } => assert_eq!(
            offered,
            second
                .ids()
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>(),
            "an invented option is refused with what was actually offered"
        ),
        other => panic!("an invented option must be refused as not offered, got {other:?}"),
    }
    assert_eq!(
        branch.gate().pending().expect("still parked").fingerprint,
        second.fingerprint,
        "a refusal must not consume the pending choice"
    );

    let answer = second.options.first().expect("an option").id.clone();
    assert_eq!(
        branch.gate().submit(&ManualSubmission {
            offer: second.offer,
            fingerprint: second.fingerprint.clone(),
            option_id: answer.clone(),
        }),
        SubmitOutcome::Accepted { option_id: answer },
        "after the refusals the same panel still accepts its real answer"
    );
    // The engine took that answer and moved on to another decision. Every seat is manual, so it
    // parks again rather than running to rest, and the third panel is the proof it resumed.
    let third = park(&branch);
    assert_ne!(third.fingerprint, second.fingerprint, "a later decision");
    let logged = branch.gate().log().entries();
    assert!(
        logged.len() >= 2
            && logged
                .iter()
                .any(|entry| entry.fingerprint == second.fingerprint
                    && entry.provenance == Provenance::Human),
        "both answered decisions are logged as a person's"
    );
}

#[test]
fn switching_the_waiting_seat_to_auto_lets_its_bot_answer_it() {
    let branch = start(&ALL_SEATS);
    branch
        .gate()
        .run(AdvanceGoal::Steps(3))
        .expect("steps are accepted");
    let pending = park(&branch);
    let actor = pending.actor.clone();
    let other = ALL_SEATS
        .iter()
        .map(|seat| PlayerId::new(*seat))
        .find(|seat| seat != &actor)
        .expect("six seats");

    // Changing a different seat must leave this panel alone.
    assert_eq!(
        branch.gate().set_mode(&other, SeatMode::Auto),
        ModeEffect::Changed {
            previous: SeatMode::Manual
        },
        "touching another seat changes only that seat"
    );
    assert_eq!(
        branch
            .gate()
            .pending()
            .expect("the panel is untouched")
            .fingerprint,
        pending.fingerprint,
        "changing another seat must not disturb a pending choice"
    );

    // Turning the waiting seat itself over to Auto releases its own panel to the bot.
    let effect = branch.gate().set_mode(&actor, SeatMode::Auto);
    assert_eq!(
        effect,
        ModeEffect::ReleasedBot {
            previous: SeatMode::Manual,
            choice: pending.fingerprint.clone(),
        },
        "the waiting seat's own panel goes to its bot"
    );
    assert_ne!(
        branch.gate().pending().map(|panel| panel.fingerprint),
        Some(pending.fingerprint.clone()),
        "that panel is closed"
    );
    let released = answer_to(&branch, &actor, &pending.fingerprint);
    assert_eq!(
        released.provenance,
        Provenance::Policy,
        "the bot answered the released decision"
    );
    let driven = drive(&branch, AdvanceGoal::Steps(2));
    assert!(driven.frames >= 1, "the engine moved on after the release");
}
#[test]
fn delegating_answers_one_decision_and_keeps_the_seat_manual() {
    let branch = start(&ALL_SEATS);
    branch
        .gate()
        .run(AdvanceGoal::Steps(3))
        .expect("steps are accepted");
    let pending = park(&branch);
    let actor = pending.actor.clone();

    assert_eq!(
        branch
            .gate()
            .delegate_pending(pending.offer)
            .expect("a panel was open"),
        actor,
        "delegation answers the panel that is open"
    );
    assert!(
        branch.gate().snapshot().seats.is_manual(&actor),
        "the seat stays manual"
    );
    let delegated = answer_to(&branch, &actor, &pending.fingerprint);
    assert_eq!(delegated.provenance, Provenance::DelegatedToPolicy);

    // The next decision for that seat must be a panel again, not a second silent delegation.
    let driven = drive(&branch, AdvanceGoal::Steps(120));
    let later = driven
        .parks
        .iter()
        .find(|panel| panel.actor == actor)
        .expect("the manual seat asks again after a delegation");
    assert_ne!(
        later.fingerprint, pending.fingerprint,
        "a different decision"
    );
    assert!(
        branch.gate().snapshot().seats.is_manual(&actor),
        "still manual after the delegation was spent"
    );
}

#[test]
fn a_run_of_n_steps_stops_at_exactly_n_when_every_seat_is_auto() {
    let branch = start(&[]);
    let mut expected = 1_usize; // frame 0, recorded at setup
    for wanted in [1_usize, 3, 7] {
        let driven = drive(&branch, AdvanceGoal::Steps(wanted));
        assert_eq!(
            driven.frames, wanted,
            "a run of {wanted} steps reports {wanted} frames"
        );
        assert!(
            driven.parks.is_empty(),
            "no seat is manual, so nothing may park"
        );
        expected += wanted;
    }
    let session = branch
        .into_session()
        .expect("the thread hands the session back");
    assert_eq!(
        session.frames.len(),
        expected,
        "the session agrees with the events about how far the branch got"
    );
    assert_eq!(session.manifest.seed, SEED);
    assert_eq!(session.manifest.rotation, ROTATION);
}

#[test]
fn pause_ends_the_goal_on_a_step_boundary_and_stop_is_terminal() {
    let branch = start(&[]);
    branch
        .gate()
        .run(AdvanceGoal::Steps(10_000))
        .expect("a long run is accepted");
    branch.gate().pause();
    assert!(
        branch.wait_until_idle(WAIT),
        "pause ended the goal instead of running to its target"
    );
    let frames = branch
        .drain_events()
        .into_iter()
        .filter(|event| matches!(event, LiveEvent::Frame(_)))
        .count();
    assert!(frames < 10_000, "the pause landed, but {frames} steps ran");
    assert_eq!(
        branch.gate().state(),
        LiveState::Ready,
        "ready, not stopped"
    );

    // Still alive: a fresh, smaller goal runs to completion.
    let driven = drive(&branch, AdvanceGoal::Steps(3));
    assert_eq!(driven.frames, 3, "the branch advanced again after pausing");

    branch.gate().stop();
    assert_eq!(branch.gate().state(), LiveState::Stopped);
    assert_eq!(
        branch.gate().run(AdvanceGoal::Steps(1)),
        Err(LiveError::NotRunnable(LiveState::Stopped)),
        "a stopped branch refuses to run"
    );
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        branch
            .drain_events()
            .into_iter()
            .filter(|event| matches!(event, LiveEvent::Frame(_)))
            .count(),
        0,
        "a stopped branch does not advance"
    );
}

/// A compact but decision-sensitive view of a session: enough to tell two playthroughs apart.
fn digest(session: &ti4_review::ReviewSession) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for frame in &session.frames {
        let _ = write!(
            out,
            "{}|{}|{}|{:?}|{}|",
            frame.index,
            frame.engine_step,
            frame.round,
            frame.phase,
            frame.active.as_deref().unwrap_or("-"),
        );
        for decision in &frame.decisions {
            let _ = write!(
                out,
                "{}>{}:{:?};",
                decision.player,
                decision.path,
                decision.chosen.as_deref().unwrap_or("-"),
            );
        }
        out.push('\n');
    }
    out
}

/// Run the same script twice, byte for byte: three separate advance commands of ten steps each.
fn script(branch: &LiveBranch) -> usize {
    let mut frames = 0;
    for _ in 0..3 {
        frames += drive(branch, AdvanceGoal::Steps(10)).frames;
    }
    frames
}

#[test]
fn the_same_command_script_produces_the_same_session() {
    // A deterministic command script on an all-Auto branch must reproduce the reviewer's own
    // outcome: the gate must not perturb the engine's ordering or RNG.
    let first = start(&[]);
    let frames_first = script(&first);
    let session_first = first.into_session().expect("first session");

    let second = start(&[]);
    let frames_second = script(&second);
    let session_second = second.into_session().expect("second session");

    assert_eq!(frames_first, frames_second);
    assert_eq!(
        digest(&session_first),
        digest(&session_second),
        "the same script produced different play"
    );
    // Cross-check against the reviewer's own single-call form of the same walk.
    let third = start(&[]);
    let one_call = drive(&third, AdvanceGoal::Steps(30)).frames;
    let session_third = third.into_session().expect("third session");
    assert_eq!(
        one_call, frames_first,
        "three commands of ten cover thirty steps"
    );
    assert_eq!(
        digest(&session_first),
        digest(&session_third),
        "batching the same steps into three commands changed the game"
    );
}

/// UI-05: a person answering for a seat sees the numbers the policy would have sampled from.
#[test]
fn a_manual_offer_carries_the_policys_numbers() {
    let branch = start(&ALL_SEATS);
    branch
        .gate()
        .run(AdvanceGoal::Steps(1))
        .expect("one step is accepted");
    let pending = park(&branch);
    let probabilities: Vec<f64> = pending
        .options
        .iter()
        .map(|option| {
            option
                .probability
                .unwrap_or_else(|| panic!("option {} has no probability", option.id))
        })
        .collect();
    let total: f64 = probabilities.iter().sum();
    assert!(
        (total - 1.0).abs() < 1e-6,
        "the shares are a distribution over the offered options, got {total}"
    );
    assert!(
        pending.options.iter().all(|option| option.score.is_some()),
        "every option carries its score"
    );
}
