//! The status phase as native arms (package KS, lane C; `Game::step_status`, `step_scoring`, `step_event_scoring`, `step_token_gain`).
//!
//! Four arms, one machine each, all HELD windows or jobs of the turn flow:
//!
//! * `Arm::Status` ([`advance_status`], KS2): a self-opening job ([`TurnJob::StatusOpen`]) that sets the flag, opens the 81.1 scoring window
//!   and says `STATUS_SCORING_BEGAN`.
//! * `Arm::Scoring` ([`advance_scoring`], KS1): the status window ([`StatusScoring`]); when nobody is left to ask it closes into
//!   [`TurnJob::StatusBookkeeping`] (81.2-81.4, `status_bookkeeping.rs`), which opens the token window.
//! * `Arm::EventScoring` ([`advance_event_scoring`], KS1): the window of an action- or agenda-timed secret ([`EventScoring`]).
//! * `Arm::Tokens` ([`advance_tokens`], KS4): the token gain ([`TokenGain`]); when the last token is placed it closes into
//!   [`TurnJob::StatusFinish`] (81.5-81.8, `status_finish.rs`).
//!
//! A string step that finds nothing to ask is the CLOSE of the window (the next step after the last answer). The native step is
//! "one decision plus everything that follows until the next decision": the call that applies the last answer also runs the close, so
//! the questions and the events of the two string steps come out in the string's order inside one `live_step`. A close that asks
//! (Yssaril's Scheming, the hand limit, the Neuraloop, the redistribution) asks through the tape, so a caller that returns between the
//! questions continues the close from the position alone (`Pending::nested`, lane B's jobs).

use crate::choice::Pick;
use crate::driver::live::{advance, ask, is_open, Asking, Env, Live, LiveError, Out};
use crate::driver::turn::inline::{gap, Flow};
use crate::driver::turn::{advance_job, ask_job, open_job, TurnFlowWin};
use crate::kernel::stage::{Held, Sx};
use crate::kernel::turn_codes::TurnGap;
use crate::query::reach;
use crate::query::seats::initiative_order;
use crate::rules::token_gain::TokenGain;
use crate::state::{CoreState, DriverFlags, PlainKind, PlainRec, TurnJob};
use crate::tables::{Flags, Scope, SystemFlags};

use super::scoring::{EventScoring, StatusScoring};

/// `Game::step_status`'s opening of the 81.1 window, in the string's order: the flag, the window, the event.
pub fn run_open(sx: &mut Sx<'_>) -> Flow {
    // Thunder's Edge: an Entropic Scar grants faction technologies at the start of the phase; the content is out (the scar tiles are TE).
    if scar_in_play(sx.state, reach::scope_of(sx.kernel)) {
        gap(sx, TurnGap::EntropicScar);
        return Flow::Halt;
    }
    sx.state.pending_mut().driver.set(DriverFlags::STATUS_RESOLVED, true);
    let order: Vec<_> = initiative_order(sx.state).iter().copied().collect();
    StatusScoring::status(&order).to_pending(sx.state);
    sx.plain(PlainRec::bare(PlainKind::StatusScoringBegan));
    Flow::Done
}

/// Whether a seat has ships in a system with an Entropic Scar (`entropic_scars::grants_available` > 0 is possible).
fn scar_in_play(core: &CoreState, scope: Scope) -> bool {
    core.board().systems().any(|(system, cell)| {
        scope.system(system).is_some_and(|d| d.has(SystemFlags::SCAR)) && cell.units.as_slice().iter().any(|u| scope.unit(u.kind()).is_some_and(|d| d.ship))
    })
}

/// `Arm::Status`.
///
/// # Errors
/// As [`advance_job`].
pub fn advance_status(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance_job(TurnJob::StatusOpen, core, env, answer)
}

/// The close job to run when a held window ends: runs it in the same call and merges what it did into `out`.
fn close_with(core: &mut CoreState, env: &Env<'_>, job: TurnJob, mut out: Out) -> Result<(Live, Out), LiveError> {
    open_job(core, job);
    let (live, more) = advance::<TurnFlowWin>(core, env, None)?;
    out.emitted.extend(more.emitted);
    out.opened.extend(more.opened);
    Ok((live, out))
}

/// One live call of a scoring window of the family `H` that closes into `job` (`None`: it closes into nothing).
fn advance_window<H: Held + crate::kernel::stage::Window>(
    core: &mut CoreState,
    env: &Env<'_>,
    answer: Option<Pick>,
    close: Option<TurnJob>,
) -> Result<(Live, Out), LiveError> {
    // A close that asked a question from inside its step is a job on the stack: it continues first.
    if is_open::<TurnFlowWin>(core) {
        return advance::<TurnFlowWin>(core, env, answer);
    }
    let before = core.clone();
    let result = advance::<H>(core, env, answer).and_then(|(live, out)| match live {
        // The game ended with the last score (98.7): nothing is closed.
        Live::Done if core.meta().finished => Ok((Live::Done, out)),
        Live::Done => match close {
            Some(job) => close_with(core, env, job, out),
            None => Ok((Live::Done, out)),
        },
        asks @ Live::Asks(_) => Ok((asks, out)),
    });
    if result.is_err() {
        *core = before;
    }
    result
}

/// The question a window of kind `kind` waits on, or the job that closed it.
fn ask_window<H: Held + crate::kernel::stage::Window>(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    if is_open::<TurnFlowWin>(core) {
        return ask_job(core, env);
    }
    ask::<H>(core, env)
}

/// `Arm::Scoring`.
///
/// # Errors
/// As [`advance`], and the gaps of the scoring window and of the close.
pub fn advance_scoring(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance_window::<StatusScoring>(core, env, answer, Some(TurnJob::StatusBookkeeping))
}

/// The question of `Arm::Scoring`.
///
/// # Errors
/// As [`ask`].
pub fn ask_scoring(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    ask_window::<StatusScoring>(core, env)
}

/// `Arm::EventScoring`. The window closes into nothing: the agenda queue an agenda window leaves on the stack is the agenda lane's.
///
/// # Errors
/// As [`advance_scoring`].
pub fn advance_event_scoring(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance_window::<EventScoring>(core, env, answer, None)
}

/// The question of `Arm::EventScoring`.
///
/// # Errors
/// As [`ask`].
pub fn ask_event_scoring(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    ask_window::<EventScoring>(core, env)
}

/// `Arm::Tokens`.
///
/// # Errors
/// As [`advance_scoring`].
pub fn advance_tokens(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance_window::<TokenGain>(core, env, answer, Some(TurnJob::StatusFinish))
}

/// The question of `Arm::Tokens`.
///
/// # Errors
/// As [`ask`].
pub fn ask_tokens(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    ask_window::<TokenGain>(core, env)
}
