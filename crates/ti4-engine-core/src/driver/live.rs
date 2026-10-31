//! The live loop's core half (package M0/KF2, `docs/D2_LF_RESUME.md`): a window the core OWNS between two steps.
//!
//! A *held* window ([`Held`]) lives on the `Pending` stack as a typed, text-free payload and nowhere else. The machine object
//! (with its children) is rebuilt from `(payload, position)` at every call ([`Held::from_pending`]), run by a fresh
//! [`Driver`] ([`advance`]), and written back ([`Held::to_pending`]) when it asks; it is removed from the stack when it ends. So
//! a checkpoint is `CoreState::clone`, a fork is a clone, and nothing the string engine holds is needed.
//!
//! A question asked from INSIDE an emission (a reaction window, an ability) suspends the step with the driver's own
//! replay-from-the-start device ([`Driver::restore_nested`]); the suspension (the operation and the answers so far) is
//! `Pending::nested`, written here and nowhere else. A blocking table never needs it (the caller answers inside the same
//! `advance` loop); it is what makes a step resumable by a caller that returns between questions (D3, the oracle's pull style).
//!
//! The arm registry ([`live_arm`]) maps a `session::Arm` to the function that runs it; an arm that is not in it stays `Lend`.
//!
//! L5: reads and writes the position through the windows it runs.

use std::rc::Rc;

use crate::choice::{Choice, Pick};
use crate::containers::Cow;
use crate::driver::session::Arm;
use crate::driver::{Driver, DriverError, Status};
use crate::kernel::ability::{NestedFacts, Unknown};
use crate::kernel::stage::{AbilityTable, Window};
use crate::kernel::{Kernel, Registry, Resolver};
use crate::state::{CoreState, EventRec, SeedSource, Window as OpenWindow};

pub use crate::kernel::stage::Held;

/// What a live window needs besides the position: built once per game by the wrapper, never per step.
#[derive(Clone)]
pub struct Env<'a> {
    pub kernel: &'a Kernel,
    /// The abilities of every seat, derived from the position (`AbilityTable::registry`).
    pub registry: Cow<Registry>,
    pub table: AbilityTable,
    /// The game's random streams (`Sx::die`). A live driver is ALWAYS seeded.
    pub seeds: Rc<dyn SeedSource>,
}

/// Why a live call did not return a question or the end.
#[derive(Debug, Clone, PartialEq)]
pub enum LiveError {
    /// No window of the kind is open.
    NotOpen,
    /// The window asks nothing (or no nested question is pending) but an answer was given.
    NotAsking,
    /// The pick is not an option of the question. The position is untouched.
    Illegal,
    /// The window broke its contract, or the resolver refused.
    Driver(DriverError),
    /// An ability condition the core cannot answer (a documented bail of the rule it reads): in the product this is the bail.
    Unknown(Unknown),
}

impl From<DriverError> for LiveError {
    fn from(e: DriverError) -> Self {
        match e {
            DriverError::Illegal => Self::Illegal,
            DriverError::NotAsking => Self::NotAsking,
            other => Self::Driver(other),
        }
    }
}

/// A question for the table, with what the renderer needs of the nested question it may be.
#[derive(Debug, Clone, PartialEq)]
pub struct Asking {
    pub choice: Choice,
    /// A question asked from inside an emission: the event of its window and the position it was asked in (the driver rolled
    /// the step back; the display facts read this state).
    pub nested: Option<(NestedFacts, Box<CoreState>)>,
    /// The position the question was put in, when it is a question asked from inside a step (the driver rolled the step back; the display
    /// facts of the question read this state). `None` for a stage question of the window.
    pub at: Option<Box<CoreState>>,
}

/// The result of running a held window to its next stop.
#[derive(Debug, Clone, PartialEq)]
pub enum Live {
    Asks(Box<Asking>),
    /// The window ended and left the stack.
    Done,
}

/// What the call produced besides the stop.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Out {
    /// The events the window emitted itself, in order.
    pub emitted: Vec<EventRec>,
    /// The windows the rules asked the driver to open (`Next::Open`), in order: the arm opens them.
    pub opened: Vec<OpenWindow>,
}

/// Open `window`: write its payload. The first [`advance`] (`None`) then runs it to its first question.
pub fn open<M: Held>(core: &mut CoreState, window: &M) {
    window.to_pending(core);
}

/// Whether a window of kind `M::KIND` is open.
#[must_use]
pub fn is_open<M: Held>(core: &CoreState) -> bool {
    core.pending().window(M::KIND).is_some()
}

/// The resolver's clock is the position's own (phase, round, turn, seating order, active seat, speaker).
fn sync_clock(r: &mut Resolver, core: &CoreState) {
    let m = core.meta();
    r.set_phase(m.phase);
    r.set_active_player(m.active);
    r.set_speaker(Some(m.speaker));
    r.set_seating_order(core.seats().seating_order());
    r.set_counters(u64::from(m.round), u64::from(m.turn_seq));
}

/// The ability table of one call: the game's table with the play facts of THIS position (a card played inside the window reads what a
/// seat places, the research it opens, the Mahact pools; they change every step, so they are computed here, never kept in the `Env`).
fn table_at(core: &CoreState, env: &Env<'_>) -> AbilityTable {
    let mut table = env.table.clone();
    if let Some(provider) = &table.facts.plays_provider {
        table.facts.plays = (provider.0)(core, env.kernel);
    }
    table
}

fn driver_of<M: Held>(window: M, core: &CoreState, env: &Env<'_>) -> Driver<M> {
    let mut d = Driver::from_state(window, core, env.registry.clone(), table_at(core, env)).with_seeds(env.seeds.clone());
    sync_clock(&mut d.resolver, core);
    d
}

/// Whether `pick` answers `choice`: one of its options, or a number to a count question.
fn is_option(choice: &Choice, pick: &Pick) -> bool {
    match pick {
        Pick::Opt(o) => choice.opts.iter().any(|x| x == o),
        Pick::Number(_) => matches!(choice.ask, crate::choice::Ask::Generic(crate::choice::generic::Ask::Count { .. })),
    }
}

/// The question the open window of kind `M::KIND` waits on, if any: a pure read for a stage question, a replay of the suspended
/// step for a nested one. `Ok(None)`: it waits on nothing (fresh, or about to end): [`advance`] with `None` runs it.
///
/// # Errors
/// The window is not open, or the replay of a suspended step failed.
pub fn ask<M: Held>(core: &CoreState, env: &Env<'_>) -> Result<Option<Asking>, LiveError> {
    let window = M::from_pending(core, env.kernel).ok_or(LiveError::NotOpen)?;
    if let Some(n) = core.pending().nested.clone() {
        let mut scratch = core.clone();
        let mut d = driver_of(window, core, env);
        d.restore_nested(&n);
        return match d.reask(&mut scratch, env.kernel)? {
            Status::Asking(choice) => Ok(Some(asking_of(&d, choice))),
            Status::Done => Ok(None),
        };
    }
    Ok(window.ask_of(core, env.kernel).map(|choice| Asking { choice, nested: None, at: None }))
}

fn asking_of<M: Window>(d: &Driver<M>, choice: Choice) -> Asking {
    let at = d.nested_state().cloned().map(Box::new);
    let nested = d.nested_facts().cloned().zip(at.clone());
    Asking { choice, nested, at }
}

/// Run the open window of kind `M::KIND` to its next question or its end. With `answer`, apply it first: to the pending nested
/// question when there is one, else to the window's own. Without, run (a fresh window; or a repeat of a question).
///
/// On any error the position is exactly what it was before the call (a clone is restored: the generic rollback of a step).
///
/// # Errors
/// See [`LiveError`].
pub fn advance<M: Held>(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    let before = core.clone();
    let result = advance_inner::<M>(core, env, answer);
    if result.is_err() {
        *core = before;
    }
    result
}

fn advance_inner<M: Held>(core: &mut CoreState, env: &Env<'_>, answer: Option<Pick>) -> Result<(Live, Out), LiveError> {
    let window = M::from_pending(core, env.kernel).ok_or(LiveError::NotOpen)?;
    let mut d = driver_of(window, core, env);
    let nested = core.pending_mut().nested.take();
    let status = match (nested, answer) {
        (Some(n), Some(pick)) => {
            // The driver replays a nested answer without judging it (the dual feeds it recorded ones): a live answer comes from outside,
            // so the question is rebuilt first and the pick must be one of its options.
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
    let out = Out { emitted: std::mem::take(&mut d.emitted), opened: std::mem::take(&mut d.opened) };
    d.store(core);
    match status {
        Status::Done => {
            d.window.closed(core, env.kernel);
            core.pending_mut().close(M::KIND);
            core.pending_mut().nested = None;
            Ok((Live::Done, out))
        }
        Status::Asking(choice) => {
            d.window.to_pending(core);
            core.pending_mut().nested = d.nested_journal();
            Ok((Live::Asks(Box::new(asking_of(&d, choice))), out))
        }
    }
}

// -- the arm registry --------------------------------------------------------------------------------

/// The function that runs one arm live: `(position, env, the answer to the arm's pending question)`.
pub type LiveFn = fn(&mut CoreState, &Env<'_>, Option<Pick>) -> Result<(Live, Out), LiveError>;

/// The function that reads the question the arm's open window waits on.
pub type AskFn = fn(&CoreState, &Env<'_>) -> Result<Option<Asking>, LiveError>;

/// The arm registry (package KF1/M0): the live functions of `arm`, `None` while the arm is not native (`Step::Lend`). A lane adds its
/// arm HERE, one line in each of the two matches (the only edit this file takes from the lanes), and removes its `Lent::Arm` use.
#[must_use]
pub fn live_arm(arm: Arm) -> Option<LiveFn> {
    match arm {
        Arm::Tokens => Some(super::windows::status::advance_tokens),
        Arm::Status => Some(super::windows::status::advance_status),
        Arm::Scoring => Some(super::windows::status::advance_scoring),
        Arm::EventScoring => Some(super::windows::status::advance_event_scoring),
        Arm::Agenda => Some(super::agenda::advance_agenda),
        Arm::Voting => Some(super::agenda::advance_voting),
        Arm::TurnPrelude => Some(super::turn::advance_prelude),
        Arm::StrategyAnnounce => Some(super::turn::advance_announce),
        Arm::Phase => Some(super::turn::advance_phase),
        Arm::Ask => Some(super::turn::ask::advance_ask),
        _ => None,
    }
}

/// The ask counterpart of [`live_arm`] (the two entries of an arm go together).
#[must_use]
pub fn ask_arm_fn(arm: Arm) -> Option<AskFn> {
    match arm {
        Arm::Tokens => Some(super::windows::status::ask_tokens),
        Arm::Scoring => Some(super::windows::status::ask_scoring),
        Arm::EventScoring => Some(super::windows::status::ask_event_scoring),
        Arm::Voting => Some(super::agenda::ask_voting),
        Arm::TurnPrelude | Arm::StrategyAnnounce | Arm::Phase | Arm::Status | Arm::Agenda => Some(super::turn::ask_job),
        Arm::Ask => Some(super::turn::ask::ask_ask),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::reaction::{AbilityId, AbilityKind, Ask as RAsk, Opt as ROpt, Relation};
    use crate::choice::{Ask, Opt};
    use crate::driver::NoSeeds;
    use crate::generated::Faction;
    use crate::ids::Seat;
    use crate::kernel::resolver::AbilityDef;
    use crate::kernel::stage::{Illegal, Slot, Spec, Sx};
    use crate::kernel::{fx, Next};
    use crate::state::{Code, EventKind, Payload, Player, Players, ProductionUse, WindowKind};

    /// A toy machine held as the (stub) production window: `data[0]` is whether its event was emitted. It emits one event and ends;
    /// the event has two optional abilities, so the emission asks from inside the step.
    #[derive(Clone)]
    struct Toy {
        emitted: bool,
    }

    impl Window for Toy {
        type Stage = bool;
        fn stage(&self) -> bool {
            self.emitted
        }
        fn step(&mut self, sx: &mut Sx<'_>) -> Next {
            if !self.emitted {
                self.emitted = true;
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

    impl Held for Toy {
        const KIND: WindowKind = WindowKind::Production;
        fn from_pending(core: &CoreState, _: &Kernel) -> Option<Self> {
            match core.pending().window(WindowKind::Production)? {
                OpenWindow::Production(p) => Some(Self { emitted: p.data[0] == 1 }),
                _ => None,
            }
        }
        fn to_pending(&self, core: &mut CoreState) {
            core.pending_mut().open(OpenWindow::Production(Box::new(ProductionUse { actor: None, data: [u32::from(self.emitted), 0, 0] })));
        }
    }

    fn ability(key: u16) -> AbilityId {
        AbilityId { kind: AbilityKind::Ability, key: Code(key) }
    }

    fn position() -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::from_index(0).unwrap())]);
        s
    }

    fn env(k: &Kernel) -> Env<'_> {
        let mut resolver = Resolver::new(&[Seat(0)], Some(Seat(0)));
        resolver.register([
            AbilityDef::new(ability(1), Seat(0), EventKind::TradeGoodsGained, Relation::After).with_optional(true).with_arg(1),
            AbilityDef::new(ability(2), Seat(0), EventKind::TradeGoodsGained, Relation::After).with_optional(true),
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
        Env { kernel: k, registry: Cow::new(resolver.registry().clone()), table, seeds: Rc::new(NoSeeds) }
    }

    fn goods(s: &CoreState) -> i16 {
        s.players().by_seat(Seat(0)).unwrap().econ.trade_goods
    }

    fn pick(id: u16) -> Pick {
        Pick::Opt(Opt::Reaction(ROpt::Ability(ability(id))))
    }

    /// Ask, drop the driver, rebuild from the position alone, answer: the same position as the run that was never interrupted.
    #[test]
    fn a_nested_question_survives_a_dropped_driver_and_continues_to_the_same_position() {
        let k = Kernel::plain();
        let env = env(&k);
        // The uninterrupted run, in one `Driver`.
        let mut straight = position();
        open(&mut straight, &Toy { emitted: false });
        let (live, _) = advance::<Toy>(&mut straight, &env, None).unwrap();
        let Live::Asks(first) = live else { panic!("the reaction window asks") };
        assert_eq!(first.choice.ask, Ask::Reaction(RAsk::Window { event: EventKind::TradeGoodsGained, relation: Relation::After }));
        assert!(first.nested.is_some(), "it is a question from inside an emission");
        assert!(straight.pending().nested.is_some(), "the suspension is in the position");
        assert_eq!(goods(&straight), 0, "the suspended run was rolled back");
        // A clone of the position stands for the checkpoint: the driver of the first call is long gone.
        let mut resumed = straight.clone();
        let asked = ask::<Toy>(&resumed, &env).unwrap().expect("the suspended question is asked again");
        assert_eq!(asked.choice, first.choice, "a replay of the suspension asks the same question");
        let (live, _) = advance::<Toy>(&mut resumed, &env, Some(pick(1))).unwrap();
        let Live::Asks(second) = live else { panic!("the second ability may still be used") };
        assert!(!second.choice.opts.iter().any(|o| *o == Opt::Reaction(ROpt::Ability(ability(1)))), "already resolved in this window");
        let (live, out) = advance::<Toy>(&mut resumed, &env, Some(Pick::Opt(Opt::Decline))).unwrap();
        assert_eq!(live, Live::Done);
        assert_eq!(goods(&resumed), 1, "the effect ran once");
        assert_eq!(out.emitted.len(), 1);
        assert!(!is_open::<Toy>(&resumed) && resumed.pending().nested.is_none(), "the window left the stack");
        // The same answers in one go on the position that was never cloned.
        advance::<Toy>(&mut straight, &env, Some(pick(1))).unwrap();
        advance::<Toy>(&mut straight, &env, Some(Pick::Opt(Opt::Decline))).unwrap();
        assert_eq!(straight, resumed, "a clone mid-window continues identically");
    }

    #[test]
    fn an_illegal_answer_leaves_the_position_exactly_as_it_was() {
        let k = Kernel::plain();
        let env = env(&k);
        let mut s = position();
        open(&mut s, &Toy { emitted: false });
        advance::<Toy>(&mut s, &env, None).unwrap();
        let held = s.clone();
        assert_eq!(advance::<Toy>(&mut s, &env, Some(Pick::Number(3))), Err(LiveError::Illegal));
        assert_eq!(s, held, "the rollback restores the position, the suspension included");
        let mut closed = position();
        assert_eq!(advance::<Toy>(&mut closed, &env, None), Err(LiveError::NotOpen));
    }

    #[test]
    fn a_nested_pick_that_is_no_option_is_refused_and_the_registry_names_the_native_arms() {
        let k = Kernel::plain();
        let env = env(&k);
        let mut s = position();
        open(&mut s, &Toy { emitted: false });
        assert!(is_open::<Toy>(&s), "an opened window is on the stack");
        advance::<Toy>(&mut s, &env, None).unwrap();
        let held = s.clone();
        assert_eq!(advance::<Toy>(&mut s, &env, Some(pick(9))), Err(LiveError::Illegal), "an ability the window does not offer");
        assert_eq!(s, held);
        assert!(live_arm(Arm::Tokens).is_some() && ask_arm_fn(Arm::Tokens).is_some());
        assert!(live_arm(Arm::Voting).is_none() && ask_arm_fn(Arm::Voting).is_none(), "an arm no lane has ported stays lent");
    }

    /// The play facts of a card played inside the window are those of the position the call starts from (`table_at`): the provider is
    /// asked at every call, never once per game. The ability reads the facts it finds and gains that many trade goods.
    #[test]
    fn the_play_facts_an_ability_finds_are_computed_from_the_position_of_the_call() {
        use crate::kernel::ability::{PlayFacts, PlaysProvider};
        let k = Kernel::plain();
        let mut env = env(&k);
        let mut table = AbilityTable::default();
        table.add(
            Spec::new(Slot(1), AbilityKind::Ability, EventKind::TradeGoodsGained, Relation::After, |eff| {
                // The facts of the position: `rules_version` stands for any fact that changes during a game.
                let seen = eff.adapter_facts().plays.as_ref().map_or(0, |p| i32::try_from(p.rules_version).unwrap_or(i32::MAX));
                fx::gain_trade_goods(eff.state(), Seat(0), seen);
                Ok(())
            })
            .optional(),
        );
        table.facts.plays_provider = Some(PlaysProvider(std::sync::Arc::new(|core, _| {
            Some(PlayFacts {
                scope: 0,
                blanked: false,
                action_card: Code(0),
                supply_base: Code(0),
                note_order: |a, b| a.cmp(&b),
                seats: Vec::new(),
                name_group: Vec::new(),
                has_effect: Vec::new(),
                skimmer: false,
                roster: None,
                rules_version: core.meta().rules_version,
                tech_order: |a, b| a.cmp(&b),
                shared: crate::kernel::ability::SharedPlay::default(),
            })
        })));
        env.table = table;
        // Two positions that differ in the one fact: the same Env, the same window, the facts follow the position.
        for version in [5u32, 9] {
            let mut s = position();
            s.meta_mut().rules_version = version;
            assert_eq!(table_at(&s, &env).facts.plays.as_ref().map(|p| p.rules_version), Some(version));
            open(&mut s, &Toy { emitted: false });
            advance::<Toy>(&mut s, &env, None).unwrap();
            advance::<Toy>(&mut s, &env, Some(pick(1))).unwrap();
            advance::<Toy>(&mut s, &env, Some(Pick::Opt(Opt::Decline))).unwrap();
            assert_eq!(goods(&s), i16::try_from(version).unwrap(), "the ability found the facts of THIS position");
        }
        // No provider: the table's own facts stand (none), and the ability sees no play facts.
        let mut plain = env.clone();
        plain.table.facts.plays_provider = None;
        assert!(table_at(&position(), &plain).facts.plays.is_none());
        let mut s = position();
        open(&mut s, &Toy { emitted: false });
        advance::<Toy>(&mut s, &plain, None).unwrap();
        advance::<Toy>(&mut s, &plain, Some(pick(1))).unwrap();
        advance::<Toy>(&mut s, &plain, Some(Pick::Opt(Opt::Decline))).unwrap();
        assert_eq!(goods(&s), 0, "without a provider a card finds no play facts");
    }

    #[test]
    fn a_window_that_asks_nothing_runs_to_its_end_and_refuses_an_answer() {
        let k = Kernel::plain();
        let mut env = env(&k);
        env.registry = Cow::new(Registry::default());
        let mut s = position();
        open(&mut s, &Toy { emitted: false });
        assert_eq!(advance::<Toy>(&mut s, &env, Some(pick(1))), Err(LiveError::NotAsking), "nothing is asked on a fresh window");
        let (live, out) = advance::<Toy>(&mut s, &env, None).unwrap();
        assert_eq!((live, out.emitted.len()), (Live::Done, 1));
        assert!(!is_open::<Toy>(&s));
    }
}
