//! What the live faces of the held windows that can PARK share (`driver::live::advance` closes a window whenever its step ends in anything but a
//! question; the combat, the aftermath chain and the invasion stay on the stack while a window of the occurrence or a sub-window runs above
//! them). L5.
//!
//! [`advance_parkable`] is `driver::live::advance` with a hook for the end of a step: the window says whether it closes, parks (stays open,
//! its payload written) or fails with a counted gap. [`table_at`] is the ability table of the call with the play facts of THIS position (a card
//! played inside the window reads them).

use crate::choice::Pick;
use crate::driver::live::{Asking, Env, Live, LiveError, Out};
use crate::driver::{Driver, Status};
use crate::kernel::ability::Unknown;
use crate::kernel::stage::{AbilityTable, Held, Window};
use crate::state::CoreState;

/// What a window does when its step ended without a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum After {
    /// The window is over: `closed` runs and it leaves the stack.
    Close,
    /// The window stays on the stack (its payload is written); the caller reads the stack to see what runs next.
    Park,
    /// The window met a position it does not carry: the call fails and the position is what it was before.
    Fail(Unknown),
}

/// The ability table of one call: the game's table with the play facts of this position.
#[must_use]
pub fn table_at(core: &CoreState, env: &Env<'_>) -> AbilityTable {
    let mut table = env.table.clone();
    if let Some(provider) = &table.facts.plays_provider {
        table.facts.plays = (provider.0)(core, env.kernel);
    }
    table
}

/// A driver for `window` at `core`, clocked by the position.
#[must_use]
pub fn driver_of<M: Window>(window: M, core: &CoreState, env: &Env<'_>) -> Driver<M> {
    let mut d = Driver::from_state(window, core, env.registry.clone(), table_at(core, env)).with_seeds(env.seeds.clone());
    let m = core.meta();
    d.resolver.set_phase(m.phase);
    d.resolver.set_active_player(m.active);
    d.resolver.set_speaker(Some(m.speaker));
    d.resolver.set_seating_order(core.seats().seating_order());
    d.resolver.set_counters(u64::from(m.round), u64::from(m.turn_seq));
    d
}

/// Whether `pick` answers `choice`: one of its options, or a number to a count question.
#[must_use]
pub fn is_option(choice: &crate::choice::Choice, pick: &Pick) -> bool {
    match pick {
        Pick::Opt(o) => choice.opts.iter().any(|x| x == o),
        Pick::Number(_) => matches!(choice.ask, crate::choice::Ask::Generic(crate::choice::generic::Ask::Count { .. })),
    }
}

/// `driver::live::advance::<M>` with `after` deciding what a step that ended in no question does. On any error the position is exactly what it
/// was before the call.
///
/// # Errors
/// See [`LiveError`].
pub fn advance_parkable<M: Held>(
    core: &mut CoreState,
    env: &Env<'_>,
    answer: Option<Pick>,
    after: &dyn Fn(&mut M, &mut CoreState, &mut Out) -> After,
) -> Result<(Live, Out), LiveError> {
    let before = core.clone();
    let result = advance_inner::<M>(core, env, answer, after);
    if result.is_err() {
        *core = before;
    }
    result
}

fn advance_inner<M: Held>(
    core: &mut CoreState,
    env: &Env<'_>,
    answer: Option<Pick>,
    after: &dyn Fn(&mut M, &mut CoreState, &mut Out) -> After,
) -> Result<(Live, Out), LiveError> {
    let window = M::from_pending(core, env.kernel).ok_or(LiveError::NotOpen)?;
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
    let mut out = Out { emitted: std::mem::take(&mut d.emitted), opened: std::mem::take(&mut d.opened) };
    d.store(core);
    match status {
        Status::Asking(choice) => {
            d.window.to_pending(core);
            core.pending_mut().nested = d.nested_journal();
            Ok((Live::Asks(Box::new(crate::driver::live::asking_of(&d, choice))), out))
        }
        Status::Done => match after(&mut d.window, core, &mut out) {
            After::Fail(why) => Err(LiveError::Unknown(why)),
            After::Park => {
                d.window.to_pending(core);
                core.pending_mut().nested = None;
                Ok((Live::Done, out))
            }
            After::Close => {
                d.window.closed(core, env.kernel);
                core.pending_mut().close(M::KIND);
                core.pending_mut().nested = None;
                Ok((Live::Done, out))
            }
        },
    }
}
