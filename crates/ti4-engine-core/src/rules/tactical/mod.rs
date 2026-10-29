//! The tactical action as one window (package A2b; `docs/todo/A2b.md`): the activation of a system, the movement step
//! (a ship, its route and boosts), the cargo hold of the ship that moves, the move itself with its rift rolls and
//! arrival event, and the close of the movement step. The string engine's `TacticalWindow` stages are [`Stage`].
//!
//! The unit of the lockstep dual is one answer, as for the transaction window: the adapter builds the machine at the
//! position the string window is in ([`Machine::activating`], [`Machine::moving`], [`Machine::loading`]), answers the
//! question the string was asked with the string's answer, and compares the position afterwards. [`Machine::single`]
//! ends the run after that answer.
//!
//! What stays with the string engine (a documented bail each, `docs/ENGINE_KNOWN_ISSUES.md` A2b): the hook chain of an
//! activation (technologies and abilities that ask or end the turn), Mahact's commander, Ceasefire, structured
//! diplomacy, a route other than the default (the adapter hands the machine the chosen route), Nova Seed, the
//! Sardakk hero and Floating Factories at the close, and the aftermath that follows (D1b).

mod activate;
mod cargo;
mod finish;
mod movement;

use crate::choice::generic::Rule;
use crate::choice::tactical::{Ask, Opt, Subtype};
use crate::choice::{Choice, Ctx, Opt as COpt, Opts, Origin, Pick, Subtype as CSub, Target};
use crate::containers::SmallList;
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::transit::Hold;
use crate::state::{Code, CoreState, Piece};

pub use finish::Closed;

/// The suspension points of [`Machine`] (the string `TacticalStage`, plus the end).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Activating,
    Moving,
    Loading,
    Done,
}

/// Facts the core has no text for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    /// The text `tactic` (the pool of the `COMMAND_TOKEN_PLACED` payload).
    pub pool: Code,
}

/// A line of the string engine's plain event log (`Game::emit`) the machine would write. Not text in the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plain {
    /// `SUPPORT_FOR_THE_THRONE_RETURNED:{owner}`.
    SupportReturned(Seat),
    /// `SYSTEM_ACTIVATED:{system}`.
    SystemActivated(System),
    /// `SYSTEM_ACTIVATED` (the free tactical action).
    FreeActivated,
    /// `FREE_TACTICAL_ACTION`.
    FreeTactical,
    /// `SHIP_MOVED`.
    ShipMoved,
    /// `SHIP_LOST_TO_GRAVITY_RIFT`.
    ShipLost,
}

/// What the last answer did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// A system was activated; the movement step follows.
    Activated,
    /// A ship was selected and its hold opened.
    Selected,
    /// A ship sailed (it arrived, or a gravity rift took it with its hold).
    Sailed { lost: bool },
    /// A passenger was taken aboard; the hold stays open.
    Loaded,
    /// The movement step was finished (the close runs apart: [`finish`]).
    Finished,
}

/// Why the machine stopped following (the adapter keeps the string result and counts the reason).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bail {
    /// The map slice holds no galaxy.
    NoGalaxy,
    /// A route other than the default, and the adapter named none.
    Route,
}

/// The route the player named when it is not the default one (`|rN`): the machine cannot list the routes, the adapter
/// hands over the path of the offered list (with the boosts that route spends).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePick {
    pub path: Vec<System>,
    pub gd: bool,
    pub ionian: bool,
}

/// The ship whose hold is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Loading {
    pub origin: System,
    pub ship: Piece,
    pub path: Vec<System>,
    pub hold: Hold,
}

/// The tactical action of one seat.
#[derive(Debug, Clone)]
pub struct Machine {
    pub(crate) seat: Seat,
    stage: Stage,
    pub(crate) facts: Facts,
    pub(crate) load: Option<Loading>,
    pub(crate) route: Option<RoutePick>,
    single: bool,
    /// The free tactical action's system, until the machine has activated it.
    free: Option<System>,
    /// The last answer's effect.
    pub step: Option<Step>,
    /// The plain events written so far, in order.
    pub plain: Vec<Plain>,
    /// The faces of the rift dice rolled so far.
    pub dice: Vec<u32>,
    /// Set when the position holds something the machine does not follow (the adapter then keeps the string result).
    pub bailed: Option<Bail>,
}

impl Machine {
    fn new(seat: Seat, stage: Stage, facts: Facts) -> Self {
        Self { seat, stage, facts, load: None, route: None, single: false, free: None, step: None, plain: Vec::new(), dice: Vec::new(), bailed: None }
    }

    /// The seat is to choose a system to activate.
    #[must_use]
    pub fn activating(seat: Seat, facts: Facts) -> Self {
        Self::new(seat, Stage::Activating, facts)
    }

    /// The seat is to choose a ship to move (or to finish the movement step).
    #[must_use]
    pub fn moving(seat: Seat, facts: Facts) -> Self {
        Self::new(seat, Stage::Moving, facts)
    }

    /// The seat fills the hold of `ship`, which moves from `origin` along `path` (origin first, the active system last).
    /// `loaded` and `closed` are what the string hold already has.
    #[must_use]
    pub fn loading(core: &CoreState, k: &Kernel, seat: Seat, facts: Facts, origin: System, ship: Piece, path: Vec<System>, loaded: &[u16], closed: bool) -> Self {
        let mut hold = Hold::for_ship(core, k, seat, origin, ship, &path);
        hold.loaded = loaded.iter().copied().collect();
        hold.closed = closed;
        let mut m = Self::new(seat, Stage::Loading, facts);
        m.load = Some(Loading { origin, ship, path, hold });
        m
    }

    /// A free tactical action (no token): the machine activates `system` on its first step.
    #[must_use]
    pub fn free(seat: Seat, facts: Facts, system: System) -> Self {
        let mut m = Self::new(seat, Stage::Activating, facts);
        m.free = Some(system);
        m
    }

    /// End the run after the next answer (the adapter's one string step).
    #[must_use]
    pub fn single(mut self) -> Self {
        self.single = true;
        self
    }

    /// The route the next move answer names (see [`RoutePick`]).
    #[must_use]
    pub fn with_route(mut self, route: RoutePick) -> Self {
        self.route = Some(route);
        self
    }

    /// The seat that acts.
    #[must_use]
    pub const fn seat(&self) -> Seat {
        self.seat
    }

    /// The hold of the ship that is loading, with its candidates: for the lockstep's value.
    #[must_use]
    pub fn hold(&self) -> Option<&Hold> {
        self.load.as_ref().map(|l| &l.hold)
    }

    fn finished(&self) -> bool {
        self.single && self.step.is_some()
    }
}

pub(crate) fn ctx(rule: Rule, subtype: Subtype, target: Option<Target>) -> Ctx {
    Ctx { origin: Origin::Rule(rule), subtype: CSub::Tactical(subtype), optional: false, target, outstanding: SmallList::new(), space_battle: false, invasion_seq: None }
}

impl Window for Machine {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if let Some(system) = self.free.take() {
            activate::free(self, sx, system);
            self.stage = Stage::Moving;
            return Next::Done;
        }
        if self.finished() {
            return Next::Done;
        }
        match self.stage {
            Stage::Activating => match activate::ask(sx.state, sx.kernel, self.seat) {
                Some(_) => Next::Continue(crate::choice::Ask::Tactical(Ask::Activate { seat: self.seat })),
                None => Next::Done,
            },
            Stage::Moving => Next::Continue(crate::choice::Ask::Tactical(Ask::Move { seat: self.seat })),
            Stage::Loading => match self.load.as_ref().and_then(|l| cargo::ask(l, self.seat)) {
                Some(c) => Next::Continue(c.ask),
                None => Next::Done,
            },
            Stage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.finished() || self.free.is_some() {
            return None;
        }
        match self.stage {
            Stage::Activating => {
                let systems = activate::ask(s, k, self.seat)?;
                let mut opts = Opts::default();
                for system in systems {
                    opts.0.push(COpt::Tactical(Opt::Activate(system)));
                }
                Some(Choice {
                    actor: self.seat,
                    ask: crate::choice::Ask::Tactical(Ask::Activate { seat: self.seat }),
                    ctx: ctx(Rule::R89_1, Subtype::ActivateSystem, None),
                    opts,
                })
            }
            Stage::Moving => movement::ask(s, k, self.seat),
            Stage::Loading => self.load.as_ref().and_then(|l| cargo::ask(l, self.seat)),
            Stage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(opt) = pick else { return Err(Illegal) };
        let COpt::Tactical(opt) = opt else { return Err(Illegal) };
        match self.stage {
            Stage::Activating => {
                let Opt::Activate(system) = opt else { return Err(Illegal) };
                if !activate::ask(sx.state, sx.kernel, self.seat).is_some_and(|l| l.contains(&system)) {
                    return Err(Illegal);
                }
                activate::run(self, sx, system)?;
                self.stage = Stage::Moving;
                self.step = Some(Step::Activated);
                Ok(())
            }
            Stage::Moving => movement::answer(self, sx, opt),
            Stage::Loading => cargo::answer(self, sx, opt),
            Stage::Done => Err(Illegal),
        }
    }
}

impl Machine {
    pub(crate) fn set_stage(&mut self, stage: Stage) {
        self.stage = stage;
    }

    /// The close of the movement step ([`finish::close`]); the machine is done afterwards.
    pub fn close(&mut self, sx: &mut Sx<'_>) -> Closed {
        let out = finish::close(self, sx);
        self.stage = Stage::Done;
        out
    }
}
