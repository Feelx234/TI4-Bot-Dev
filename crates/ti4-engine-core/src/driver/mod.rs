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
use crate::kernel::stage::{AbilityTable, Illegal, Sx, Tape, Window};
use crate::kernel::{Kernel, Next, Resolver, TimingError};
use crate::state::{CoreState, EventRec, Window as OpenWindow};

pub mod windows;

/// Where a run stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// The question to put to `choice.actor`. Answer it with [`Driver::resume`].
    Asking(Choice),
    /// The window is finished.
    Done,
}

/// Why a run or a resume failed.
#[derive(Debug, Clone, PartialEq)]
pub enum DriverError {
    /// The pick is not an option of the question.
    Illegal,
    /// Nothing is being asked (`resume` without a question, or after `Done`).
    NotAsking,
    /// The window asked for a question it cannot describe (`ask_of` is `None` or differs).
    Contract(&'static str),
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

#[derive(Debug, Clone)]
enum Op {
    Step,
    Answer(Pick),
}

#[derive(Debug, Clone)]
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
    tape: Tape,
    awaiting: Awaiting,
    finished: bool,
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
            tape: Tape::default(),
            awaiting: Awaiting::Nothing,
            finished: false,
        }
    }

    /// Step the window to its next question or its end.
    ///
    /// # Errors
    /// The resolver refused, or the window broke its contract.
    pub fn run(&mut self, s: &mut CoreState, k: &Kernel) -> Result<Status, DriverError> {
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
                    Pick::Opt(o) => choice.opts.iter().any(|x| *x == o),
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
            self.tape.rewind();
            let outcome = self.once(s, k, &op);
            let dry = self.tape.needed().cloned();
            if dry.is_some() || outcome.is_err() {
                // Undo the run: a nested question found the tape dry (ask it), or the run failed (the
                // question stays where it was).
                *s = snapshot.0;
                self.window = snapshot.1;
                self.resolver.restore(snapshot.2);
                self.emitted.truncate(snapshot.3);
                self.opened.truncate(snapshot.4);
                self.next_id = snapshot.5;
            }
            if let Some(choice) = dry {
                self.awaiting = Awaiting::Nested(op);
                return Ok(Status::Asking(choice));
            }
            if outcome.is_err() {
                self.awaiting = prior.take().unwrap_or(Awaiting::Nothing);
            }
            match outcome? {
                Flow::Ask(choice) => {
                    self.awaiting = Awaiting::Stage(choice.clone());
                    return Ok(Status::Asking(choice));
                }
                Flow::Done => {
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

    fn once(&mut self, s: &mut CoreState, k: &Kernel, op: &Op) -> Result<Flow, DriverError> {
        let mut sx = Sx {
            state: s,
            kernel: k,
            resolver: &mut self.resolver,
            table: &self.table,
            tape: &mut self.tape,
            next_id: &mut self.next_id,
            emitted: &mut self.emitted,
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
                    let choice = self.window.ask_of(sx.state, sx.kernel).ok_or(DriverError::Contract("Continue without a question"))?;
                    if choice.ask != ask {
                        return Err(DriverError::Contract("Continue(ask) differs from ask_of"));
                    }
                    flow = Flow::Ask(choice);
                    break;
                }
            }
        }
        Ok(flow)
    }

    /// Whether the window ended.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.finished
    }
}

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
    use crate::kernel::stage::{AbilityImpl, Sx};
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
            AbilityDef::new(ability(), Seat(0), EventKind::TradeGoodsGained, Relation::After).with_optional(true),
            AbilityDef::new(AbilityId { kind: AbilityKind::Ability, key: Code(2) }, Seat(0), EventKind::TradeGoodsGained, Relation::After)
                .with_optional(true),
        ]);
        let mut table = AbilityTable::default();
        // The effect gains a trade good: a replayed step that applied it twice would show.
        table.add(AbilityImpl {
            id: ability(),
            condition: None,
            effect: |s, _, _, _| {
                fx::gain_trade_goods(s, Seat(0), 1);
                Ok(())
            },
        });
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
}
