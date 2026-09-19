//! One live branch: a simulation thread, and the gate the user's commands pass through.
//!
//! ## Why there is a thread at all
//!
//! [`plans/R02_REPLAYER.md`] asks to "keep execution single-threaded" and to pause by inspecting
//! `Game::legal_options()` before each `Game::step()`. Measured against the reference session
//! `out/reviews/pruned-diplomacy-seed11-t05.ti4review.json` (603 frames), that probe cannot deliver
//! the locked behaviour, for three independent reasons:
//!
//! 1. **A step settles more than one decision.** 670 settled decisions over 602 engine steps; 37
//!    steps (6.1%) settled more than one, and a production step settled eight. Pausing at a step
//!    boundary therefore cannot be a pause "before the seat's next choice, including reactions,
//!    transactions, combat, payments, production and agendas".
//! 2. **The probe cannot see what it would pause for.** `legal_options()` never inspects
//!    `Game::trade`, and `Game::step()` dispatches trade, voting, aftermath, diplomacy, agenda
//!    talks, tokens, scoring and `technology::start_turn` (which itself loops) before it asks
//!    anything — 116 `Table::ask`/`ask_seeing` call sites across 22 modules.
//! 3. **`legal_options()` is a hint.** The authoritative event is the decorator being asked. That is
//!    the only point that sees every decision, at the moment the engine actually needs one.
//!
//! A pause taken *inside* `Table::ask` needs someone on the other side of the pause, so this module
//! runs the game on one thread and puts the user's commands behind a [`Gate`]. `Rc<RefCell<_>>` is
//! replaced by a mutex and two condition variables — the execution is still single-threaded, in that
//! exactly one thread ever touches `Game`; only the *waiting* is concurrent. Aborting a step and
//! retrying it was rejected: `Game` is not `Clone`, and the open windows, `prepared_turn_seq`,
//! `event_sequence` and galaxy live on `Game`, so a retry could not be reconstructed.
//!
//! ## Deadlock-freedom
//!
//! Commands are not messages: the UI thread mutates the [`Gate`] directly and gets its answer
//! synchronously. The only sim→UI channel is bounded and written with `try_send`; a full queue
//! counts the drop in [`Gate::dropped_events`] rather than blocking the simulation, because a
//! simulation blocked on a UI that is waiting for the simulation is the one way this design could
//! hang. Nothing the UI waits on is behind that channel: a pending offer is read from the gate.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender, SyncSender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use ti4_engine::choice::Choice;
use ti4_model::id::PlayerId;
use ti4_review::{
    AdvanceReport, AdvanceUnit as ReviewAdvanceUnit, LiveReview, ReviewFrame, ReviewSession,
    SessionOutcome, SimulationConfig,
};

use crate::control::{
    ManualControl, ManualSubmission, ModeEffect, PendingManualChoice, Provenance, ReplayRecord,
    SeatControl, SeatMode, SubmitOutcome,
};
use crate::decider::{AnswerLog, ManualFallbacks, ManualInbox};
use crate::rebuild::{Mismatch, PrefixAnswer, RebuildError, ReplayScript};

/// Events the simulation may queue for the UI before it starts dropping them.
///
/// Generous (one event per step) and deliberately finite; overflow is counted, never silent.
pub const MAX_QUEUED_EVENTS: usize = 4_096;

/// How long a UI thread will wait for the simulation thread to notice it should stop.
pub const SHUTDOWN_GRACE: Duration = Duration::from_millis(5_000);

/// Where the live branch is in its lifecycle.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveState {
    /// Built, never advanced.
    #[default]
    Ready,
    /// Advancing, or asked to advance.
    Running,
    /// Parked inside `Table::ask` on a manual seat; nothing has been mutated.
    WaitingForHuman,
    /// The user stopped it; no further advance is accepted.
    Stopped,
    /// The game reached a terminal state.
    Completed,
    /// The engine or the reviewer failed; the session is inspectable but not playable.
    Failed,
}

impl LiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Running => "running",
            Self::WaitingForHuman => "waiting_for_human",
            Self::Stopped => "stopped",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    /// Whether an advance command is allowed in this state.
    #[must_use]
    pub const fn accepts_run(self) -> bool {
        matches!(self, Self::Ready | Self::Running | Self::WaitingForHuman)
    }
}

/// What the next advance should run to.
///
/// `Steps`, `NextRound` and `EndOfGame` are driven one engine step at a time so the UI sees every
/// frame and so `pause`/`stop` land on a step boundary. `Decisions` and `Actions` are handed to the
/// reviewer's own `advance`, which is how R01 counts them; they report one batch instead of a frame
/// per step, and a manual seat still pauses inside them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdvanceGoal {
    /// Exactly this many engine steps.
    Steps(usize),
    /// Until the round counter advances past where it started.
    NextRound,
    /// Until the game is terminal.
    EndOfGame,
    /// The reviewer's own `AdvanceUnit::Decision` budget.
    Decisions(usize),
    /// The reviewer's own `AdvanceUnit::Action` budget.
    Actions(usize),
}

/// The parts of a frame a UI needs while a branch is live.
///
/// Deliberately not a `ReviewFrame`: those carry a whole `GameState` each (114–183 KB measured), and
/// the live branch must be able to queue thousands of these.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameTick {
    pub index: usize,
    pub engine_step: usize,
    pub round: u32,
    pub phase: String,
    pub active: Option<String>,
    pub decisions: usize,
    pub finished: bool,
    pub error: Option<String>,
}

impl FrameTick {
    fn of(frame: &ReviewFrame) -> Self {
        Self {
            index: frame.index,
            engine_step: frame.engine_step,
            round: frame.round,
            phase: format!("{:?}", frame.phase),
            active: frame.active.clone(),
            decisions: frame.decisions.len(),
            finished: frame.finished,
            error: frame.error.clone(),
        }
    }
}

/// What the simulation has to say about itself.
#[derive(Clone, Debug, PartialEq)]
pub enum LiveEvent {
    /// The game is set up and about to advance.
    Started { seats: Vec<PlayerId>, frames: usize },
    /// One engine step was appended.
    Frame(FrameTick),
    /// A manual seat is parked with this offer (also readable from [`Gate::pending`]).
    Waiting(Box<PendingManualChoice>),
    /// The offer was answered, by whoever answered it.
    Resumed {
        actor: PlayerId,
        provenance: Provenance,
    },
    /// A whole advance call finished (the batch form of [`Self::Frame`]).
    Batch(AdvanceReport),
    /// Lifecycle moved.
    State(LiveState),
    /// Something unrecoverable happened; the state is `Failed`.
    Failed(String),
}

/// Why a command could not be carried out.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveError {
    /// The branch is `Stopped`, `Completed` or `Failed`; nothing more can run.
    NotRunnable(LiveState),
    /// The simulation thread panicked or was lost.
    ThreadLost,
}

/// The reply the parked simulation gets from the gate.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Answer {
    /// The human clicked this option id.
    Option { option_id: String },
    /// The human let the policy answer this one decision.
    Delegated,
    /// The seat went to `Auto`, so the panel closed without an answer.
    Released,
    /// The branch was shut down while parked; the policy must unwind the engine.
    Shutdown,
}

#[derive(Debug)]
struct GateState {
    control: ManualControl,
    answer: Option<Answer>,
    goal: Option<AdvanceGoal>,
    pause: bool,
    state: LiveState,
    shutdown: bool,
}

impl GateState {
    fn new(seats: SeatControl) -> Self {
        Self {
            control: ManualControl::new(seats),
            answer: None,
            goal: None,
            pause: false,
            state: LiveState::Ready,
            shutdown: false,
        }
    }
}

/// What the waiting-answer slot had to say about one ask.
enum Queued {
    /// An answer was waiting for this exact offer.
    Matched(String),
    /// An answer was waiting for a different offer; already counted.
    Stale,
    /// Nothing was waiting.
    Empty,
}

/// What the gate told the decorator to do about one ask.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum GateDecision {
    /// Use this option; a person chose it.
    Human { option_id: String },
    /// Ask the policy underneath. `delegated` distinguishes a one-shot delegation.
    Policy { delegated: bool },
    /// Use this recorded option, but invoke the policy once underneath and discard its answer, so the
    /// sampling stream stays where the game being branched left it.
    Replay { option_id: String },
    /// The recorded prefix does not match this ask. Refuse it: a rebuild that cannot account for a
    /// decision must not hand back a playable branch.
    Refused,
}

/// Ask identity within the live branch, written only by the simulation thread.
#[derive(Debug, Default)]
pub struct AskClock {
    frame: AtomicU64,
    ask: AtomicU32,
}

impl AskClock {
    /// Mark the step boundary the engine is stopped on, restarting the ask counter.
    fn begin_frame(&self, frame: u64) {
        self.frame.store(frame, Ordering::Relaxed);
        self.ask.store(0, Ordering::Relaxed);
    }

    /// Which ask within the current frame this is, counting from 0.
    fn next_ask(&self) -> (u64, u32) {
        let ask = self.ask.fetch_add(1, Ordering::Relaxed);
        (self.frame.load(Ordering::Relaxed), ask)
    }
}

/// The single shared control object of one live branch.
///
/// The UI thread calls the command methods and reads the getters; the simulation thread calls
/// [`Gate::decision_for`] from inside `Table::ask` and the loop's own `await_goal`/`take_pause`. One
/// mutex, so there is no ordering problem between a mode change and a pending offer — the two things
/// that must never disagree are in the same lock.
#[derive(Debug)]
pub struct Gate {
    state: Mutex<GateState>,
    /// Signalled when an answer, a release or a shutdown may wake a parked ask.
    answered: Condvar,
    /// Signalled when a new advance goal, or a pause, needs the loop's attention.
    work: Condvar,
    inbox: ManualInbox,
    log: AnswerLog,
    clock: AskClock,
    /// The branch's event queue, attached once by [`LiveBranch::start`]. `None` for a detached gate,
    /// where there is nobody to tell.
    events: Mutex<Option<SyncSender<LiveEvent>>>,
    /// A recorded prefix to replay, if this gate is rebuilding rather than playing.
    replay: Mutex<Option<ReplayScript>>,
    /// The first prefix mismatch, if a replay refused.
    divergence: Mutex<Option<Mismatch>>,
    /// A policy failure underneath a replayed choice, which means the RNG could not be realigned.
    policy_failure: Mutex<Option<RebuildError>>,
    /// Prefix choices replayed, and times the policy underneath was invoked.
    replayed: AtomicUsize,
    policy_calls: AtomicUsize,
    /// Recorded settled decisions, when recording is on. This is what R02-005 persists.
    recording: AtomicBool,
    records: Mutex<Vec<ReplayRecord>>,
    /// Whether an unanswered manual ask may park. `false` for a detached gate (unit tests and a
    /// rebuild), where answers must already be queued or replayed.
    blocking: bool,
    unanswered: AtomicUsize,
    stale: AtomicUsize,
    dropped_events: AtomicUsize,
    shutdown_fallbacks: AtomicUsize,
    /// Which `Decider` entry point the engine used, counted per ask.
    ///
    /// This exists because the two paths cannot be distinguished from outside the engine, and "the
    /// bound path also works" is worth measuring rather than arguing: `SeatObservation::bind` is
    /// `pub(crate)`, so no test can construct one by hand.
    asks_viewless: AtomicUsize,
    asks_bound: AtomicUsize,
}

impl Gate {
    /// A gate for one live branch: unanswered manual asks park until answered.
    #[must_use]
    pub fn live(seats: SeatControl) -> Self {
        Self::build(seats, true)
    }

    /// A gate that never parks: a manual ask with no queued answer falls back to the policy,
    /// counted. Used by unit tests and by anything that must not block.
    #[must_use]
    pub fn detached(seats: SeatControl) -> Self {
        Self::build(seats, false)
    }

    /// A gate that replays a recorded prefix: while the script lasts every ask is answered from it,
    /// with the policy invoked once underneath and discarded, and each answer validated. Nothing
    /// parks, because a rebuild has no human in front of it.
    #[must_use]
    pub fn replaying(seats: SeatControl, script: ReplayScript) -> Self {
        let gate = Self::build(seats, false);
        *gate
            .replay
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(script);
        gate
    }

    /// Mark the step boundary the engine is stopped on, restarting the per-frame ask counter. Whoever
    /// is driving the engine calls this before every step.
    pub fn begin_frame(&self, frame: u64) {
        self.clock.begin_frame(frame);
    }

    /// The frame and ask ordinal of the ask most recently taken, i.e. the one being answered now.
    #[must_use]
    pub fn current_ordinal(&self) -> (u64, u32) {
        (
            self.clock.frame.load(Ordering::Relaxed),
            self.clock.ask.load(Ordering::Relaxed).saturating_sub(1),
        )
    }

    /// Record every settled decision as a [`ReplayRecord`]. Off by default: the reviewer's own trace
    /// already exists, and a rebuild should not pay for a second record set it never reads.
    pub fn enable_recording(&self) {
        self.recording.store(true, Ordering::Relaxed);
    }

    /// The settled decisions recorded so far, in ask order.
    #[must_use]
    pub fn records(&self) -> Vec<ReplayRecord> {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Prefix choices replayed.
    #[must_use]
    pub fn replayed(&self) -> usize {
        self.replayed.load(Ordering::Relaxed)
    }

    /// Times the policy underneath a decorator was invoked. A replay must count exactly one call per
    /// prefix choice, or the RNG alignment the plan requires has not happened.
    #[must_use]
    pub fn policy_calls(&self) -> usize {
        self.policy_calls.load(Ordering::Relaxed)
    }

    pub(crate) fn record_policy_call(&self) {
        self.policy_calls.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_policy_failure(&self, actor: PlayerId, detail: String) {
        let (frame, _ask) = self.current_ordinal();
        let mut slot = self
            .policy_failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot.is_none() {
            *slot = Some(RebuildError::PolicyFailed {
                frame,
                actor,
                detail,
            });
        }
    }

    /// The first prefix mismatch, if the replay refused.
    #[must_use]
    pub fn divergence(&self) -> Option<Mismatch> {
        self.divergence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// A policy failure underneath a replayed choice, if one happened.
    #[must_use]
    pub fn policy_failure(&self) -> Option<RebuildError> {
        self.policy_failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Record one settled decision, if recording is on.
    pub(crate) fn record_answer(
        &self,
        choice: &Choice,
        chosen: &str,
        provenance: Provenance,
        frame: u64,
        ask: u32,
    ) {
        if !self.recording.load(Ordering::Relaxed) {
            return;
        }
        let Ok(pending) =
            PendingManualChoice::new(crate::control::BranchId::SOURCE, frame, ask, None, choice)
        else {
            return;
        };
        if let Ok(record) = ReplayRecord::record(&pending, chosen, provenance) {
            self.records
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(record);
        }
    }

    fn build(seats: SeatControl, blocking: bool) -> Self {
        Self {
            state: Mutex::new(GateState::new(seats)),
            answered: Condvar::new(),
            work: Condvar::new(),
            inbox: ManualInbox::new(),
            log: AnswerLog::new(),
            clock: AskClock::default(),
            events: Mutex::new(None),
            replay: Mutex::new(None),
            divergence: Mutex::new(None),
            policy_failure: Mutex::new(None),
            replayed: AtomicUsize::new(0),
            policy_calls: AtomicUsize::new(0),
            recording: AtomicBool::new(false),
            records: Mutex::new(Vec::new()),
            blocking,
            unanswered: AtomicUsize::new(0),
            stale: AtomicUsize::new(0),
            dropped_events: AtomicUsize::new(0),
            shutdown_fallbacks: AtomicUsize::new(0),
            asks_viewless: AtomicUsize::new(0),
            asks_bound: AtomicUsize::new(0),
        }
    }

    fn lock(&self) -> MutexGuard<'_, GateState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Attach the queue this gate reports into. Only the simulation thread ever sends.
    fn attach_events(&self, sender: SyncSender<LiveEvent>) {
        if let Ok(mut slot) = self.events.lock() {
            *slot = Some(sender);
        }
    }

    /// Report one event, counting it as dropped rather than blocking when the queue is full.
    pub(crate) fn publish_event(&self, event: LiveEvent) {
        let Ok(slot) = self.events.lock() else {
            return;
        };
        let Some(sender) = slot.as_ref() else {
            return;
        };
        if matches!(
            sender.try_send(event),
            Err(mpsc::TrySendError::Full(_) | mpsc::TrySendError::Disconnected(_))
        ) {
            self.dropped_events.fetch_add(1, Ordering::Relaxed);
        }
    }

    // ---------------------------------------------------------------- UI side

    /// The seat modes and the pending offer, for drawing the panel.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let guard = self.lock();
        Snapshot {
            state: guard.state,
            seats: guard.control.seats().clone(),
            pending: guard.control.pending().cloned(),
        }
    }

    #[must_use]
    pub fn state(&self) -> LiveState {
        self.lock().state
    }

    #[must_use]
    pub fn pending(&self) -> Option<PendingManualChoice> {
        self.lock().control.pending().cloned()
    }

    /// Mode the seat had before, and what that did to a waiting panel.
    pub fn set_mode(&self, seat: &PlayerId, mode: SeatMode) -> ModeEffect {
        let mut guard = self.lock();
        let effect = guard.control.set_mode(seat, mode);
        if matches!(effect, ModeEffect::ReleasedBot { .. }) {
            guard.answer = Some(Answer::Released);
            self.answered.notify_all();
        }
        self.work.notify_all();
        effect
    }

    /// Flip one seat. Changing a seat never touches another seat's panel.
    pub fn toggle(&self, seat: &PlayerId) -> ModeEffect {
        let mode = if self.lock().control.seats().is_manual(seat) {
            SeatMode::Auto
        } else {
            SeatMode::Manual
        };
        self.set_mode(seat, mode)
    }

    /// Answer the offer on screen.
    ///
    /// Refusals (stale fingerprint, invented option, duplicate, nothing pending) leave everything as
    /// it was: the panel stays open and the engine stays parked. That is the whole point of the
    /// fingerprint, and it is [`ManualControl`]'s state machine doing it, not a second copy.
    pub fn submit(&self, submission: &ManualSubmission) -> SubmitOutcome {
        let mut guard = self.lock();
        let outcome = guard.control.submit(submission);
        if let SubmitOutcome::Accepted { option_id } = &outcome {
            guard.answer = Some(Answer::Option {
                option_id: option_id.clone(),
            });
            self.answered.notify_all();
        }
        outcome
    }

    /// Let the policy answer *this* decision and keep the seat manual.
    ///
    /// # Errors
    /// [`LiveError::NotRunnable`] when no panel is open. There is nothing to delegate until the
    /// engine has actually asked, and quietly queueing a delegation for a decision that may never
    /// come would let a stray click answer a later one.
    pub fn delegate_pending(&self) -> Result<PlayerId, LiveError> {
        let mut guard = self.lock();
        let Some(actor) = guard.control.delegate_pending_once() else {
            return Err(LiveError::NotRunnable(guard.state));
        };
        guard.answer = Some(Answer::Delegated);
        self.answered.notify_all();
        Ok(actor)
    }

    /// Let the policy answer this seat's *next* decision, keeping the seat manual. The early-click
    /// form of [`Self::delegate_pending`]: the same one-shot, made before the engine reached the ask.
    pub fn delegate_seat_once(&self, seat: &PlayerId) {
        self.lock().control.delegate_seat_once(seat);
    }

    /// Ask the loop to advance. An advance issued while one is running is taken after it finishes;
    /// a goal requested but not yet started is replaced by the newer one.
    ///
    /// # Errors
    /// [`LiveError::NotRunnable`] when the branch is `Stopped`, `Completed` or `Failed`.
    pub fn run(&self, goal: AdvanceGoal) -> Result<(), LiveError> {
        let mut guard = self.lock();
        if !guard.state.accepts_run() || guard.shutdown {
            return Err(LiveError::NotRunnable(guard.state));
        }
        guard.goal = Some(goal);
        guard.pause = false;
        Self::set_state_locked(&mut guard, LiveState::Running);
        self.work.notify_all();
        Ok(())
    }

    /// Stop at the next step boundary, keeping any pending choice.
    pub fn pause(&self) {
        self.lock().pause = true;
        self.work.notify_all();
    }

    /// Stop for good at the next step boundary. A pending panel is *not* discarded: the plan is
    /// explicit that stopping is a step-boundary operation, and silently answering a human's
    /// question is not how a stop should behave.
    pub fn stop(&self) {
        let mut guard = self.lock();
        guard.pause = true;
        guard.goal = None;
        if guard.state.accepts_run() {
            Self::set_state_locked(&mut guard, LiveState::Stopped);
        }
        self.work.notify_all();
        self.answered.notify_all();
    }

    /// Tell the loop to unwind and hand the review back.
    ///
    /// If a manual seat is parked, the policy answers that one decision so the engine can return
    /// from `ask` — there is no way to abandon a step mid-ask — and the count is recorded in
    /// [`Gate::shutdown_fallbacks`] so nobody mistakes the resulting session for a clean one.
    pub fn shutdown(&self) {
        let mut guard = self.lock();
        guard.shutdown = true;
        guard.goal = None;
        guard.pause = true;
        self.answered.notify_all();
        self.work.notify_all();
    }

    #[must_use]
    pub fn is_shutdown(&self) -> bool {
        self.lock().shutdown
    }

    #[must_use]
    pub fn log(&self) -> &AnswerLog {
        &self.log
    }

    #[must_use]
    pub fn inbox(&self) -> &ManualInbox {
        &self.inbox
    }

    /// Defensive counters; see [`ManualFallbacks`].
    #[must_use]
    pub fn fallbacks(&self) -> ManualFallbacks {
        crate::decider::ManualFallbacks {
            unanswered: self.unanswered.load(Ordering::Relaxed),
            stale: self.stale.load(Ordering::Relaxed),
        }
    }

    #[must_use]
    pub fn shutdown_fallbacks(&self) -> usize {
        self.shutdown_fallbacks.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn dropped_events(&self) -> usize {
        self.dropped_events.load(Ordering::Relaxed)
    }

    /// How the engine delivered asks: `(viewless choose, bound choose_seeing)`.
    #[must_use]
    pub fn delivery(&self) -> (usize, usize) {
        (
            self.asks_viewless.load(Ordering::Relaxed),
            self.asks_bound.load(Ordering::Relaxed),
        )
    }

    pub(crate) fn record_delivery(&self, bound: bool) {
        let counter = if bound {
            &self.asks_bound
        } else {
            &self.asks_viewless
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    // --------------------------------------------------------- simulation side

    /// Block until there is something to run, or the branch is shut down.
    fn await_goal(&self) -> Option<AdvanceGoal> {
        let mut guard = self.lock();
        while guard.goal.is_none() && !guard.shutdown {
            guard = self
                .work
                .wait(guard)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        if guard.shutdown {
            return None;
        }
        guard.goal.take()
    }

    /// Whether an advance is already queued for the loop.
    fn goal_queued(&self) -> bool {
        self.lock().goal.is_some()
    }

    /// Take the pause request, if one was made since the last step.
    fn take_pause(&self) -> bool {
        let mut guard = self.lock();
        std::mem::take(&mut guard.pause)
    }

    fn set_state(&self, state: LiveState) {
        let mut guard = self.lock();
        Self::set_state_locked(&mut guard, state);
    }

    fn set_state_locked(guard: &mut MutexGuard<'_, GateState>, state: LiveState) {
        guard.state = state;
    }

    /// Answer one ask from the engine. Called from [`crate::decider::ControlledDecider`] only.
    pub(crate) fn decision_for(&self, seat: &PlayerId, choice: &Choice) -> GateDecision {
        let fingerprint = crate::control::ChoiceFingerprint::from_choice(choice);
        let (frame, ask) = self.clock.next_ask();
        let _ = (frame, ask); // answers are recorded by the decorator, which knows the final option
        // The plan's priority order: replay prefix first, then one-shot delegation, then the Auto
        // policy, then a queued manual answer (consulted in the manual branch below).
        if let Some(answer) = self.replay_answer(seat, choice) {
            if matches!(answer, GateDecision::Replay { .. }) {
                self.replayed.fetch_add(1, Ordering::Relaxed);
            }
            return answer;
        }
        // A queued answer beats the live paths: it is a scripted answer that already knows this
        // offer's fingerprint, and nobody is waiting to be asked. A queued answer for a *different*
        // offer is counted as the controller bug it is and stops there, rather than also being
        // reported as "nobody answered".
        match self.take_queued(&fingerprint) {
            Queued::Matched(option_id) if choice.option(&option_id).is_some() => {
                return GateDecision::Human { option_id };
            }
            Queued::Matched(_impossible) => {
                self.stale.fetch_add(1, Ordering::Relaxed);
                return GateDecision::Policy { delegated: false };
            }
            Queued::Stale => return GateDecision::Policy { delegated: false },
            Queued::Empty => {}
        }
        let (delegated, manual) = {
            let mut guard = self.lock();
            let delegated = guard.control.take_delegation(seat);
            (delegated, guard.control.seats().is_manual(seat))
        };
        if delegated || !manual {
            return GateDecision::Policy { delegated };
        }
        if !self.blocking {
            self.unanswered.fetch_add(1, Ordering::Relaxed);
            debug_assert!(
                false,
                "manual seat {seat} was asked with a detached gate and no queued answer"
            );
            return GateDecision::Policy { delegated: false };
        }
        self.park(seat, choice, &fingerprint, frame, ask)
    }

    /// Answer this ask from the replay prefix, if one is installed and not yet exhausted.
    ///
    /// Validation lives in [`crate::rebuild::ReplayScript`]; a mismatch is recorded once and turns
    /// into [`GateDecision::Refused`], which makes the engine refuse the step. Refusing is the point:
    /// a rebuild that cannot account for a decision must not produce a playable branch.
    fn replay_answer(&self, seat: &PlayerId, choice: &Choice) -> Option<GateDecision> {
        let mut slot = self
            .replay
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let script = slot.as_mut()?;
        match script.answer_for(seat, choice) {
            PrefixAnswer::Done => None,
            PrefixAnswer::Replay { option_id } => Some(GateDecision::Replay { option_id }),
            PrefixAnswer::Mismatch(mismatch) => {
                let mut divergence = self
                    .divergence
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if divergence.is_none() {
                    *divergence = Some(mismatch);
                }
                drop(divergence);
                Some(GateDecision::Refused)
            }
        }
    }

    /// Publish the offer and wait for the user. No mutation has happened and none will until this
    /// returns, because the engine is still inside `ask`.
    fn park(
        &self,
        seat: &PlayerId,
        choice: &Choice,
        fingerprint: &crate::control::ChoiceFingerprint,
        frame: u64,
        ask: u32,
    ) -> GateDecision {
        let offer = match PendingManualChoice::new(
            crate::control::BranchId::SOURCE,
            frame,
            ask,
            None,
            choice,
        ) {
            Ok(offer) => offer,
            Err(_degenerate) => {
                // An empty or absurd offer cannot be shown; letting the policy answer keeps the
                // step valid and is counted as the anomaly it is.
                self.unanswered.fetch_add(1, Ordering::Relaxed);
                return GateDecision::Policy { delegated: false };
            }
        };
        {
            let mut guard = self.lock();
            guard.answer = None;
            guard.control.publish(offer.clone());
            Self::set_state_locked(&mut guard, LiveState::WaitingForHuman);
        }
        self.publish_event(LiveEvent::Waiting(Box::new(offer)));

        loop {
            let mut guard = self.lock();
            while guard.answer.is_none()
                && !guard.shutdown
                && guard
                    .control
                    .pending()
                    .is_some_and(|pending| &pending.fingerprint == fingerprint)
            {
                guard = self
                    .answered
                    .wait(guard)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            let answer = guard.answer.take();
            let shutdown = guard.shutdown;
            drop(guard);
            // Shut down while parked: the policy must unwind the engine so `ask` can return.
            let answer = match answer {
                Some(answer) => answer,
                None if shutdown => Answer::Shutdown,
                // Nothing for this panel yet; wait for the next wake-up.
                None => continue,
            };
            match answer {
                Answer::Option { option_id } => {
                    if choice.option(&option_id).is_some() {
                        self.resume(seat, Provenance::Human);
                        return GateDecision::Human { option_id };
                    }
                    self.stale.fetch_add(1, Ordering::Relaxed);
                }
                Answer::Delegated => {
                    self.resume(seat, Provenance::DelegatedToPolicy);
                    return GateDecision::Policy { delegated: true };
                }
                Answer::Released => {
                    self.resume(seat, Provenance::Policy);
                    return GateDecision::Policy { delegated: false };
                }
                Answer::Shutdown => {
                    self.shutdown_fallbacks.fetch_add(1, Ordering::Relaxed);
                    self.resume(seat, Provenance::Policy);
                    return GateDecision::Policy { delegated: false };
                }
            }
        }
    }

    fn resume(&self, seat: &PlayerId, provenance: Provenance) {
        let mut guard = self.lock();
        if !matches!(guard.state, LiveState::WaitingForHuman) {
            return;
        }
        guard.state = LiveState::Running;
        drop(guard);
        let _ = seat;
        self.publish_event(LiveEvent::Resumed {
            actor: seat.clone(),
            provenance,
        });
    }

    fn take_queued(&self, fingerprint: &crate::control::ChoiceFingerprint) -> Queued {
        match self.inbox.take_matching(fingerprint) {
            Ok(Some(option_id)) => Queued::Matched(option_id),
            Ok(None) => Queued::Empty,
            Err(_stale) => {
                self.stale.fetch_add(1, Ordering::Relaxed);
                Queued::Stale
            }
        }
    }
}

/// What the UI needs to draw one frame of the control surface.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub state: LiveState,
    pub seats: SeatControl,
    pub pending: Option<PendingManualChoice>,
}

/// One running branch: the gate, the event queue, and the thread that owns the game.
///
/// The `LiveReview` is built *inside* the thread and never leaves it while the thread runs. It holds
/// `Rc` and `Box<dyn Decider>`, so it is not `Send`, and smuggling it out would need `unsafe` for no
/// benefit: what a UI or a later package needs is the finished [`ReviewSession`], which is plain
/// owned data and comes back on a terminal channel of its own. That is also why reconstruction
/// (R02-004) starts a thread from the inputs instead of trying to clone a game.
pub struct LiveBranch {
    gate: Arc<Gate>,
    events: Mutex<mpsc::Receiver<LiveEvent>>,
    sessions: Mutex<Option<mpsc::Receiver<ReviewSession>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl LiveBranch {
    /// Start a branch from a checkpoint and map pool. Seats begin wherever `seats` says, and a seat
    /// that says nothing is `Auto`, so an untouched branch plays exactly like the reviewer.
    ///
    /// # Errors
    /// [`LiveError::ThreadLost`] if the simulation thread cannot be spawned.
    pub fn start(config: SimulationConfig, seats: SeatControl) -> Result<Self, LiveError> {
        Self::with_gate(config, Arc::new(Gate::live(seats)))
    }

    /// Start on a gate the caller already built, so modes or a replay prefix can be seeded first.
    ///
    /// # Errors
    /// [`LiveError::ThreadLost`] if the simulation thread cannot be spawned.
    pub fn with_gate(config: SimulationConfig, gate: Arc<Gate>) -> Result<Self, LiveError> {
        let (sender, receiver) = mpsc::sync_channel(MAX_QUEUED_EVENTS);
        let (session_sender, session_receiver) = mpsc::channel();
        gate.attach_events(sender);
        let thread_gate = Arc::clone(&gate);
        let thread = std::thread::Builder::new()
            .name("ti4-live-branch".to_owned())
            .spawn(move || simulate(&config, &thread_gate, &session_sender))
            .map_err(|_| LiveError::ThreadLost)?;
        Ok(Self {
            gate,
            events: Mutex::new(receiver),
            sessions: Mutex::new(Some(session_receiver)),
            thread: Mutex::new(Some(thread)),
        })
    }

    #[must_use]
    pub fn gate(&self) -> &Arc<Gate> {
        &self.gate
    }

    /// The next event, or `None` when nothing is queued right now.
    #[must_use]
    pub fn try_event(&self) -> Option<LiveEvent> {
        self.events.lock().ok()?.try_recv().ok()
    }

    /// Wait for the next event.
    ///
    /// # Errors
    /// [`LiveError::ThreadLost`] when the timeout expires or the thread has stopped sending.
    pub fn event(&self, timeout: Duration) -> Result<LiveEvent, LiveError> {
        let receiver = self.events.lock().map_err(|_| LiveError::ThreadLost)?;
        receiver
            .recv_timeout(timeout)
            .map_err(|_| LiveError::ThreadLost)
    }

    /// Drain every queued event, newest last.
    pub fn drain_events(&self) -> Vec<LiveEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.try_event() {
            events.push(event);
        }
        events
    }

    /// Wait until the branch is parked on a manual decision, or stops trying to advance.
    ///
    /// This is what a test or a UI polls instead of sleeping; it reads the same gate the panel reads.
    #[must_use]
    pub fn wait_until_parked(&self, timeout: Duration) -> Option<PendingManualChoice> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(pending) = self.gate.pending() {
                return Some(pending);
            }
            // `Running` and `WaitingForHuman` both mean a park may still arrive: the second covers
            // the moment just after an answer was accepted but before the engine woke up, where the
            // panel is closed and the state has not yet flipped back. Only a branch at rest — Ready,
            // stopped, completed or failed — says no further ask is coming, and saying so quickly is
            // what makes a missing pause a fast failure instead of a timeout.
            if !matches!(
                self.gate.state(),
                LiveState::Running | LiveState::WaitingForHuman
            ) {
                return None;
            }
            if std::time::Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Wait until the branch is neither advancing nor waiting for a human, i.e. its goal is done.
    #[must_use]
    pub fn wait_until_idle(&self, timeout: Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if !matches!(
                self.gate.state(),
                LiveState::Running | LiveState::WaitingForHuman
            ) {
                return true;
            }
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Stop the thread and take the session it ended with. `None` if the thread panicked or a session
    /// was already taken.
    pub fn into_session(self) -> Option<ReviewSession> {
        self.gate.shutdown();
        let handle = self.thread.lock().ok()?.take();
        if let Some(handle) = handle {
            // A panicked thread produced no session, and pretending otherwise would hand a UI a
            // branch whose engine died.
            handle.join().ok()?;
        }
        let receiver = self.sessions.lock().ok()?.take()?;
        receiver.recv_timeout(SHUTDOWN_GRACE).ok()
    }
}

impl Drop for LiveBranch {
    fn drop(&mut self) {
        // Never join from Drop: a thread parked on a decision cannot be answered by an owner that is
        // already going away. Ask it to unwind and let the runtime retire it.
        self.gate.shutdown();
    }
}

/// The simulation thread's whole life: build, then run goals until told to stop or shut down.
fn simulate(config: &SimulationConfig, gate: &Arc<Gate>, sessions: &Sender<ReviewSession>) {
    let hook = |seat: &PlayerId,
                policy: Box<dyn ti4_engine::choice::Decider>|
     -> Box<dyn ti4_engine::choice::Decider> {
        Box::new(crate::decider::ControlledDecider::new(
            policy,
            seat.clone(),
            Arc::clone(gate),
        ))
    };
    let mut review = match LiveReview::start_with_control(config, &hook) {
        Ok(review) => review,
        Err(error) => {
            gate.set_state(LiveState::Failed);
            gate.publish_event(LiveEvent::Failed(error.to_string()));
            return;
        }
    };
    gate.publish_event(LiveEvent::Started {
        seats: seats_of(&review),
        frames: review.session.frames.len(),
    });

    while let Some(goal) = gate.await_goal() {
        let report = run_goal(&mut review, gate, goal);
        gate.publish_event(LiveEvent::Batch(report));
        if gate.is_shutdown() {
            break;
        }
        if gate.goal_queued() {
            // Another advance was asked for while this one was running. Stay `Running` and take it:
            // flicking to `Ready` between two back-to-back goals would look to a UI like a branch
            // that had come to rest, and a test would stop watching mid-walk.
            continue;
        }
        let finished = review.is_terminal();
        let stalled = matches!(
            review.session.outcome,
            SessionOutcome::SafetyLimit { .. } | SessionOutcome::EngineFailed { .. }
        );
        if stalled {
            gate.set_state(LiveState::Failed);
        } else if finished {
            gate.set_state(LiveState::Completed);
        } else if gate.state() == LiveState::Running {
            gate.set_state(LiveState::Ready);
        }
        let state = gate.state();
        gate.publish_event(LiveEvent::State(state));
        if matches!(
            state,
            LiveState::Stopped | LiveState::Completed | LiveState::Failed
        ) && gate.await_goal().is_none()
        {
            break;
        }
    }

    // A terminal handoff, at most one item: a UI that never asked for the session simply drops it.
    let _ = sessions.send(review.session);
}

/// The physical seats, from the seating order the engine itself recorded.
fn seats_of(review: &LiveReview) -> Vec<PlayerId> {
    review
        .session
        .frames
        .last()
        .map_or_else(Vec::new, |frame| frame.state.seating_order.clone())
}
/// Drive one advance goal: engine step by engine step where the goal means steps, otherwise handed
/// to the reviewer's own counter so `Decision` and `Action` mean what they mean in R01.
fn run_goal(review: &mut LiveReview, gate: &Gate, goal: AdvanceGoal) -> AdvanceReport {
    let start_round = review.session.frames.last().map_or(0, |frame| frame.round);
    let mut report = AdvanceReport {
        steps: 0,
        decisions: 0,
        actions: 0,
        reached_target: false,
    };
    match goal {
        AdvanceGoal::Decisions(count) => return review.advance(ReviewAdvanceUnit::Decision, count),
        AdvanceGoal::Actions(count) => return review.advance(ReviewAdvanceUnit::Action, count),
        AdvanceGoal::Steps(want) => {
            while report.steps < want {
                if !step_once(review, gate, &mut report) {
                    return report;
                }
            }
            report.reached_target = true;
        }
        AdvanceGoal::NextRound => loop {
            if !step_once(review, gate, &mut report) {
                return report;
            }
            if review
                .session
                .frames
                .last()
                .is_some_and(|frame| frame.round > start_round)
            {
                report.reached_target = true;
                return report;
            }
        },
        AdvanceGoal::EndOfGame => loop {
            if !step_once(review, gate, &mut report) {
                return report;
            }
            if review.is_terminal() {
                report.reached_target = true;
                return report;
            }
        },
    }
    report
}

/// One engine step. `false` means the loop must stop: pause, stop, shutdown or terminal.
fn step_once(review: &mut LiveReview, gate: &Gate, report: &mut AdvanceReport) -> bool {
    if gate.take_pause() || gate.is_shutdown() || review.is_terminal() {
        return false;
    }
    let before = review.session.frames.len();
    gate.clock.begin_frame(before as u64);
    review.advance(ReviewAdvanceUnit::Step, 1);
    for frame in &review.session.frames[before..] {
        report.decisions += frame.decisions.len();
        report.actions += usize::from(frame.action_completed);
        gate.publish_event(LiveEvent::Frame(FrameTick::of(frame)));
    }
    report.steps += review.session.frames.len().saturating_sub(before);
    true
}
