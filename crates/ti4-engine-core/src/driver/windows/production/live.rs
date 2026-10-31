//! The live face of the production step: how a use of PRODUCTION is opened and how the arm runs it (the generic `driver::live::advance` of the
//! machine: nothing pauses a use, a scoring window never opens inside it).

use super::ProductionMachine;
use crate::choice::Pick;
use crate::driver::live::{advance, ask, Asking, Env, Live, LiveError, Out};
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::children::ProduceFacts;
use crate::kernel::stage::Held;
use crate::rules::production::window::ProductionWindow;
use crate::state::{CoreState, ProductionUse, UseMode, UsePhase};

/// How a use of PRODUCTION starts (see [`UseMode`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseOpen {
    /// The production step of a tactical action (the aftermath's `Producing`).
    Tactical { who: Seat, system: System },
    /// A use at the home system outside a tactical action (the Warfare secondary).
    Home { who: Seat, system: System },
    /// A production by an ability, the caller's facts being the window (`Spawn::Produce`).
    Ability { facts: ProduceFacts },
}

/// Open a use of PRODUCTION: the payload on the stack. The first [`advance_production`] (`None`) runs the start-of-use effects of the mode and
/// stops at the first question (or runs the use to its end when it asks nothing).
pub fn open_production(core: &mut CoreState, open: UseOpen) {
    let p = match open {
        UseOpen::Tactical { who, system } => ProductionUse { who, system, mode: UseMode::Tactical, phase: UsePhase::Start, window: ProductionWindow::blank(who, system), answered: false },
        UseOpen::Home { who, system } => ProductionUse { who, system, mode: UseMode::Home, phase: UsePhase::Start, window: ProductionWindow::blank(who, system), answered: false },
        UseOpen::Ability { facts } => {
            let mut window = ProductionWindow::ability(&facts);
            window.emit_timing = true;
            ProductionUse { who: facts.who, system: facts.system, mode: UseMode::Ability, phase: UsePhase::Using, window, answered: false }
        }
    };
    ProductionMachine::new(p).to_pending(core);
}

/// The question the open use waits on (the arm's `AskFn`).
///
/// # Errors
/// As [`ask`].
pub fn ask_production(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    ask::<ProductionMachine>(core, env)
}

/// Run the open use to its next question or its end (the arm's `LiveFn`). `Live::Done` is the end of the use: it has left the stack.
///
/// # Errors
/// See [`LiveError`]; the position is then what it was before the call.
pub fn advance_production(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    advance::<ProductionMachine>(core, env, answer)
}
