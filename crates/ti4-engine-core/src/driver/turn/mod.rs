//! The turn flow as native jobs (package KP, lane B; `docs/todo/D2-islands-composition.md` section 4): the start of a turn, the end of
//! an action and of a turn, the phase changes, the first strategy phase's announcement, and the per-step prelude.
//!
//! One held window ([`TurnFlowWin`], kind [`WindowKind::TurnFlow`]) whose payload is only WHICH job it is ([`TurnJob`]). A job is one
//! `Window::step` that composes the windows the string composes in the same order and asks every question through the tape
//! ([`inline`]); the driver rolls the step back at a question and `driver::live` stores the answers given so far
//! (`Pending::nested`), so a caller that returns between questions, a clone of the position mid-job and a fresh driver all continue
//! identically.
//!
//! Who opens a job: the arms of the step loop open their own ([`advance_prelude`], [`advance_phase`], [`advance_announce`]); the end of
//! an action is opened by whichever arm finished the action ([`open_finish`]; the closing menu's "end your turn" opens
//! [`TurnJob::End`] itself).

pub mod action;
pub mod action_menu;
pub mod ask;
pub mod closing;
pub mod component;
pub mod draft;
pub mod end;
pub mod faction;
pub mod hero;
pub mod facts;
pub mod inline;
pub mod leader;
pub mod phase;
pub mod prelude;
pub mod produce;
pub mod relic;
pub mod start;
pub mod starting;
pub mod technology;
pub mod swaps;

use crate::choice::{Choice, Pick};
use crate::driver::live::{advance, ask, is_open, open, Asking, Env, Live, LiveError, Out};
use crate::kernel::stage::{Held, Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::state::{CoreState, TurnJob, Window as OpenWindow, WindowKind};

use inline::Flow;

/// The held window of a turn-flow job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnFlowWin {
    pub job: TurnJob,
}

impl Window for TurnFlowWin {
    const PARKS: bool = true;

    type Stage = TurnJob;

    fn stage(&self) -> TurnJob {
        self.job
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        // A halted job (a question waits for its answer, or a gap was noted) writes nothing more; the driver rolls the step back.
        let _: Flow = match self.job {
            TurnJob::Prelude => start::run_prelude(sx),
            TurnJob::Finish => end::run_finish(sx),
            TurnJob::End => end::run_end(sx),
            TurnJob::Phase => phase::run_phase(sx),
            TurnJob::Announce => phase::run_announce(sx),
            TurnJob::Reconcile => prelude::run_reconcile(sx),
            TurnJob::Draft(card) => draft::run_draft(sx, card),
            TurnJob::AfterTactical => crate::driver::windows::strategic::after::run_after(sx),
            TurnJob::Strategic(card) => crate::driver::windows::strategic::begin::run_begin(sx, card),
            TurnJob::StatusOpen => crate::driver::windows::status::run_open(sx),
            TurnJob::StatusBookkeeping => crate::driver::windows::status_bookkeeping::run_bookkeeping(sx),
            TurnJob::StatusFinish => crate::driver::windows::status_finish::run_finish(sx),
            TurnJob::AgendaOpen => crate::driver::agenda::flow::run_open(sx),
            TurnJob::AgendaNext => crate::driver::agenda::flow::run_next(sx),
            TurnJob::Action(key) => action::run_action(sx, key),
        };
        Next::Done
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        None
    }

    fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}

impl Held for TurnFlowWin {
    const KIND: WindowKind = WindowKind::TurnFlow;

    fn from_pending(core: &CoreState, _: &Kernel) -> Option<Self> {
        match core.pending().window(WindowKind::TurnFlow)? {
            OpenWindow::TurnFlow(job) => Some(Self { job: *job }),
            _ => None,
        }
    }

    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::TurnFlow(self.job));
    }
}

/// Open `job` (the arm that finished an action opens [`TurnJob::Finish`]). The next live call runs it.
pub fn open_job(core: &mut CoreState, job: TurnJob) {
    open(core, &TurnFlowWin { job });
}

/// The end of an action: `ACTION_COMPLETED`, then the closing menu or the end of the turn (`Game::finish_action`).
pub fn open_finish(core: &mut CoreState) {
    open_job(core, TurnJob::Finish);
}

/// The job open on the stack, if any.
#[must_use]
pub fn open_job_of(core: &CoreState) -> Option<TurnJob> {
    match core.pending().window(WindowKind::TurnFlow)? {
        OpenWindow::TurnFlow(job) => Some(*job),
        _ => None,
    }
}

/// Run `job` live: open it when it is not open (a fresh step), then to its next question or its end.
///
/// # Errors
/// As [`advance`]; [`LiveError::NotOpen`] for an answer when no job is open.
pub fn advance_job(job: TurnJob, core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    // A job this call opens leaves with the call when it fails: the position is as it was.
    let fresh = if is_open::<TurnFlowWin>(core) {
        None
    } else {
        if answer.is_some() {
            return Err(LiveError::NotOpen);
        }
        let before = core.clone();
        open_job(core, job);
        Some(before)
    };
    let result = advance::<TurnFlowWin>(core, env, answer);
    if result.is_err()
        && let Some(before) = fresh
    {
        *core = before;
    }
    result
}

/// The question the open job waits on (`None`: a fresh or absent job asks nothing yet).
///
/// # Errors
/// As [`ask`].
pub fn ask_job(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    if !is_open::<TurnFlowWin>(core) {
        return Ok(None);
    }
    ask::<TurnFlowWin>(core, env)
}

/// `Arm::TurnPrelude`.
///
/// # Errors
/// As [`advance_job`].
pub fn advance_prelude(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance_job(TurnJob::Prelude, core, env, answer)
}

/// `Arm::Phase`.
///
/// # Errors
/// As [`advance_job`].
pub fn advance_phase(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance_job(TurnJob::Phase, core, env, answer)
}

/// The reconcile hooks every step starts with (`Game::step_flow` before the arm is chosen).
///
/// # Errors
/// As [`advance_job`].
pub fn advance_reconcile(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance_job(TurnJob::Reconcile, core, env, answer)
}

/// `Arm::StrategyAnnounce`.
///
/// # Errors
/// As [`advance_job`].
pub fn advance_announce(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance_job(TurnJob::Announce, core, env, answer)
}
