//! `Pending`: the open windows and the question (skeleton; the model is section 2, packages F4 and
//! D1). Replaces the private window fields of `Game` and `GameState.pending`.

use crate::containers::SmallList;
use crate::ids::Seat;

/// The one legacy tag `GameState.pending` ever holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingTag {
    Move,
}

/// One kind per `Option<Window>` field of `Game` (`game.rs:919-975`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WindowKind {
    StrategySecondary,
    StrategySecondaryAfterTactical,
    LeaderStrategy,
    LeaderFollowers,
    Scoring,
    EventScoring,
    Tokens,
    Voting,
    ExecutiveOrder,
    AgendaQueueAfterEventScoring,
    Tactical,
    Aftermath,
    Trade,
    Diplomacy,
    AgendaTalks,
}

/// An open window: its kind, who it waits on and a small payload whose layout the owning area
/// defines (D1 replaces this by one struct per kind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub kind: WindowKind,
    pub actor: Option<Seat>,
    pub data: [u32; 3],
}

/// The question on top: an area number, a code and arguments (F4 replaces it by `choice::Ask`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingAsk {
    pub area: u8,
    pub code: u16,
    pub args: [u32; 3],
}

/// Driver progress flags (`Game.status_resolved`, `agenda_resolved`, `strategy_phase_announced`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DriverFlags(pub u8);

impl DriverFlags {
    pub const STATUS_RESOLVED: u8 = 1;
    pub const AGENDA_RESOLVED: u8 = 1 << 1;
    pub const STRATEGY_PHASE_ANNOUNCED: u8 = 1 << 2;

    #[must_use]
    pub const fn has(self, flag: u8) -> bool {
        self.0 & flag == flag
    }

    pub fn set(&mut self, flag: u8, on: bool) {
        if on {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pending {
    /// Events staged by effects that had no resolver in reach (`kernel::fx::stage`), announced in order
    /// by `fx::announce_staged` (the string engine keeps them as `private:#staged:*` marks).
    pub staged: Vec<super::EventRec>,
    pub stack: SmallList<Window, 4>,
    pub top: Option<PendingAsk>,
    pub tag: Option<PendingTag>,
    pub driver: DriverFlags,
}
