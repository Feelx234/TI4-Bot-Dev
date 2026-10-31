//! `Arm::Ask`: the question the table is asked (`AskArm`). The halves lane B owns: a turn-flow job that is open and suspended on a
//! question of its own, the strategy draft, the starting technology and the closing menu (KP4). The other sources of a question (the
//! action menu, a follower window, ...) belong to their lanes; until they are native the arm stops with a counted gap.

use crate::choice::strategy::Opt as SOpt;
use crate::choice::technology::Opt as TOpt;
use crate::choice::{Opt, Pick};
use crate::driver::live::{advance, ask, is_open, Asking, Env, Live, LiveError, Out};
use crate::driver::session::{live_ask_arm, AskArm, Facts};
use crate::state::TurnJob;
use crate::kernel::ability::Unknown;
use crate::kernel::turn_codes::TurnGap;
use crate::state::CoreState;

use super::TurnFlowWin;

/// The gap of a question source no lane has made native yet.
fn not_native(core: &CoreState) -> LiveError {
    let arm = live_ask_arm(&Facts::of_core(core, false));
    LiveError::Unknown(Unknown::TurnGap(TurnGap::AskArm(arm as u8)))
}

fn asking(choice: crate::choice::Choice) -> Asking {
    Asking { choice, nested: None, at: None }
}

/// The question the arm waits on.
///
/// # Errors
/// [`TurnGap::AskArm`] for a source of questions that is not native.
pub fn ask_ask(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    if is_open::<TurnFlowWin>(core) {
        return ask::<TurnFlowWin>(core, env);
    }
    match live_ask_arm(&Facts::of_core(core, false)) {
        AskArm::Draft => Ok(super::draft::question(core).map(asking)),
        AskArm::StartingTech => Ok(super::starting::question(core, env.kernel, env.table.codes.turn.starting_technology).map(asking)),
        _ => Err(not_native(core)),
    }
}

/// Answer the arm's question (or run the job it has open).
///
/// # Errors
/// As [`ask_ask`]; the position is then untouched.
pub fn advance_ask(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    if is_open::<TurnFlowWin>(core) {
        return advance::<TurnFlowWin>(core, env, answer);
    }
    match live_ask_arm(&Facts::of_core(core, false)) {
        AskArm::Draft => {
            let Some(question) = super::draft::question(core) else { return Ok((Live::Done, Out::default())) };
            let Some(pick) = answer else { return Ok((Live::Asks(Box::new(asking(question))), Out::default())) };
            let Pick::Opt(Opt::Strategy(SOpt::Card(card))) = pick else { return Err(LiveError::Illegal) };
            if !question.opts.iter().any(|o| *o == Opt::Strategy(SOpt::Card(card))) {
                return Err(LiveError::Illegal);
            }
            super::advance_job(TurnJob::Draft(card), core, env, None)
        }
        AskArm::StartingTech => {
            let source = env.table.codes.turn.starting_technology;
            let Some(question) = super::starting::question(core, env.kernel, source) else { return Ok((Live::Done, Out::default())) };
            let Some(pick) = answer else { return Ok((Live::Asks(Box::new(asking(question))), Out::default())) };
            let Pick::Opt(Opt::Technology(TOpt::Tech(technology))) = pick else { return Err(LiveError::Illegal) };
            if !question.opts.iter().any(|o| *o == Opt::Technology(TOpt::Tech(technology))) {
                return Err(LiveError::Illegal);
            }
            let Some(roster) = roster_of(core, env) else { return Err(LiveError::Unknown(Unknown::TurnGap(TurnGap::NoRoster))) };
            // The step is atomic: a refusal leaves the position as it was.
            let before = core.clone();
            if !super::starting::apply(core, env.kernel, roster, technology) {
                *core = before;
                return Err(LiveError::Illegal);
            }
            Ok((Live::Done, Out::default()))
        }
        _ => Err(not_native(core)),
    }
}

/// The unit roster of the game's sources (the play facts the table computes from the position).
fn roster_of(core: &CoreState, env: &Env<'_>) -> Option<&'static crate::query::tech::UnitRoster> {
    let provider = env.table.facts.plays_provider.as_ref()?;
    (provider.0)(core, env.kernel)?.roster
}
