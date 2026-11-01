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

// -- a use of PRODUCTION by an ability, run inside the effect of a table rule (lane M6d) ---------------------------------------------------

/// The facts of a production by an ability for `who` in `system` with `limit` units, read from the position with the kernel the adapter lent
/// for payments (the faction hooks of a production). `None`: the adapter lent none, no roster, or a number beyond 32 bits.
#[must_use]
pub fn ability_facts(core: &CoreState, host: &crate::kernel::Kernel, facts: &crate::kernel::ability::AdapterFacts, who: Seat, system: System, limit: i32) -> Option<ProduceFacts> {
    let k = facts.production_kernel_for(host)?;
    let roster = facts.roster?;
    let home = crate::rules::production::home::home_facts(core, k, roster, who, system, false)?;
    Some(ProduceFacts { limit, ..home.produce })
}

/// Whether a production by an ability of `limit` units in `system` would ask anything now (the string
/// `ProductionWindow::for_ability(..).pending_choice().is_some()`).
#[must_use]
pub fn can_produce_by_ability(core: &CoreState, host: &crate::kernel::Kernel, facts: &crate::kernel::ability::AdapterFacts, who: Seat, system: System, limit: i32) -> bool {
    use crate::kernel::stage::Window;
    let Some(pf) = ability_facts(core, host, facts, who, system, limit) else { return false };
    let Some(k) = facts.production_kernel_for(host) else { return false };
    ProductionWindow::ability(&pf).ask_of(core, k).is_some()
}

/// Run a production by an ability as the effect of a table rule: the use of PRODUCTION (`UseOpen::Ability`) stepped inside the effect, its
/// questions put to the effect's tape, `UNITS_PRODUCED` announced when it made something. `Ok(false)`: the adapter lent no payment kernel or
/// roster (the effect is reported unknown).
///
/// # Errors
/// Never; the result type is the effect's.
pub fn produce_in(eff: &mut crate::kernel::ability::Eff<'_, '_>, who: Seat, system: System, limit: i32) -> Result<bool, crate::kernel::resolver::TimingError> {
    use crate::kernel::stage::Window;
    use crate::kernel::Next;
    let Some(pf) = ability_facts(eff.read().0, eff.read().1, eff.adapter_facts(), who, system, limit) else {
        eff.unknown(crate::kernel::ability::Unknown::MissingFact);
        return Ok(false);
    };
    let Some(k) = eff.adapter_facts().production_kernel_for(eff.kernel()) else { return Ok(false) };
    let mut window = ProductionWindow::ability(&pf);
    window.emit_timing = true;
    let mut machine = ProductionMachine::new(ProductionUse { who, system, mode: UseMode::Ability, phase: UsePhase::Using, window, answered: false });
    for _ in 0..64 {
        // (a bound: a use asks one question per build, payment and placement, and the limit is small)
        let Some(next) = eff.with_pay_sx(|sx| machine.step(sx)) else { return Ok(false) };
        if !matches!(next, Next::Continue(_)) {
            break;
        }
        let Some(choice) = machine.ask_of(eff.state(), k) else { break };
        let pick = eff.ask(&choice);
        if eff.with_pay_sx(|sx| machine.answer(sx, pick)) != Some(Ok(())) {
            break;
        }
    }
    Ok(true)
}
