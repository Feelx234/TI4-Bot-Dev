//! The live face of the tactical action: how it is opened and how the arm runs it.
//!
//! The arm is [`advance_tactical`], a variant of `driver::live::advance::<TacticalAction>`: a window that handed the action to the aftermath
//! ([`TacticalAction::parked`]) stays on the stack under the aftermath's entry; the generic `advance` would close it (a `Next` that is not a
//! question ends the window).

use super::{facts, TacticalAction};
use crate::choice::Pick;
use crate::driver::live::{ask, Asking, Env, Live, LiveError, Out};
use crate::driver::{Driver, Status};
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::stage::Held;
use crate::state::{CoreState, PlainKind, PlainRec, TStage, Tactical};

/// A tactical action begins (the menu's answer): the activation question comes next, and `TACTICAL_ACTION_BEGAN` is announced. The notes
/// every seat holds are remembered for the "after combat" secrets. (`settle_extreme_duress` precedes it: the dispatcher's.)
pub fn open_tactical(core: &mut CoreState, seat: Seat) {
    let notes = facts::note_holdings(core);
    TacticalAction::new(Tactical { player: seat, stage: TStage::Activating, notes_at_start: notes }).to_pending(core);
    fx::plain(core, PlainRec::bare(PlainKind::TacticalActionBegan));
}

/// A free tactical action in `system` (no token; Thunder's Edge Warfare, Z'eu): the first run activates it and goes to the movement step.
pub fn open_free(core: &mut CoreState, seat: Seat, system: System) {
    let notes = facts::note_holdings(core);
    TacticalAction::new(Tactical { player: seat, stage: TStage::Free(system), notes_at_start: notes }).to_pending(core);
}

/// A free tactical action in `system` that begins INSIDE a step of another job (Thunder's Edge Warfare's primary): the activation is
/// announced now, in the order the string says it (`SYSTEM_ACTIVATED`, `FREE_TACTICAL_ACTION`, then the caller's own `STRATEGIC_ACTION_BEGAN`),
/// and the action is left on the stack at its movement step for the next arm call.
pub fn begin_free(sx: &mut crate::kernel::stage::Sx<'_>, seat: Seat, system: System) {
    let notes = facts::note_holdings(sx.state);
    let leaf_facts = crate::rules::tactical::Facts { pool: sx.table.codes.tactical.tactic };
    let mut leaf = crate::rules::tactical::Machine::free(seat, leaf_facts, system).live();
    let _ = crate::kernel::stage::Window::step(&mut leaf, sx);
    super::hooks::marks(sx, seat, system);
    TacticalAction::new(Tactical { player: seat, stage: TStage::Moving, notes_at_start: notes }).to_pending(sx.state);
}

/// The question the open action waits on (the arm's `AskFn`).
///
/// # Errors
/// As [`ask`].
pub fn ask_tactical(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    ask::<TacticalAction>(core, env)
}

/// Run the open action to its next question, its hand-off to the aftermath, or its end (the arm's `LiveFn`). `Live::Done` is the end of the
/// STEP: the action has left the stack, or (a hand-off) it stays on it under the aftermath.
///
/// # Errors
/// See [`LiveError`]; the position is then what it was before the call.
pub fn advance_tactical(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    let before = core.clone();
    let result = advance_inner(core, env, answer);
    if result.is_err() {
        *core = before;
    }
    result
}

fn driver_of(window: TacticalAction, core: &CoreState, env: &Env<'_>) -> Driver<TacticalAction> {
    crate::driver::windows::live_common::driver_of(window, core, env)
}

fn is_option(choice: &crate::choice::Choice, pick: &Pick) -> bool {
    match pick {
        Pick::Opt(o) => choice.opts.iter().any(|x| x == o),
        Pick::Number(_) => matches!(choice.ask, crate::choice::Ask::Generic(crate::choice::generic::Ask::Count { .. })),
    }
}

fn advance_inner(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    let window = TacticalAction::from_pending(core, env.kernel).ok_or(LiveError::NotOpen)?;
    let mut d = driver_of(window, core, env);
    let nested = core.pending_mut().nested.take();
    let status = match (nested, answer) {
        (Some(n), Some(pick)) => {
            d.restore_nested(&n);
            let Status::Asking(choice) = d.reask(core, env.kernel)? else { return Err(LiveError::NotAsking) };
            if !is_option(&choice, &pick) {
                return Err(LiveError::Illegal);
            }
            d.resume(core, env.kernel, pick)?
        }
        (Some(n), None) => {
            d.restore_nested(&n);
            d.reask(core, env.kernel)?
        }
        (None, Some(pick)) => {
            if !d.restore_stage(core, env.kernel) {
                return Err(LiveError::NotAsking);
            }
            d.resume(core, env.kernel, pick)?
        }
        (None, None) => d.run(core, env.kernel)?,
    };
    if let Some(why) = d.unknown() {
        return Err(LiveError::Unknown(why));
    }
    let out = Out { journal: crate::kernel::fx::journal_recs(&d.resolver, 0), emitted: std::mem::take(&mut d.emitted), opened: std::mem::take(&mut d.opened), held: false };
    d.store(core);
    match status {
        Status::Asking(choice) => {
            d.window.to_pending(core);
            core.pending_mut().nested = d.nested_journal();
            Ok((Live::Asks(Box::new(crate::driver::live::asking_of(&d, choice))), out))
        }
        Status::Done => {
            if d.window.parked {
                // Handed to the aftermath: stay open under its entry.
                d.window.to_pending(core);
                core.pending_mut().nested = None;
                return Ok((Live::Done, out));
            }
            d.window.closed(core, env.kernel);
            core.pending_mut().close(TacticalAction::KIND);
            core.pending_mut().nested = None;
            Ok((Live::Done, out))
        }
    }
}
