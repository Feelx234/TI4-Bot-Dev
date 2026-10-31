//! `Arm::Ask`: the question the table is asked (`AskArm`). The halves lane B owns: a turn-flow job that is open and suspended on a
//! question of its own, the strategy draft, the starting technology and the closing menu (KP4). The other sources of a question (the
//! action menu, a follower window, ...) belong to their lanes; until they are native the arm stops with a counted gap.

use crate::choice::Pick;
use crate::driver::live::{advance, ask, is_open, Asking, Env, Live, LiveError, Out};
use crate::driver::session::{live_ask_arm, Facts};
use crate::kernel::ability::Unknown;
use crate::kernel::turn_codes::TurnGap;
use crate::state::CoreState;

use super::TurnFlowWin;

/// The gap of a question source no lane has made native yet.
fn not_native(core: &CoreState) -> LiveError {
    let arm = live_ask_arm(&Facts::of_core(core, false));
    LiveError::Unknown(Unknown::TurnGap(TurnGap::AskArm(arm as u8)))
}

/// The question the arm waits on.
///
/// # Errors
/// [`TurnGap::AskArm`] for a source of questions that is not native.
pub fn ask_ask(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    if is_open::<TurnFlowWin>(core) {
        return ask::<TurnFlowWin>(core, env);
    }
    Err(not_native(core))
}

/// Answer the arm's question (or run the job it has open).
///
/// # Errors
/// As [`ask_ask`]; the position is then untouched.
pub fn advance_ask(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    if is_open::<TurnFlowWin>(core) {
        return advance::<TurnFlowWin>(core, env, answer);
    }
    Err(not_native(core))
}
