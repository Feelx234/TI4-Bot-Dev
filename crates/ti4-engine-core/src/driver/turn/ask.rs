//! `Arm::Ask`: the question the table is asked (`AskArm`). The halves lane B owns: a turn-flow job that is open and suspended on a
//! question of its own, the strategy draft, the starting technology and the closing menu (KP4). The other sources of a question (the
//! action menu, a follower window, ...) belong to their lanes; until they are native the arm stops with a counted gap.

use crate::choice::strategy::Opt as SOpt;
use crate::choice::technology::Opt as TOpt;
use crate::choice::turn::Opt as TurnOpt;
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
        AskArm::Closing => closing_question(core, env).map(|q| Some(asking(q))),
        AskArm::ActionMenu => action_menu_question(core, env).map(|q| Some(asking(q))),
        _ => Err(not_native(core)),
    }
}

/// The closing menu of the seat in it (`Game::closing_options`).
fn closing_question(core: &CoreState, env: &Env<'_>) -> Result<crate::choice::Choice, LiveError> {
    let seat = core.pending().turn_closing.ok_or(LiveError::NotOpen)?;
    let roster = roster_of(core, env).ok_or(LiveError::Unknown(Unknown::TurnGap(TurnGap::NoRoster)))?;
    let options = super::closing::options(core, env.kernel, roster, seat).map_err(|why| LiveError::Unknown(Unknown::TurnGap(why)))?;
    let second = crate::driver::menu::second_action(core, seat, core.pending().actions_this_turn);
    Ok(crate::driver::menu::closing_choice(seat, options, second))
}

/// Answer the arm's question (or run the job it has open).
///
/// # Errors
/// As [`ask_ask`]; the position is then untouched.
pub fn advance_ask(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    if is_open::<TurnFlowWin>(core) {
        let before = core.clone();
        let result = advance::<TurnFlowWin>(core, env, answer);
        return chain_trade(core, env, result, before);
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
        AskArm::Closing => {
            let question = closing_question(core, env)?;
            let Some(pick) = answer else { return Ok((Live::Asks(Box::new(asking(question))), Out::default())) };
            // "End your turn" ends it; anything else the menu offers takes an action (Fleet Logistics' second one) or is a free option.
            let chosen = pick_opt(pick);
            if !question.opts.iter().any(|o| *o == chosen) {
                return Err(LiveError::Illegal);
            }
            run_chosen(core, env, chosen)
        }
        AskArm::ActionMenu => {
            let question = action_menu_question(core, env)?;
            let Some(pick) = answer else { return Ok((Live::Asks(Box::new(asking(question))), Out::default())) };
            let chosen = pick_opt(pick);
            if !question.opts.iter().any(|o| *o == chosen) {
                return Err(LiveError::Illegal);
            }
            run_chosen(core, env, chosen)
        }
        _ => Err(not_native(core)),
    }
}

/// The action menu of the active seat (`Game::turn_options`).
fn action_menu_question(core: &CoreState, env: &Env<'_>) -> Result<crate::choice::Choice, LiveError> {
    let seat = core.meta().active.ok_or(LiveError::NotOpen)?;
    let roster = roster_of(core, env).ok_or(LiveError::Unknown(Unknown::TurnGap(TurnGap::NoRoster)))?;
    super::action_menu::question(core, env.kernel, roster, seat).map_err(|why| LiveError::Unknown(Unknown::TurnGap(TurnGap::Menu(why))))
}

/// Run the option a seat chose from the action menu or the closing menu: "end your turn" is the end job, every other option is
/// `TurnJob::Action`.
fn run_chosen(core: &mut CoreState, env: &Env<'_>, chosen: Opt) -> Result<(Live, Out), LiveError> {
    let job = match chosen {
        Opt::Turn(TurnOpt::EndTurn) => TurnJob::End,
        Opt::Turn(option) => TurnJob::Action(option.key().ok_or(LiveError::Illegal)?),
        _ => return Err(LiveError::Illegal),
    };
    let before = core.clone();
    let result = super::advance_job(job, core, env, None);
    chain_trade(core, env, result, before)
}

/// A transaction option opens its window inside the job (after Extreme Duress); the window runs to its first question in the same call, as the
/// string's step does. Any other result stands. `before`: the position the call began with (a refusal leaves it).
fn chain_trade(core: &mut CoreState, env: &Env<'_>, result: Result<(Live, Out), LiveError>, before: CoreState) -> Result<(Live, Out), LiveError> {
    let (live, mut out) = result?;
    if live != Live::Done {
        return Ok((live, out));
    }
    if is_open::<TurnFlowWin>(core) || core.pending().window(crate::state::WindowKind::Trade).is_none() {
        return Ok((Live::Done, out));
    }
    match crate::driver::windows::trade::advance_trade(core, env, None) {
        Ok((live, more)) => {
            out.emitted.extend(more.emitted);
            out.opened.extend(more.opened);
            Ok((live, out))
        }
        Err(e) => {
            *core = before;
            Err(e)
        }
    }
}

fn pick_opt(pick: Pick) -> Opt {
    match pick {
        Pick::Opt(o) => o,
        Pick::Number(_) => Opt::Decline,
    }
}

/// The unit roster of the game's sources (the play facts the table computes from the position).
fn roster_of(core: &CoreState, env: &Env<'_>) -> Option<&'static crate::query::tech::UnitRoster> {
    let provider = env.table.facts.plays_provider.as_ref()?;
    (provider.0)(core, env.kernel)?.roster
}
