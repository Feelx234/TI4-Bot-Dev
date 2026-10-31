//! The typed payload of an open production step (`Window::Production`; lane M2, `docs/M2_RESUME.md`): what one use of PRODUCTION keeps
//! between two questions, without text. The machine that runs it is `driver::windows::production`; it is rebuilt from this payload and the
//! position at every call, so a checkpoint is a plain `CoreState::clone`.
//!
//! [`ProductionWindow`] is the plain data of the window `rules::production::window` runs (its fields were the window's own; the rules keep the
//! `impl`s). [`ProductionUse`] is the composer's: the use's mode (which opener started it), where it stands (start effects, the window, the
//! effects at the end of the use) and whether an answer was given (the string runs the end-of-use effects after an answer, never for a use
//! that asked nothing).
//!
//! [`OneRun`] / [`OStage`] are the plain state of `rules::production::one::ProduceOne` (one unit outside a use of PRODUCTION: Chaos Mapping,
//! Freelancers), here so that a window that hosts it (the hosted exploration) can keep it between two questions.

use crate::choice::production::Spot;
use crate::containers::SmallList;
use crate::generated::{System, Unit};
use crate::ids::Seat;

use super::Code;

/// The suspension points of the production window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PStage {
    Choosing,
    Paying { unit: Unit, owed: i32, made: u8, cost: i32, paid: i32 },
    Placing { unit: Unit, made: u8 },
    Done,
}

/// One use of PRODUCTION (the plain data of the string `ProductionWindow`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionWindow {
    pub who: Seat,
    pub system: System,
    pub stage: PStage,
    pub limit: i32,
    pub remaining: i32,
    pub free_capacity: i32,
    pub small_exemptions: i32,
    pub credit: i32,
    pub discount_remaining: i32,
    pub fleet_limit: i32,
    pub armada: i32,
    pub free_this_use: bool,
    pub flagship_free: bool,
    pub only_unit: Option<Unit>,
    pub max_unit_cost: Option<i32>,
    pub buildable: [Option<Unit>; 9],
    /// Units produced so far in this use and where they went.
    pub produced: SmallList<(Unit, Spot), 12>,
    pub purchased: bool,
    pub placement_timing_done: bool,
    /// The rules version offers Berekar Berekon inside the production question.
    pub agent_rules: bool,
    /// Berekar Berekon was exhausted in this use.
    pub agent_spent: bool,
    /// The caller has a timing handle: the ground-force production window is announced.
    pub emit_timing: bool,
    /// The units come from an ability, not from the PRODUCTION of a unit (`ProductionWindow::for_ability`, package W5).
    pub ability: bool,
}

/// Which opener started the use: the effects around the window differ (T-I2-3, T-I2-4 of `docs/todo/D2-islands-machines.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseMode {
    /// The production step of a tactical action (`AftermathWindow::enter_production`): the use counter goes up, the Hacan hero is offered,
    /// Sarween Tools / AI Development Algorithm are asked and `PRODUCTION_USED` is announced before the first question.
    Tactical,
    /// A use of PRODUCTION at the home system outside a tactical action (`production::resolve_timed`: the Warfare secondary): the discount an
    /// earlier action left is dropped (from the version that offers Berekar Berekon) and the hero is offered; nothing is announced.
    Home,
    /// A production by an ability (`ProductionWindow::for_ability`: the Arborec and Muaat leaders): no effects around the window but those of its end.
    Ability,
}

/// Where the composer stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsePhase {
    /// Nothing ran yet: the start-of-use effects of the mode, then the window.
    Start,
    /// The window asks (or has just been answered).
    Using,
    /// The window ended after an answer: the effects at the end of the use.
    Finishing,
    Done,
}

/// An open use of PRODUCTION (the string `ProductionWindow` with the start and end effects `enter_production` / `finish_use` put around it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionUse {
    pub who: Seat,
    pub system: System,
    pub mode: UseMode,
    pub phase: UsePhase,
    pub window: ProductionWindow,
    /// An answer was given to the window: the end-of-use effects run when the window is done (a use that asked nothing has none).
    pub answered: bool,
}

impl ProductionUse {
    /// A payload for observers that only know whose use it is (the dual's `d1_action::sub_of`): the rest is the machine's.
    #[must_use]
    pub const fn observed(who: Seat, system: System) -> Self {
        Self { who, system, mode: UseMode::Tactical, phase: UsePhase::Using, window: ProductionWindow::blank(who, system), answered: false }
    }

    /// The seat whose use this is.
    #[must_use]
    pub const fn actor(&self) -> Seat {
        self.who
    }
}

impl ProductionWindow {
    /// A window that has not read the position yet (the opener of a mode whose start computes the facts).
    #[must_use]
    pub const fn blank(who: Seat, system: System) -> Self {
        Self {
            who,
            system,
            stage: PStage::Done,
            limit: 0,
            remaining: 0,
            free_capacity: 0,
            small_exemptions: 0,
            credit: 0,
            discount_remaining: 0,
            fleet_limit: 0,
            armada: 0,
            free_this_use: false,
            flagship_free: false,
            only_unit: None,
            max_unit_cost: None,
            buildable: [None; 9],
            produced: SmallList::new(),
            purchased: false,
            placement_timing_done: false,
            agent_rules: false,
            agent_spent: false,
            emit_timing: false,
            ability: false,
        }
    }
}

/// The suspension points of `ProduceOne`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OStage {
    Start,
    Choosing,
    Paying { unit: Unit, cost: i32, paid: i32 },
    Placing { unit: Unit },
    Done,
}

/// What `ProduceOne` needs of the position that the core does not hold (plain data; `kernel::children::OneFacts` is this type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OneFacts {
    /// What the seat may produce at all (`buildable_for`), in option order.
    pub buildable: [Option<Unit>; 9],
    /// The fleet limit as the enforcement works it out (`fleet::over_supply`: the unlimited bill, else the limit).
    pub fleet_limit: i32,
    /// The interned printed name of the effect and its content alias (`None`: it names itself nowhere).
    pub name: Option<Code>,
    pub alias: Option<Code>,
}

/// The plain state of one `ProduceOne` (one unit for `who` in `system`): everything the window keeps between two questions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OneRun {
    pub who: Seat,
    pub system: System,
    pub facts: OneFacts,
    pub stage: OStage,
    pub made: Option<(Unit, Spot)>,
    /// The unit stands over the fleet supply or the capacity of its system: the string asks which units go, which the window cannot.
    pub unsupported: bool,
}

impl OneRun {
    #[must_use]
    pub const fn new(who: Seat, system: System, facts: OneFacts) -> Self {
        Self { who, system, facts, stage: OStage::Start, made: None, unsupported: false }
    }
}
