//! The window contract (package D1; docs/ENGINE_CORE_DRIVER.md): what a rule area implements so that the
//! driver can run it to its next question and resume it with an answer.
//!
//! A window is a small `Clone` value (its `Stage` plus the few facts it carries) with three functions:
//!
//! * [`Window::step`]: advance until the window must ask ([`Next::Continue`]) or is finished
//!   ([`Next::Done`]). The string `settle`.
//! * [`Window::ask_of`]: the pure description of the question the window is waiting on, from
//!   `(window, state, kernel)` only. The string `pending_choice`.
//! * [`Window::answer`]: apply the answer to that question. The string `resolve`.
//!
//! `step` and `answer` get a [`Sx`]: the state, the kernel, and the three services a window needs that a
//! plain `Cx` does not have: `emit` (an event through the A5 `Resolver`, the string `ctx.emit`), `observe`
//! (the P5 program points) and `ask` (a question that is *not* a stage: a reaction window's decision).
//!
//! # Suspension
//!
//! A window never holds a suspended call stack. A question the window itself puts is a stage
//! (`Next::Continue`); the state is exactly `(window, state)`, so a checkpoint is a clone. A question put
//! by something a window *called* (an ability that resolves inside `sx.emit`) cannot be a stage of this
//! window; it is answered from the [`Tape`]: the driver snapshots `(state, window, resolver)`, runs the
//! step, and if the tape ran dry it rolls back, reports the question, and re-runs the same step from the
//! snapshot with the answer appended. A step is therefore deterministic over `(snapshot, tape)` and must
//! not read anything else (no clock, no global; randomness is the `rng` slice of the state).

use super::resolver::{Resolver, TimingError};
use super::seam::{Obs, Phase, Q};
use crate::state::{RngDomain, SeedSource};
use super::{Asker, Cx, Kernel, Next};
use crate::choice::{Choice, Pick};
use crate::state::{CoreState, EventRec};

/// A pick that was not one of the options of the question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Illegal;

/// The window contract. See the module docs.
pub trait Window: Clone {
    /// One variant per suspension point (and per program point worth naming).
    type Stage: Copy + std::fmt::Debug + PartialEq;

    fn stage(&self) -> Self::Stage;

    /// Run until a question or the end. Every `Next::Continue(ask)` must be the ask of
    /// [`Self::ask_of`] (the driver checks).
    fn step(&mut self, sx: &mut Sx<'_>) -> Next;

    /// The question the window waits on, `None` when it waits on nothing. Pure.
    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice>;

    /// Apply the answer to [`Self::ask_of`].
    ///
    /// # Errors
    /// The pick is not an option of the question.
    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal>;
}

pub use super::ability::{AbilityTable, Applies, CondCx, Codes, Condition, Eff, Effect, Forbids, NestedFacts, RulesHost, Slot, Spec, Tape};

/// The services of a window's `step` and `answer` (the string `Resolving` + `TimingContext`).
pub struct Sx<'a> {
    pub state: &'a mut CoreState,
    pub kernel: &'a Kernel,
    pub resolver: &'a mut Resolver,
    pub table: &'a AbilityTable,
    pub tape: &'a mut Tape,
    /// The next trace id of an emitted event (`fx::announce_staged`'s `next_id`).
    pub next_id: &'a mut u64,
    /// The events the window itself emitted (not the ones the abilities of those windows caused).
    pub emitted: &'a mut Vec<EventRec>,
    /// Seeds the random streams (`SHA-256(seed || domain text)`, the adapter's): see [`Sx::die`].
    pub seeds: &'a dyn SeedSource,
}

impl<'a> Sx<'a> {
    /// The plain `Cx` of the seam functions (`Offer::apply`, observers); questions go to the tape.
    pub fn cx(&mut self) -> Cx<'_> {
        Cx::new(&mut *self.state, self.kernel, &mut *self.tape)
    }

    #[must_use]
    pub fn q(&self) -> Q<'_> {
        Q::new(&*self.state, self.kernel)
    }

    /// Draw a die with `sides` sides from the state's stream of `domain` (`GameRng::die`). The streams are in
    /// the state (`rng` slice), so a rolled-back step restores them with the rest and a re-run draws the same.
    pub fn die(&mut self, domain: RngDomain, sides: u32) -> u32 {
        self.state.rng_mut().die(domain, self.seeds, sides)
    }

    /// Fire the observers of a program point (P5).
    pub fn observe(&mut self, obs: &Obs, phase: Phase) {
        self.cx().observe(obs, phase);
    }

    /// Put a question that is not a stage of this window (the string `table.ask` inside a callee).
    pub fn ask(&mut self, choice: &Choice) -> Pick {
        self.tape.pick(&*self.state, choice)
    }

    /// Emit an event through the WHEN/AFTER windows of the resolver (the string `ctx.emit`); `resolve` is the
    /// event's own resolution between the two windows. A zero id is replaced by the next trace id.
    ///
    /// # Errors
    /// What the resolver refuses with.
    pub fn emit_with(
        &mut self,
        mut rec: EventRec,
        resolve: impl FnOnce(&mut EventRec, &mut RulesHost<'_>, &mut Resolver),
    ) -> Result<EventRec, TimingError> {
        if rec.id == 0 {
            rec.id = *self.next_id;
            *self.next_id += 1;
        }
        let top = self.resolver.emission_stack().is_empty();
        let mut host = RulesHost { state: &mut *self.state, kernel: self.kernel, table: self.table, tape: &mut *self.tape, next_id: &mut *self.next_id, seeds: self.seeds };
        let done = self.resolver.emit(&mut host, rec, resolve)?;
        if top {
            self.emitted.push(done.clone());
        }
        Ok(done)
    }

    /// [`Self::emit_with`] with no resolution of its own.
    ///
    /// # Errors
    /// What the resolver refuses with.
    pub fn emit(&mut self, rec: EventRec) -> Result<EventRec, TimingError> {
        self.emit_with(rec, |_, _, _| {})
    }
}
