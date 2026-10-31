//! The typed payload of an open invasion (`Window::Invasion`; lane M3, `docs/D2_LF_RESUME.md` and `docs/M3_RESUME.md`): what the string
//! `InvasionWindow` keeps between two questions, without text. The machine that runs it is `driver::windows::invasion`; it is rebuilt from
//! this payload and the position at every call, so a checkpoint is a plain `CoreState::clone`.
//!
//! The 16 fields of the string window map as follows: `invader`, `system` are the same; `stage` is [`IStage`]; `report` is [`IReport`];
//! `pending_scoring_occurrences` is `scoring`; `current_ground_occurrence` is `ground_occurrence`; `notes_at_tactical_start` is `notes`;
//! `bombard_plan`, `bombard_index`, `bombard_occurrence`, `bombard_announced` are [`IBombard`] (the coexistence cursor is its `cursor`);
//! `extra_commits`, `temporary_space_commits`, `combat_only` are the same; `strict_timing_error` is a `LiveError`, `galaxy` is the map.

use crate::containers::SeatSet;
use crate::generated::{Planet, System};
use crate::ids::Seat;

use super::{Code, InvasionDie, NoteHold, Piece};

/// A planet some ground force was committed from by a faction's hook (`CommitOrigin`).
pub type CommitOrigin = (System, Planet);

/// One planet of the rolled bombardment (`BombardPlan`): the units standing there before the dice, the dice as the board shows them, the
/// hit groups of the bombarding units, how many ground forces stood there and whose they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IPlanetBombard {
    pub planet: Planet,
    pub before: Vec<Piece>,
    pub dice: Vec<InvasionDie>,
    pub groups: Vec<u32>,
    pub held: u16,
    pub victims: SeatSet,
}

/// The coexistence pause of the bombardment (`BombardCursor`): which planet, the groups, the next group to assign, the units destroyed so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IBombardCursor {
    pub planet_index: u8,
    pub planet: Planet,
    pub groups: Vec<u32>,
    pub next_group: u8,
    pub taken: u16,
    pub held: u16,
    pub victims: SeatSet,
}

/// The bombardment (`bombard_plan`, `bombard_index`, `bombard_occurrence`, `bombard_announced`, and the pause).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IBombard {
    pub plan: Vec<IPlanetBombard>,
    pub index: u8,
    pub occurrence: u64,
    /// The reroll windows after the rolls were opened (the hits were recomputed from the possibly rerolled dice).
    pub announced: bool,
    pub cursor: Option<IBombardCursor>,
}

/// A bill the invader is paying: the custodians' six influence, or a Dunlain Reaper's two resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IBill {
    pub who: Seat,
    pub influence: bool,
    pub cost: u16,
    /// Worth collected so far.
    pub paid: u16,
}

/// Where the custodians offer stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ICustodians {
    /// Not looked at yet.
    Check,
    /// The offer is on the table.
    Asking,
    /// A paid "yes": the bill is being paid.
    Paying(IBill),
}

/// Where a ground round stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IRound {
    /// "Fight a round on {planet}" is asked of the invader.
    Asking,
    /// The start of the round asks the Dunlain Reaper of participant `at` (0 the invader, 1 the defender); `bill` is its payment.
    Reaper { at: u8, bill: Option<IBill>, asked: bool, source: Code },
}

/// Where an open invasion has reached (the string `Stage`; the bombardment pause is `IBombard::cursor`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IStage {
    /// Just opened: `INVASION_BEGAN` is announced, the bombardment is rolled.
    Begin,
    Bombarding,
    Custodians(ICustodians),
    Committing,
    Fighting { planets: Vec<Planet>, index: u8, defender: Seat, round: IRound },
    Advancing { planets: Vec<Planet>, index: u8 },
    ChoosingNextCombat { planets: Vec<Planet>, index: u8, planet: Planet, remaining: Vec<Seat> },
    FinalizingControl { planets: Vec<Planet>, index: u8, planet: Planet, previous: Option<Seat> },
    Done,
}

/// What the invasion did (`InvasionReport`); the hero windows that fight a committed planet read `captured`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IReport {
    /// The planets a first claim explored.
    pub explored: Vec<Planet>,
    pub committed: Vec<Planet>,
    /// Each planet that changed hands and its former holder.
    pub captured: Vec<(Planet, Option<Seat>)>,
    pub bombardment_kills: u16,
    pub custodians_removed: bool,
    pub coexisted: Vec<Planet>,
}

/// An open invasion of an aftermath (the string `InvasionWindow`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invasion {
    pub invader: Seat,
    pub system: System,
    pub stage: IStage,
    pub report: IReport,
    /// Feat occurrences whose scoring window the caller opens, first first; `true`: a combat's (one scoring per player).
    pub scoring: Vec<(u64, bool)>,
    pub ground_occurrence: Option<u64>,
    pub notes: NoteHold,
    pub bombard: IBombard,
    /// Planets this invasion committed ground forces from through a faction's hook.
    pub extra_commits: Vec<CommitOrigin>,
    /// Units committed as ground forces for the moment (Matriarch's fighters), by landing planet.
    pub temp_commits: Vec<(Planet, Vec<Piece>)>,
    /// Already committed hero combats leave invasion/control continuations to their caller.
    pub combat_only: bool,
}

impl Invasion {
    /// A payload for observers that only know whose invasion it is (the dual's `d1_action::sub_of`): the rest is the machine's.
    #[must_use]
    pub fn observed(invader: Seat, system: System) -> Self {
        Self {
            invader,
            system,
            stage: IStage::Done,
            report: IReport::default(),
            scoring: Vec::new(),
            ground_occurrence: None,
            notes: NoteHold::new(),
            bombard: IBombard::default(),
            extra_commits: Vec::new(),
            temp_commits: Vec::new(),
            combat_only: false,
        }
    }

    /// The seat whose invasion this is.
    #[must_use]
    pub const fn actor(&self) -> Seat {
        self.invader
    }
}
