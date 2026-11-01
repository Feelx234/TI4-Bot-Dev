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
use crate::state::{CoreState, EventRec, PlainRec};

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

    /// An answer the question accepts although it is not among its options (a question that takes free text: the
    /// transaction window's free-form deal, package W15). The driver lets such a pick through to [`Self::answer`].
    fn accepts_unlisted(&self, _pick: &Pick) -> bool {
        false
    }
}

/// What a window adds to [`Window`] to be HELD on the `Pending` stack between steps (package M0, `driver::live`): the machine is
/// rebuilt from `(payload, position)` at every call and written back when it asks; nothing else holds it.
pub trait Held: Window {
    /// The kind of the stack entry this machine is the payload of.
    const KIND: crate::state::WindowKind;

    /// The machine as the payload and the position make it (`None`: no window of this kind is open). Pure, and a function of
    /// `(core, payload)` only: a payload field exists for what the position cannot re-derive.
    fn from_pending(core: &CoreState, k: &Kernel) -> Option<Self>;

    /// Write the payload of the machine to the stack (`Pending::open(Window::..)`), replacing the entry of its kind.
    fn to_pending(&self, core: &mut CoreState);

    /// The window ended: the closing writes of the composer, before its entry leaves the stack. Default: none (a window's last
    /// `step` normally does them).
    fn closed(&self, _core: &mut CoreState, _k: &Kernel) {}
}

pub use super::ability::{Runner, AbilityTable, Applies, CondCx, Codes, Condition, Eff, Effect, Forbids, NestedFacts, RulesHost, Slot, Spec, Tape};

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
        let top = self.resolver.emission_stack().len() == self.resolver.base_depth();
        let mut host = RulesHost { state: &mut *self.state, kernel: self.kernel, table: self.table, tape: &mut *self.tape, next_id: &mut *self.next_id, seeds: self.seeds };
        let done = self.resolver.emit(&mut host, rec, resolve)?;
        if top {
            self.emitted.push(done.clone());
        }
        Ok(done)
    }

    /// A plain event of the string's `Game::emit`, appended to the ordered emission queue at once (package P1).
    pub fn plain(&mut self, rec: PlainRec) {
        super::fx::plain(&mut *self.state, rec);
    }

    /// [`Self::emit`] followed by the mirror of everything the resolver emitted meanwhile into the emission queue: the
    /// string's `emit_typed` (`let logged = timing.log().len(); emit; mirror_timing_log(logged)`). Nothing is mirrored
    /// when the emission fails, as in the string, where the error returns before the mirror.
    ///
    /// # Errors
    /// What the resolver refuses with.
    pub fn emit_mirrored(&mut self, rec: EventRec) -> Result<EventRec, TimingError> {
        let top = self.resolver.emission_stack().len() == self.resolver.base_depth();
        let mark = super::fx::journal_mark(self.resolver);
        let done = self.emit(rec)?;
        super::fx::mirror_journal(&mut *self.state, &*self.resolver, mark);
        if top {
            // `Live::emit_typed`: the card moves a faction effect staged in that event's windows are announced now, not at the next component action.
            super::fx_cards::announce_staged_cards(self)?;
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

impl super::ability::Runner for Sx<'_> {
    fn state(&mut self) -> &mut CoreState {
        self.state
    }

    fn read(&self) -> (&CoreState, &Kernel) {
        (&*self.state, self.kernel)
    }

    fn with_kernel<T>(&mut self, f: impl FnOnce(&mut CoreState, &Kernel) -> T) -> T {
        f(&mut *self.state, self.kernel)
    }

    fn facts(&self) -> &super::ability::AdapterFacts {
        &self.table.facts
    }

    fn emit(&mut self, rec: EventRec) -> Result<EventRec, TimingError> {
        Sx::emit(self, rec)
    }

    fn ask(&mut self, choice: &Choice) -> Pick {
        Sx::ask(self, choice)
    }

    fn unknown(&mut self, why: super::ability::Unknown) {
        if self.tape.unknown.get().is_none() {
            self.tape.unknown.set(Some(why));
        }
    }

    fn with_seeds<T>(&mut self, f: impl FnOnce(&mut CoreState, &dyn SeedSource) -> T) -> T {
        f(&mut *self.state, self.seeds)
    }

    fn with_sx<T>(&mut self, f: impl FnOnce(&mut Sx<'_>) -> T) -> T {
        f(self)
    }

    fn has_registered_play(&self, card: crate::generated::ActionCard) -> bool {
        self.table.card_play(card).is_some()
    }

    fn run_registered_play(&mut self, who: crate::ids::Seat, card: crate::generated::ActionCard) -> Result<(), TimingError> {
        let Some(play) = self.table.card_play(card).copied() else { return Ok(()) };
        self.tape.emitted.clear();
        let mut host = RulesHost { state: &mut *self.state, kernel: self.kernel, table: self.table, tape: &mut *self.tape, next_id: &mut *self.next_id, seeds: self.seeds };
        let done = (play.effect)(&mut host, &mut *self.resolver, who, card);
        self.emitted.append(&mut self.tape.emitted);
        done
    }

    fn after_play(&mut self) -> Result<(), TimingError> {
        let Some(after) = self.table.after_play else { return Ok(()) };
        self.tape.emitted.clear();
        let mut host = RulesHost { state: &mut *self.state, kernel: self.kernel, table: self.table, tape: &mut *self.tape, next_id: &mut *self.next_id, seeds: self.seeds };
        let done = after(&mut host, &mut *self.resolver);
        self.emitted.append(&mut self.tape.emitted);
        done
    }
}

// -- Child windows (package B6d) -------------------------------------------------------------------------

/// What a [`Child`] did when it was stepped.
#[derive(Debug)]
pub enum Ran<W> {
    /// The child waits on a question (the ask of [`Window::ask_of`]).
    Asks(crate::choice::Ask),
    /// The child asks the driver for something else (`Next::Open`, `Next::Emit`, `Next::Chain`): the parent returns it as its
    /// own step and steps the child again.
    Pass(Next),
    /// The child finished; the window comes back so the parent can read what it did (a payment's `took`, a research's
    /// `researched`).
    Done(W),
}

/// A window run inside another window's step: the question-driven payment or research a card effect contains.
///
/// The parent holds the child as a field (the state stays `(window, state)`, so a checkpoint is a clone) and delegates:
/// `step` until it returns [`Ran::Done`], `ask_of` while it runs, `answer` for a pick the child's question got. A child
/// that finishes during `answer` is finished by the next `step` (the contract of [`Window::step`]: a window that is done
/// answers [`Next::Done`]).
#[derive(Debug, Clone)]
pub struct Child<W: Window> {
    window: Option<W>,
}

impl<W: Window> Default for Child<W> {
    fn default() -> Self {
        Self { window: None }
    }
}

impl<W: Window> Child<W> {
    /// No child.
    #[must_use]
    pub const fn idle() -> Self {
        Self { window: None }
    }

    /// Start `window` as the child.
    pub fn start(&mut self, window: W) {
        self.window = Some(window);
    }

    /// Whether a child is running.
    #[must_use]
    pub const fn running(&self) -> bool {
        self.window.is_some()
    }

    /// The running child.
    #[must_use]
    pub const fn window(&self) -> Option<&W> {
        self.window.as_ref()
    }

    /// Step the child. `None` when there is none.
    pub fn step(&mut self, sx: &mut Sx<'_>) -> Option<Ran<W>> {
        let window = self.window.as_mut()?;
        match window.step(sx) {
            Next::Continue(ask) => Some(Ran::Asks(ask)),
            Next::Done => self.window.take().map(Ran::Done),
            other => Some(Ran::Pass(other)),
        }
    }

    /// The question the child waits on.
    #[must_use]
    pub fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        self.window.as_ref()?.ask_of(s, k)
    }

    /// Give the child its answer.
    ///
    /// # Errors
    /// There is no child, or the pick is not an option of its question.
    pub fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.window.as_mut().ok_or(Illegal)?.answer(sx, pick)
    }
}

#[cfg(test)]
mod child_tests {
    use super::*;
    use crate::choice::{Ask, Opt, Origin, Subtype};
    use crate::choice::cards::{Ask as CAsk, Opt as COpt, What};
    use crate::driver::{Driver, Status};
    use crate::generated::ActionCard;
    use crate::ids::Seat;
    use crate::kernel::Resolver;

    /// A toy window that asks `left` yes/no questions of seat 0 and counts the answers it got.
    #[derive(Debug, Clone)]
    struct Asks {
        left: u8,
        got: u8,
        waiting: bool,
    }

    impl Window for Asks {
        type Stage = bool;
        fn stage(&self) -> bool {
            self.waiting
        }
        fn step(&mut self, _: &mut Sx<'_>) -> Next {
            if self.left == 0 {
                return Next::Done;
            }
            self.waiting = true;
            Next::Continue(Ask::Cards(CAsk::Pick { card: ActionCard::Spy, what: What::Accept }))
        }
        fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
            (self.waiting && self.left > 0).then(|| Choice {
                actor: Seat(0),
                ask: Ask::Cards(CAsk::Pick { card: ActionCard::Spy, what: What::Accept }),
                ctx: crate::choice::Ctx {
                    origin: Origin::ActionCard(ActionCard::Spy),
                    subtype: Subtype::Cards(crate::choice::cards::Subtype::Pick { card: ActionCard::Spy, what: What::Accept }),
                    optional: false,
                    target: None,
                    outstanding: crate::containers::SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: [Opt::Cards(COpt::Answer(true)), Opt::Cards(COpt::Answer(false))].into_iter().collect(),
            })
        }
        fn answer(&mut self, _: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
            let Pick::Opt(Opt::Cards(COpt::Answer(yes))) = pick else { return Err(Illegal) };
            self.got += u8::from(yes);
            self.left -= 1;
            self.waiting = false;
            Ok(())
        }
    }

    /// A parent that runs two `Asks` children one after the other and records what each got.
    #[derive(Debug, Clone)]
    struct Parent {
        child: Child<Asks>,
        started: u8,
        results: Vec<u8>,
    }

    impl Window for Parent {
        type Stage = u8;
        fn stage(&self) -> u8 {
            self.started
        }
        fn step(&mut self, sx: &mut Sx<'_>) -> Next {
            loop {
                if !self.child.running() {
                    if self.started == 2 {
                        return Next::Done;
                    }
                    self.started += 1;
                    self.child.start(Asks { left: 2, got: 0, waiting: false });
                }
                match self.child.step(sx) {
                    Some(Ran::Asks(ask)) => return Next::Continue(ask),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(done)) => self.results.push(done.got),
                    None => return Next::Done,
                }
            }
        }
        fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
            self.child.ask_of(s, k)
        }
        fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
            self.child.answer(sx, pick)
        }
    }

    /// The parent asks the child's questions, hands the picks to it, collects the finished child and starts the next one.
    #[test]
    fn a_child_asks_through_its_parent_and_hands_back_its_result() {
        let mut s = CoreState::all_loaded();
        let k = Kernel::plain();
        let parent = Parent { child: Child::idle(), started: 0, results: Vec::new() };
        let mut d = Driver::new(parent, Resolver::new(&[Seat(0)], Some(Seat(0))), AbilityTable::default(), 1);
        let mut status = d.run(&mut s, &k).unwrap();
        let mut asked = 0;
        // yes, no, no, yes
        let answers = [0, 1, 1, 0];
        while let Status::Asking(choice) = status {
            let pick = choice.pick(answers[asked]).unwrap();
            asked += 1;
            status = d.resume(&mut s, &k, pick).unwrap();
        }
        assert_eq!(asked, 4, "two children, two questions each");
        assert_eq!(d.window.results, vec![1, 1], "each child's own count came back");
        assert!(!d.window.child.running());
    }

    /// An idle child steps to nothing and refuses an answer.
    #[test]
    fn an_idle_child_does_nothing_and_refuses_an_answer() {
        let mut child: Child<Asks> = Child::idle();
        assert!(!child.running() && child.window().is_none());
        let mut s = CoreState::all_loaded();
        let k = Kernel::plain();
        let mut d = Driver::new(Parent { child: Child::idle(), started: 2, results: Vec::new() }, Resolver::new(&[Seat(0)], Some(Seat(0))), AbilityTable::default(), 1);
        assert!(matches!(d.run(&mut s, &k).unwrap(), Status::Done));
        let _ = &mut child;
        assert!(child.ask_of(&s, &k).is_none());
    }
}
