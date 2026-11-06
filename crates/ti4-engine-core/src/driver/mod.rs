//! The driver (layer L5; package D1): runs one [`Window`] to its next question and resumes it with an
//! answer (docs/ENGINE_CORE_DRIVER.md).
//!
//! * [`Driver::run`]: step the window until it asks ([`Status::Asking`]) or ends ([`Status::Done`]).
//! * [`Driver::resume`]: answer the question reported last, then step again.
//!
//! The driver owns what is not game state: the A5 [`Resolver`] (windows, frequency scopes, the journal), the
//! [`AbilityTable`] behind it, the trace-id counter, the events the window emitted, and the windows it
//! asked the driver to open. Everything the rules decide is in `(window, CoreState)`, and both clone
//! cheaply, so the nested-question replay (see `kernel::stage`) is a snapshot and a re-run.
//!
//! Deterministic by construction: no clock, no global, no hash iteration; randomness is the state's
//! `rng` streams, which a rolled-back run restores with the rest of the state.

use crate::choice::{Choice, Pick};
use crate::kernel::ability::Unknown;
use crate::kernel::stage::{AbilityTable, Illegal, NestedFacts, Sx, Tape, Window};
use crate::kernel::{Kernel, Next, Resolver, TimingError};
use crate::state::{CoreState, EventRec, RngDomain, SeedSource, Window as OpenWindow};

pub mod agenda;
pub mod aftermath;
pub mod children;
pub mod dispatch;
pub mod kernel_env;
pub mod live;
pub mod menu;
pub mod menu_cards;
pub mod menu_faction;
pub mod menu_sources;
pub mod probe;
pub mod session;
pub mod step;
pub mod tactical_tail;
pub mod turn;
pub mod turn_flow;
pub mod windows;

/// Where a run stopped.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant, reason = "one value per step, never stored in bulk; boxing the Choice would cost an allocation per question")]
pub enum Status {
    /// The question to put to `choice.actor`. Answer it with [`Driver::resume`].
    Asking(Choice),
    /// The window is finished.
    Done,
}

/// How a window broke the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Breach {
    /// `Next::Continue` while `ask_of` is `None`.
    ContinueWithoutQuestion,
    /// `Next::Continue(ask)` with an ask other than `ask_of`'s.
    AskMismatch,
}

/// Why a run or a resume failed.
#[derive(Debug, Clone, PartialEq)]
pub enum DriverError {
    /// The pick is not an option of the question.
    Illegal,
    /// Nothing is being asked (`resume` without a question, or after `Done`).
    NotAsking,
    /// The window asked for a question it cannot describe (`ask_of` is `None` or differs).
    Contract(Breach),
    /// The resolver refused.
    Timing(TimingError),
}

impl From<TimingError> for DriverError {
    fn from(e: TimingError) -> Self {
        Self::Timing(e)
    }
}

impl From<Illegal> for DriverError {
    fn from(_: Illegal) -> Self {
        Self::Illegal
    }
}

type Op = crate::state::NestedOp;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant, reason = "one value per step, never stored in bulk; boxing the Choice would cost an allocation per question")]
enum Awaiting {
    Nothing,
    /// The window's own question (a stage).
    Stage(Choice),
    /// A question from inside an emission: answer it and re-run `op`.
    Nested(Op),
}

/// One running window.
pub struct Driver<W: Window> {
    pub window: W,
    pub resolver: Resolver,
    pub table: AbilityTable,
    /// The next trace id.
    pub next_id: u64,
    /// The events the window emitted itself, in order.
    pub emitted: Vec<EventRec>,
    /// The windows the rule areas asked the driver to open (`Next::Open`), in order.
    pub opened: Vec<OpenWindow>,
    /// The seeds of the random streams a window draws from (`Sx::die`); `NoSeeds` until the adapter gives some.
    pub seeds: std::rc::Rc<dyn SeedSource>,
    /// The adapter gave the driver seeds (`with_seeds`): it draws from the streams of the game.
    seeded: bool,
    tape: Tape,
    facts: Option<NestedFacts>,
    facts_state: Option<CoreState>,
    unknown: Option<Unknown>,
    awaiting: Awaiting,
    finished: bool,
    /// The continuation of the last dry run, when a converted path stopped it ([`Park`]).
    park: Option<std::sync::Arc<Park>>,
}

// -- the step machine of the converted paths (stepmachine-1, `docs/STEPMACHINE1_RESUME.md`) ---------------------------------------

thread_local! {
    static PARKING: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    /// The parks this thread's drivers kept and the answers that resumed one (`park_stats`).
    static PARK_STATS: std::cell::Cell<(u64, u64)> = const { std::cell::Cell::new((0, 0)) };
    /// With `park_detail(true)`: the nested answers by path ([`PathKey`]).
    static PARK_DETAIL: std::cell::RefCell<Option<Vec<(PathKey, u64)>>> = const { std::cell::RefCell::new(None) };
}

/// The parks this thread's drivers kept and the answers that resumed one, since the thread began (a counter of the bench and the tests).
#[must_use]
pub fn park_stats() -> (u64, u64) {
    PARK_STATS.with(std::cell::Cell::get)
}

/// What a nested answer is counted by ([`park_detail`]): the converted path that parked it (the card, and whether it is the primary), else the
/// kind of the window whose operation was replayed; and the question's subtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathKey {
    pub parked: Option<(crate::generated::StrategyCard, bool)>,
    pub kind: crate::state::WindowKind,
    pub subtype: crate::choice::Subtype,
}

/// Count the nested answers by path from now on (`true`), or stop and forget (`false`). A bench counter.
pub fn park_detail(on: bool) {
    PARK_DETAIL.with(|d| *d.borrow_mut() = on.then(Vec::new));
}

/// The counts of [`park_detail`], in key order.
#[must_use]
pub fn park_detail_counts() -> Vec<(PathKey, u64)> {
    let mut all = PARK_DETAIL.with(|d| d.borrow().clone().unwrap_or_default());
    all.sort_unstable();
    all
}

/// Count one nested answer of `key` (see [`park_detail`]).
pub(crate) fn note_answer(key: impl FnOnce() -> PathKey) {
    PARK_DETAIL.with(|d| {
        if let Some(m) = d.borrow_mut().as_mut() {
            let key = key();
            match m.iter_mut().find(|(k, _)| *k == key) {
                Some((_, n)) => *n += 1,
                None => m.push((key, 1)),
            }
        }
    });
}

/// Whether the converted paths park their continuation at a question (on by default). `false`: every nested answer replays the operation
/// from its start, as before stepmachine-1 (the switch that lets one build compare the two).
pub fn set_parking(on: bool) {
    PARKING.with(|p| p.set(on));
}

/// See [`set_parking`].
#[must_use]
pub fn parking() -> bool {
    PARKING.with(std::cell::Cell::get)
}

/// A run of an operation stopped at a question of a converted path, as data: everything the run had written up to the question (the position,
/// the resolver, the trace counter, the events the window emitted, the windows it opened) and the path's own continuation ([`Resume`]). The
/// suspension keeps it with the remembered question (`live::Asked`); an answer in the same position resumes the operation HERE instead of
/// replaying it from its start (`Driver::resume_parked`). Plain data: a clone of the position shares it, each copy resumes on its own clones.
#[derive(Debug, Clone)]
pub struct Park {
    pub state: CoreState,
    pub resolver: Resolver,
    pub next_id: u64,
    pub emitted: Vec<EventRec>,
    /// Filled by the driver when it takes the park (the windows the run opened are the driver's, not the run's).
    pub opened: Vec<OpenWindow>,
    pub at: Resume,
}

/// Where a parked operation resumes: one variant per converted path.
#[derive(Debug, Clone)]
pub enum Resume {
    /// The child loop of a strategy card's ability (`windows::strategic::perform`).
    Perform(windows::strategic::perform::Parked),
}

impl Resume {
    /// Whether the continuation belongs to `op` of `window` (the path parked from the operation the driver ran, not from a window run inline).
    pub(crate) fn fits(&self, op: &Op, window: &dyn std::any::Any) -> bool {
        match self {
            Self::Perform(p) => p.fits(op, window),
        }
    }

    /// The path's key for a counter ([`PathKey::parked`]).
    #[must_use]
    pub fn key(&self) -> (crate::generated::StrategyCard, bool) {
        match self {
            Self::Perform(p) => p.key(),
        }
    }

    /// Take the operation up at the question with `pick`. `None`: the window is not the one the path parked in.
    fn resume(&self, sx: &mut Sx<'_>, window: &mut dyn std::any::Any, pick: Pick) -> Option<Resumed> {
        match self {
            Self::Perform(p) => p.resume(sx, window, pick),
        }
    }
}

/// How the operation ended a resumed run took up (what `Driver::once` makes of the window's op).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resumed {
    /// The op was the window's `answer`: step on (`Flow::Again`).
    Answered,
    /// The op was the window's `step`, which ended with `Next::Done`.
    Stepped,
}

/// The seeds of a driver that never draws: every stream is all zeros.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSeeds;

impl SeedSource for NoSeeds {
    fn seed_for(&self, _: u64, _: RngDomain) -> [u8; 32] {
        [0; 32]
    }
}

impl<W: Window> Driver<W> {
    #[must_use]
    pub fn new(window: W, resolver: Resolver, table: AbilityTable, next_id: u64) -> Self {
        Self {
            window,
            resolver,
            table,
            next_id,
            emitted: Vec::new(),
            opened: Vec::new(),
            seeds: std::rc::Rc::new(NoSeeds),
            seeded: false,
            tape: Tape::default(),
            facts: None,
            facts_state: None,
            unknown: None,
            awaiting: Awaiting::Nothing,
            finished: false,
            park: None,
        }
    }

    /// A driver whose resolver and trace counter are the state's own (D2-4b): the persistent part of the
    /// resolver is `state.pending.resolver`, the registry is the caller's (derived, see
    /// `AbilityTable::registry`), and the clock and the seats are set by the caller as for [`Self::new`].
    #[must_use]
    pub fn from_state(window: W, state: &CoreState, registry: crate::containers::Cow<crate::kernel::Registry>, table: AbilityTable) -> Self {
        let carried = &state.pending().resolver;
        Self::new(window, Resolver::from_state(carried, registry), table, carried.next_id)
    }

    /// Write the resolver's persistent part and the trace counter back into the state (what a driver that
    /// was made by [`Self::from_state`] owes the state when its window ends or asks).
    pub fn store(&self, state: &mut CoreState) {
        state.pending_mut().resolver = self.resolver.into_state(self.next_id);
    }

    /// The same driver drawing its dice from streams seeded by `seeds`.
    #[must_use]
    pub fn with_seeds(mut self, seeds: std::rc::Rc<dyn SeedSource>) -> Self {
        self.seeds = seeds;
        self.seeded = true;
        self
    }

    /// Step the window to its next question or its end.
    ///
    /// # Errors
    /// The resolver refused, or the window broke its contract.
    pub fn run(&mut self, s: &mut CoreState, k: &Kernel) -> Result<Status, DriverError> {
        crate::unsupported::guard(s);
        self.tape.clear();
        self.settle(s, k, Op::Step, Awaiting::Nothing)
    }

    /// Answer the question reported last and step on.
    ///
    /// # Errors
    /// [`DriverError::NotAsking`] when nothing is asked; [`DriverError::Illegal`] when `pick` is not an
    /// option (the state is untouched); otherwise as [`Self::run`].
    pub fn resume(&mut self, s: &mut CoreState, k: &Kernel, pick: Pick) -> Result<Status, DriverError> {
        match std::mem::replace(&mut self.awaiting, Awaiting::Nothing) {
            Awaiting::Nothing => Err(DriverError::NotAsking),
            Awaiting::Stage(choice) => {
                let counts = matches!(choice.ask, crate::choice::Ask::Generic(crate::choice::generic::Ask::Count { .. }));
                let legal = match pick {
                    Pick::Opt(o) => choice.opts.iter().any(|x| *x == o) || self.window.accepts_unlisted(&pick),
                    Pick::Number(_) => counts,
                };
                if !legal {
                    self.awaiting = Awaiting::Stage(choice);
                    return Err(DriverError::Illegal);
                }
                self.tape.clear();
                let prior = Awaiting::Stage(choice);
                self.settle(s, k, Op::Answer(pick), prior)
            }
            Awaiting::Nested(op) => {
                self.tape.push(pick);
                let prior = Awaiting::Nested(op.clone());
                self.settle(s, k, op, prior)
            }
        }
    }

    /// Run `op`, then (after an answer) step until a question or the end. A run that put a nested question
    /// the tape could not answer is rolled back and reported.
    fn settle(&mut self, s: &mut CoreState, k: &Kernel, first: Op, prior: Awaiting) -> Result<Status, DriverError> {
        let mut op = first;
        let mut prior = Some(prior);
        loop {
            let snapshot = (s.clone(), self.window.clone(), self.resolver.checkpoint(), self.emitted.len(), self.opened.len(), self.next_id);
            #[cfg(feature = "probe-counters")]
            let census_mark = crate::state::census::mark();
            self.tape.rewind();
            let outcome = self.once(s, k, &op);
            if let Some(why) = self.tape.take_unknown() {
                self.unknown.get_or_insert(why);
            }
            let dry = self.tape.needed().cloned();
            if W::PARKS {
                self.take_park(dry.is_some() && outcome.is_ok(), &op);
            } else {
                // No converted path parks in this window (`Resume::fits` refuses it): drop what a run inside it may have parked.
                self.tape.parked = crate::state::Memo::default();
                self.park = None;
            }
            if dry.is_some() || outcome.is_err() {
                // Undo the run: a nested question found the tape dry (ask it), or the run failed (the
                // question stays where it was).
                #[cfg(feature = "probe-counters")]
                crate::state::census::rolled_back(census_mark, dry.is_some());
                *s = snapshot.0;
                self.window = snapshot.1;
                self.resolver.restore(snapshot.2);
                self.emitted.truncate(snapshot.3);
                self.opened.truncate(snapshot.4);
                self.next_id = snapshot.5;
            }
            if let Some(choice) = dry {
                self.facts = self.tape.needed_facts().cloned();
                self.facts_state = self.tape.needed_state().cloned();
                self.awaiting = Awaiting::Nested(op);
                return Ok(Status::Asking(choice));
            }
            if outcome.is_err() {
                self.awaiting = prior.take().unwrap_or(Awaiting::Nothing);
            }
            match outcome? {
                Flow::Ask(choice) => {
                    probe::seen(&self.window);
                    self.facts = None;
                    self.facts_state = None;
                    self.awaiting = Awaiting::Stage(choice.clone());
                    return Ok(Status::Asking(choice));
                }
                Flow::Done => {
                    probe::seen(&self.window);
                    self.finished = true;
                    self.awaiting = Awaiting::Nothing;
                    return Ok(Status::Done);
                }
                Flow::Again => {
                    self.tape.clear();
                    op = Op::Step;
                }
            }
        }
    }

    /// After a run: keep the continuation a converted path parked at the question that found the tape dry (`keep`: the run was dry and did
    /// not fail), with the windows the run opened. The path must have parked from the op it belongs to (`Resume::fits`), else nothing is kept.
    fn take_park(&mut self, keep: bool, op: &Op) {
        let parked = std::mem::take(&mut self.tape.parked);
        self.park = None;
        if !keep || !parking() {
            return;
        }
        let Some(park) = parked.shared::<Park>() else { return };
        if !park.at.fits(op, &self.window as &dyn std::any::Any) {
            return;
        }
        let mut park = std::sync::Arc::unwrap_or_clone(park);
        park.opened = self.opened.clone();
        self.park = Some(std::sync::Arc::new(park));
        PARK_STATS.with(|n| n.set((n.get().0 + 1, n.get().1)));
    }

    /// The continuation the last dry run parked, if a converted path stopped it.
    #[must_use]
    pub fn park(&self) -> Option<&std::sync::Arc<Park>> {
        self.park.as_ref()
    }

    /// [`Self::resume`] of a restored nested question (`restore_nested`) whose run parked at it: the run takes up the operation at the
    /// question with `pick` (everything before the question is `park`'s), instead of replaying it from its start. On a dry question or an
    /// error it is rolled back to the position of the call, exactly as a replay is.
    ///
    /// `as_called` writes into the parked position what the call that parked it wrote at its END (the window's payload, the resolver's
    /// persistent part: a replay starts from the position that call left, the parked run from the one it began with), from the position of
    /// this call.
    ///
    /// # Errors
    /// As [`Self::resume`] (the pick is not judged: the caller checked it against the question).
    pub fn resume_parked(&mut self, s: &mut CoreState, k: &Kernel, park: &Park, pick: Pick, as_called: impl FnOnce(&CoreState, &mut CoreState)) -> Result<Status, DriverError> {
        let Awaiting::Nested(op) = std::mem::replace(&mut self.awaiting, Awaiting::Nothing) else { return Err(DriverError::NotAsking) };
        self.tape.push(pick);
        let answers = self.tape.answers().to_vec();
        let snapshot = (s.clone(), self.window.clone(), self.emitted.len(), self.opened.len(), self.next_id);
        PARK_STATS.with(|n| n.set((n.get().0, n.get().1 + 1)));
        probe::seen(&self.window);
        #[cfg(test)]
        RUNS.with(|n| n.set(n.get() + 1));
        let mut at = park.state.clone();
        as_called(s, &mut at);
        *s = at;
        let resolver = std::mem::replace(&mut self.resolver, park.resolver.clone());
        self.next_id = park.next_id;
        self.emitted.clone_from(&park.emitted);
        self.opened.clone_from(&park.opened);
        self.tape.set_consumed(&answers);
        let outcome = {
            let mut sx = Sx {
                state: s,
                kernel: k,
                resolver: &mut self.resolver,
                table: &self.table,
                tape: &mut self.tape,
                next_id: &mut self.next_id,
                emitted: &mut self.emitted,
                seeds: &*self.seeds,
            };
            park.at.resume(&mut sx, &mut self.window as &mut dyn std::any::Any, pick)
        };
        if let Some(why) = self.tape.take_unknown() {
            self.unknown.get_or_insert(why);
        }
        let dry = self.tape.needed().cloned();
        let failed = outcome.is_none();
        self.take_park(dry.is_some() && !failed, &op);
        if dry.is_some() || failed {
            *s = snapshot.0;
            self.window = snapshot.1;
            self.resolver = resolver;
            self.emitted.truncate(snapshot.2);
            self.opened.truncate(snapshot.3);
            self.next_id = snapshot.4;
        }
        if let Some(choice) = dry {
            self.facts = self.tape.needed_facts().cloned();
            self.facts_state = self.tape.needed_state().cloned();
            self.awaiting = Awaiting::Nested(op);
            return Ok(Status::Asking(choice));
        }
        match outcome {
            // (The window the path parked in is not the one of this driver: the caller checked the kind; a replay would not get here.)
            None => {
                self.tape.set_answers(&answers[..answers.len() - 1]);
                self.awaiting = Awaiting::Nested(op);
                Err(DriverError::Contract(Breach::ContinueWithoutQuestion))
            }
            Some(Resumed::Stepped) => {
                probe::seen(&self.window);
                self.finished = true;
                self.awaiting = Awaiting::Nothing;
                Ok(Status::Done)
            }
            Some(Resumed::Answered) => {
                self.tape.clear();
                self.settle(s, k, Op::Step, Awaiting::Nothing)
            }
        }
    }

    fn once(&mut self, s: &mut CoreState, k: &Kernel, op: &Op) -> Result<Flow, DriverError> {
        probe::seen(&self.window);
        #[cfg(test)]
        RUNS.with(|n| n.set(n.get() + 1));
        let mut sx = Sx {
            state: s,
            kernel: k,
            resolver: &mut self.resolver,
            table: &self.table,
            tape: &mut self.tape,
            next_id: &mut self.next_id,
            emitted: &mut self.emitted,
            seeds: &*self.seeds,
        };
        let next = match op {
            Op::Step => self.window.step(&mut sx),
            Op::Answer(pick) => {
                self.window.answer(&mut sx, *pick)?;
                return Ok(Flow::Again);
            }
        };
        let mut flow = Flow::Done;
        for n in next.flatten() {
            match n {
                Next::Done | Next::Chain(_) => {}
                Next::Open(w) => self.opened.push(w),
                Next::Emit(rec) => {
                    sx.emit(rec)?;
                }
                Next::Continue(ask) => {
                    let choice = self.window.ask_of(sx.state, sx.kernel).ok_or(DriverError::Contract(Breach::ContinueWithoutQuestion))?;
                    if choice.ask != ask {
                        return Err(DriverError::Contract(Breach::AskMismatch));
                    }
                    flow = Flow::Ask(choice);
                    break;
                }
            }
        }
        Ok(flow)
    }

    // -- the live path (package M0, `driver::live`): a driver rebuilt from the state a suspended step left --------------

    /// The window of this driver is at its own question: take that question as the one reported last, so that
    /// [`Self::resume`] can answer it. `Ok(false)` when the window asks nothing in this position.
    pub fn restore_stage(&mut self, s: &CoreState, k: &Kernel) -> bool {
        match self.window.ask_of(s, k) {
            Some(choice) => {
                self.awaiting = Awaiting::Stage(choice);
                true
            }
            None => false,
        }
    }

    /// The step was suspended by a question from inside an emission: put the driver back at that question (the tape holds
    /// the answers given so far, the operation is re-run by the next [`Self::resume`] / [`Self::reask`]).
    pub fn restore_nested(&mut self, nested: &crate::state::Nested) {
        self.tape.set_answers(nested.answers.as_slice());
        self.awaiting = Awaiting::Nested(nested.op);
    }

    /// Re-run the suspended operation with the answers on the tape and report the question it stops at again (the display
    /// facts of a nested question are not part of the state: they are the product of this re-run, or of the run `driver::live`
    /// remembers them from). [`Self::resume`] does not need it: a restored driver resumes as one that re-ran (its rollback is whole).
    ///
    /// # Errors
    /// [`DriverError::NotAsking`] when no nested question was restored; otherwise as [`Self::run`].
    pub fn reask(&mut self, s: &mut CoreState, k: &Kernel) -> Result<Status, DriverError> {
        let Awaiting::Nested(op) = std::mem::replace(&mut self.awaiting, Awaiting::Nothing) else { return Err(DriverError::NotAsking) };
        let prior = Awaiting::Nested(op);
        self.settle(s, k, op, prior)
    }

    /// The suspension to store when the driver stopped at a question from inside an emission (`None`: the question is a stage
    /// of the window).
    #[must_use]
    pub fn nested_journal(&self) -> Option<crate::state::Nested> {
        match &self.awaiting {
            Awaiting::Nested(op) => Some(crate::state::Nested { op: *op, answers: crate::containers::SmallList::from_slice(self.tape.answers()), asked: crate::state::Memo::default() }),
            _ => None,
        }
    }

    /// A condition of the ability table the core could not answer (a documented bail of the rule it reads); the
    /// run is not to be trusted.
    #[must_use]
    pub fn unknown(&self) -> Option<Unknown> {
        self.unknown
    }

    /// What the nested question reported last was asked about (the event of its window, the relation, the
    /// emission chain): `None` for a question that is a stage of the window.
    #[must_use]
    pub fn nested_facts(&self) -> Option<&NestedFacts> {
        self.facts.as_ref()
    }

    /// Whether the driver draws from the streams of the game (`with_seeds`); without, its dice are all zeros.
    #[must_use]
    pub fn is_seeded(&self) -> bool {
        self.seeded
    }

    /// The state a nested question was put in (`nested_facts` says whether the last question was one).
    #[must_use]
    pub fn nested_state(&self) -> Option<&CoreState> {
        self.facts_state.as_ref()
    }

    /// Whether the window ended.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.finished
    }
}

#[cfg(test)]
thread_local! {
    /// The operations this thread's drivers ran ([`Driver::once`]): the tests of `driver::live` count the runs a nested answer costs.
    pub(crate) static RUNS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

#[allow(clippy::large_enum_variant, reason = "one value per step, never stored in bulk; boxing the Choice would cost an allocation per question")]
enum Flow {
    Ask(Choice),
    Done,
    /// An answer was applied: step on.
    Again,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::reaction::{AbilityId, AbilityKind, Ask as RAsk, Opt as ROpt, Relation};
    use crate::choice::{Ask, Opt};
    use crate::generated::Faction;
    use crate::ids::Seat;
    use crate::kernel::fx;
    use crate::kernel::resolver::AbilityDef;
    use crate::kernel::stage::{Slot, Spec, Sx};
    use crate::state::{Code, EventKind, Payload, Player, Players};

    /// A window that emits one event and ends: the shape of every `ctx.emit` in the string windows.
    #[derive(Clone)]
    struct Announce {
        done: bool,
    }

    impl Window for Announce {
        type Stage = bool;
        fn stage(&self) -> bool {
            self.done
        }
        fn step(&mut self, sx: &mut Sx<'_>) -> Next {
            if !self.done {
                self.done = true;
                if sx.emit(fx::event(EventKind::TradeGoodsGained, Payload::new())).is_err() {
                    return Next::Done;
                }
            }
            Next::Done
        }
        fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
            None
        }
        fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
            Err(Illegal)
        }
    }

    fn ability() -> AbilityId {
        AbilityId { kind: AbilityKind::Ability, key: Code(1) }
    }

    fn setup() -> (CoreState, Kernel, Driver<Announce>) {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::from_index(0).unwrap())]);
        let mut resolver = Resolver::new(&[Seat(0)], Some(Seat(0)));
        resolver.register([
            AbilityDef::new(ability(), Seat(0), EventKind::TradeGoodsGained, Relation::After).with_optional(true).with_arg(1),
            AbilityDef::new(AbilityId { kind: AbilityKind::Ability, key: Code(2) }, Seat(0), EventKind::TradeGoodsGained, Relation::After)
                .with_optional(true),
        ]);
        let mut table = AbilityTable::default();
        // The effect gains a trade good: a replayed step that applied it twice would show.
        table.add(
            Spec::new(Slot(1), AbilityKind::Ability, EventKind::TradeGoodsGained, Relation::After, |eff| {
                fx::gain_trade_goods(eff.state(), Seat(0), 1);
                Ok(())
            })
            .optional(),
        );
        (s, Kernel::plain(), Driver::new(Announce { done: false }, resolver, table, 10))
    }

    #[test]
    fn an_emission_asks_inside_the_step_and_the_replay_applies_it_once() {
        let (mut s, k, mut d) = setup();
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("the reaction window asks") };
        assert_eq!(c.ask, Ask::Reaction(RAsk::Window { event: EventKind::TradeGoodsGained, relation: Relation::After }));
        assert_eq!(c.opts.len(), 3, "two abilities and decline");
        assert!(d.emitted.is_empty() && !d.window.done, "the run was rolled back");
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 0);
        // Pick the first ability; the second window question (the other ability, still eligible) follows.
        let st = d.resume(&mut s, &k, Pick::Opt(Opt::Reaction(ROpt::Ability(ability())))).unwrap();
        let Status::Asking(c2) = st else { panic!("the second ability may still be used") };
        assert!(!c2.opts.iter().any(|o| *o == Opt::Reaction(ROpt::Ability(ability()))), "already resolved in this window");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap(), Status::Done);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 1, "the effect ran once");
        assert_eq!(d.emitted.len(), 1);
        assert_eq!(d.emitted[0].id, 10, "trace ids come from the driver");
        assert_eq!(d.next_id, 11);
    }

    #[test]
    fn a_driver_made_from_the_state_continues_the_trace_ids_and_the_spent_keys() {
        let (mut s, k, mut d) = setup();
        d.run(&mut s, &k).unwrap();
        d.resume(&mut s, &k, Pick::Opt(Opt::Reaction(ROpt::Ability(ability())))).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap(), Status::Done);
        assert!(!d.resolver.used_keys().is_empty(), "the window spent a once-per-trigger key");
        assert_eq!(s.pending().resolver.next_id, 1, "nothing is stored before the driver says so");
        d.store(&mut s);
        assert_eq!(s.pending().resolver.next_id, 11);
        assert_eq!(s.pending().resolver.initiative.as_slice(), &[Seat(0)]);
        // the next window of the game starts from the state alone
        let (_, _, fresh) = setup();
        let next = Driver::from_state(Announce { done: false }, &s, crate::containers::Cow::new(d.resolver.registry().clone()), fresh.table);
        assert_eq!(next.next_id, 11, "trace ids continue");
        assert_eq!(next.resolver.used_keys(), d.resolver.used_keys(), "spent keys carry over, in order");
        // a checkpoint of the state carries it too
        assert_eq!(s.clone().pending().resolver, s.pending().resolver);
    }

    #[test]
    fn the_same_answers_give_the_same_state() {
        let run = || {
            let (mut s, k, mut d) = setup();
            d.run(&mut s, &k).unwrap();
            d.resume(&mut s, &k, Pick::Opt(Opt::Reaction(ROpt::Ability(ability())))).unwrap();
            d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap();
            (s, d.emitted.clone(), d.resolver.log().to_vec())
        };
        let (a, b) = (run(), run());
        assert!(a.0 == b.0 && a.1 == b.1 && a.2 == b.2);
    }

    /// An ability that asks a question of its own and emits a nested event: the question comes out with the facts of its
    /// window (the adapter's trigger), the nested emission opens its own windows, and the effect runs once.
    #[test]
    fn an_effect_asks_with_the_facts_of_its_window_and_emits_a_nested_event() {
        let ask_choice = || {
            let mut opts = crate::choice::Opts::default();
            opts.push(Opt::Done);
            opts.push(Opt::Decline);
            Choice {
                actor: Seat(0),
                ask: Ask::Generic(crate::choice::generic::Ask::Confirm),
                ctx: crate::choice::Ctx {
                    origin: crate::choice::Origin::Reaction(EventKind::TradeGoodsGained),
                    subtype: crate::choice::Subtype::Generic(crate::choice::generic::Subtype::Unclassified),
                    optional: true,
                    target: None,
                    outstanding: Default::default(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts,
            }
        };
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::from_index(0).unwrap())]);
        let mut resolver = Resolver::new(&[Seat(0)], Some(Seat(0)));
        let id = AbilityId { kind: AbilityKind::Ability, key: Code(5) };
        let inner = AbilityId { kind: AbilityKind::Ability, key: Code(6) };
        resolver.register([
            AbilityDef::new(id, Seat(0), EventKind::TradeGoodsGained, Relation::After).with_arg(5),
            AbilityDef::new(inner, Seat(0), EventKind::PlanetControlGained, Relation::After).with_arg(6),
        ]);
        let mut table = AbilityTable::default();
        table.add(Spec::new(Slot(5), AbilityKind::Ability, EventKind::TradeGoodsGained, Relation::After, |eff| {
            let c = {
                let mut opts = crate::choice::Opts::default();
                opts.push(Opt::Done);
                Choice {
                    actor: eff.owner(),
                    ask: Ask::Generic(crate::choice::generic::Ask::Confirm),
                    ctx: crate::choice::Ctx {
                        origin: crate::choice::Origin::Reaction(EventKind::TradeGoodsGained),
                        subtype: crate::choice::Subtype::Generic(crate::choice::generic::Subtype::Unclassified),
                        optional: false,
                        target: None,
                        outstanding: Default::default(),
                        space_battle: false,
                        invasion_seq: None,
                    },
                    opts,
                }
            };
            if eff.ask(&c) == Pick::Opt(Opt::Done) {
                fx::gain_trade_goods(eff.state(), Seat(0), 3);
                eff.emit(fx::event(EventKind::PlanetControlGained, Payload::new()))?;
            }
            Ok(())
        }));
        table.add(Spec::new(Slot(6), AbilityKind::Ability, EventKind::PlanetControlGained, Relation::After, |eff| {
            fx::gain_trade_goods(eff.state(), Seat(0), 10);
            Ok(())
        }));
        let _ = ask_choice;
        let k = Kernel::plain();
        let mut d = Driver::new(Announce { done: false }, resolver, table, 10);
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("the ability asks") };
        assert_eq!(c.ask, Ask::Generic(crate::choice::generic::Ask::Confirm));
        let facts = d.nested_facts().expect("a nested question carries the facts of its window");
        assert_eq!((facts.event.kind, facts.event.id, facts.relation), (EventKind::TradeGoodsGained, 10, Relation::After));
        assert!(facts.chain.is_empty());
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 0, "rolled back");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Done)).unwrap(), Status::Done);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.trade_goods, 13, "the effect and the nested one, once");
        assert_eq!(d.emitted.len(), 1, "only the window's own emission is the window's");
        assert_eq!(d.next_id, 12, "the nested event took the next trace id");
    }

    /// A window that draws three dice and then emits (so the driver may roll the step back).
    #[derive(Clone)]
    struct Roller {
        done: bool,
        faces: Vec<u32>,
    }

    impl Window for Roller {
        type Stage = bool;
        fn stage(&self) -> bool {
            self.done
        }
        fn step(&mut self, sx: &mut Sx<'_>) -> Next {
            if !self.done {
                self.done = true;
                self.faces = (0..3).map(|_| sx.die(RngDomain::Dice, 10)).collect();
                let _ = sx.emit(fx::event(EventKind::TradeGoodsGained, Payload::new()));
            }
            Next::Done
        }
        fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
            None
        }
        fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
            Err(Illegal)
        }
    }

    struct Plain;
    impl SeedSource for Plain {
        fn seed_for(&self, seed: u64, _: RngDomain) -> [u8; 32] {
            let mut out = [0u8; 32];
            out[..8].copy_from_slice(&seed.to_le_bytes());
            out
        }
    }

    #[test]
    fn dice_drawn_in_a_step_that_is_rolled_back_are_drawn_once() {
        let (mut s, k, d) = setup();
        s.rng = crate::state::Rng::new(5);
        let mut d = Driver::new(Roller { done: false, faces: Vec::new() }, d.resolver, d.table, 10).with_seeds(std::rc::Rc::new(Plain));
        // The reaction window asks inside the emission: the step is rolled back, the dice with it.
        assert!(matches!(d.run(&mut s, &k).unwrap(), Status::Asking(_)));
        d.resume(&mut s, &k, Pick::Opt(Opt::Reaction(ROpt::Ability(ability())))).unwrap();
        d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap();
        let mut bare = crate::state::Rng::new(5);
        let expect: Vec<u32> = (0..3).map(|_| bare.die(RngDomain::Dice, &Plain, 10)).collect();
        assert_eq!(d.window.faces, expect);
        assert!(s.rng.positions().eq(bare.positions()), "three draws were taken, not six");
    }

    /// A position that uses a switched-off feature is refused at the driver's entry under `Policy::Panic`
    /// (docs/DISABLE_XF123.md; package D2-2) and runs as before under `Off`.
    #[test]
    fn the_driver_refuses_a_switched_off_position_at_its_entry() {
        use crate::unsupported::{Policy, with_policy};
        let (mut s, k, mut d) = setup();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::LastBastion)]);
        with_policy(Policy::Off, || {
            let _ = d.run(&mut s.clone(), &k);
        });
        let (mut s2, k2, mut d2) = setup();
        s2.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheCrimsonRebellion)]);
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_policy(Policy::Panic, || {
                let _ = d2.run(&mut s2, &k2);
            });
        }));
        std::panic::set_hook(prev);
        let text = r.err().and_then(|e| e.downcast_ref::<String>().cloned()).unwrap_or_default();
        assert!(text.contains("Crimson"), "{text}");
        let _ = &mut s;
    }
}
