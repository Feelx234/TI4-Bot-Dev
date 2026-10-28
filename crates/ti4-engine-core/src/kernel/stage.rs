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

use super::resolver::{AbilityDef, CannotDef, Host, Resolver, TimingError};
use super::seam::{Obs, Phase, Q};
use crate::state::{RngDomain, SeedSource};
use super::{Asker, Cx, Kernel, Next};
use crate::choice::reaction::AbilityId;
use crate::choice::{self, Choice, Pick};
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

/// The answers of the current operation, in the order they were given; the nested questions of a step
/// read them in order. `needed` is the first question that found the tape dry.
#[derive(Debug, Clone, Default)]
pub struct Tape {
    answers: Vec<Pick>,
    used: usize,
    needed: Option<Choice>,
}

impl Tape {
    pub fn clear(&mut self) {
        self.answers.clear();
        self.rewind();
    }

    /// Start reading from the first answer again (a re-run of the step).
    pub fn rewind(&mut self) {
        self.used = 0;
        self.needed = None;
    }

    pub fn push(&mut self, pick: Pick) {
        self.answers.push(pick);
    }

    /// The question that found the tape dry in the run just made.
    #[must_use]
    pub fn needed(&self) -> Option<&Choice> {
        self.needed.as_ref()
    }
}

impl Asker for Tape {
    fn pick(&mut self, _: &CoreState, choice: &Choice) -> Pick {
        if let Some(p) = self.answers.get(self.used) {
            self.used += 1;
            return *p;
        }
        if self.needed.is_none() {
            self.needed = Some(choice.clone());
        }
        // The run is rolled back; any option lets it finish.
        choice.pick(0).unwrap_or(Pick::Opt(choice::Opt::Decline))
    }
}

/// A condition of an ability (`Host::condition`).
pub type Condition = fn(&CoreState, &Kernel, &AbilityDef, &EventRec) -> bool;
/// The effect of an ability (`Host::effect`). Events it causes are staged (`fx::stage`), not emitted.
pub type Effect = fn(&mut CoreState, &Kernel, &AbilityDef, &mut EventRec) -> Result<(), TimingError>;
/// The predicate of a registered "cannot" (`Host::forbids`).
pub type Forbids = fn(&CoreState, &CannotDef, &EventRec) -> bool;

/// What an ability does, by id.
#[derive(Clone, Copy)]
pub struct AbilityImpl {
    pub id: AbilityId,
    pub condition: Option<Condition>,
    pub effect: Effect,
}

/// The rules behind the abilities the resolver's registry lists (`Host::condition/effect/forbids`).
/// Empty until the abilities are ported; an ability without an entry is eligible and does nothing.
#[derive(Clone, Default)]
pub struct AbilityTable {
    abilities: Vec<AbilityImpl>,
    forbids: Vec<(u32, Forbids)>,
}

impl AbilityTable {
    pub fn add(&mut self, a: AbilityImpl) {
        self.abilities.push(a);
    }

    /// The predicate of the cannot whose `arg` is given.
    pub fn add_forbid(&mut self, arg: u32, f: Forbids) {
        self.forbids.push((arg, f));
    }

    fn of(&self, id: AbilityId) -> Option<&AbilityImpl> {
        self.abilities.iter().find(|a| a.id == id)
    }
}

/// The resolver's `Host` under the real rules: dispatches on the ability id, asks through the tape.
pub struct RulesHost<'a> {
    pub state: &'a mut CoreState,
    pub kernel: &'a Kernel,
    pub table: &'a AbilityTable,
    pub tape: &'a mut Tape,
}

impl Host for RulesHost<'_> {
    fn state(&self) -> &CoreState {
        self.state
    }

    fn condition(&mut self, ability: &AbilityDef, event: &EventRec, _: &Resolver) -> bool {
        match self.table.of(ability.id).and_then(|a| a.condition) {
            Some(c) => c(self.state, self.kernel, ability, event),
            None => true,
        }
    }

    fn forbids(&mut self, cannot: &CannotDef, event: &EventRec) -> bool {
        self.table.forbids.iter().find(|(arg, _)| *arg == cannot.arg).is_some_and(|(_, f)| f(self.state, cannot, event))
    }

    fn effect(&mut self, ability: &AbilityDef, event: &mut EventRec, _: &mut Resolver) -> Result<(), TimingError> {
        match self.table.of(ability.id) {
            Some(a) => (a.effect)(self.state, self.kernel, ability, event),
            None => Ok(()),
        }
    }

    fn decide(&mut self, choice: &Choice, _: &EventRec, _: &[u64]) -> Pick {
        self.tape.pick(self.state, choice)
    }
}

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
        let mut host = RulesHost { state: &mut *self.state, kernel: self.kernel, table: self.table, tape: &mut *self.tape };
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
