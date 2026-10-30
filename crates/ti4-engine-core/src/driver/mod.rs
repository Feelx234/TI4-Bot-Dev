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

pub mod aftermath;
pub mod children;
pub mod menu;
pub mod session;
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

#[derive(Debug, Clone)]
enum Op {
    Step,
    Answer(Pick),
}

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
            if let Some(why) = self.tape.take_unknown() {
                self.unknown.get_or_insert(why);
            }
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
                    self.facts = None;
                    self.facts_state = None;
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
}
