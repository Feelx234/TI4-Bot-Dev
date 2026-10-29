//! `Pending`: the open windows and the question (skeleton; the model is section 2, packages F4 and
//! D1). Replaces the private window fields of `Game` and `GameState.pending`.

use crate::containers::{SeatSet, SmallList};
use crate::generated::{Agenda, Leader, StrategyCard};
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

/// How a follower of a strategic action's secondary stands (`strategy::SecondaryResolution`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryHow {
    /// Not asked: the follower may not follow (no token, no card to follow, a faction rule).
    Ineligible,
    Declined,
    Followed,
}

/// The follower window of a strategic action (`strategy::StrategySecondaryWindow`): the primary player's card, the
/// followers in resolution order, how far the window has come and how each follower answered (package D1a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Secondary {
    pub primary: Seat,
    pub card: StrategyCard,
    pub followers: SmallList<Seat, 8>,
    /// The index of the next follower to look at (the ones before it are in `resolved`).
    pub next: u8,
    pub resolved: SmallList<(Seat, SecondaryHow), 8>,
    /// Finishing the window exhausts the primary player's copy of the card.
    pub exhausts_card: bool,
}

/// When the secrets of a scoring window may be scored (`secrets::Timing`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoringTiming {
    Status,
    Action,
    Agenda,
}

/// The bookkeeping of an open scoring window (`objectives::Book`; the core's `rules::scoring_book::ScoringBook` is built
/// from it): who is asked next, what each player used, the plot card still to place (package D1a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scoring {
    /// Seats still to be asked in the string's stored order: the next one LAST.
    pub pending: SmallList<Seat, 8>,
    pub public_used: SeatSet,
    pub secret_used: SeatSet,
    /// Plots Within Plots: the Firmament that still has to place a plot card, and whose token goes on it.
    pub plot_card: Option<(Seat, Seat)>,
    pub timing: ScoringTiming,
    /// The feat occurrence the window settles (`FeatOccurrence`), pinned when it opened.
    pub occurrence: Option<u64>,
    /// `EventScoreLimit::OnePerPlayer`.
    pub one_per_player: bool,
}

/// An open window: one variant per window kind of `Game` (`game.rs:919-975`). The kinds that have a typed payload carry
/// it; the others are placeholders (`Untyped`) until their package types them (D1a: finished areas, D1b: the action phase).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Window {
    StrategySecondary(Secondary),
    /// The follower window of Thunder's Edge Warfare, resolved after the free tactical action.
    StrategySecondaryAfterTactical(Secondary),
    /// A hero's strategy primary and selected secondaries are still resolving (`Game.leader_strategy`).
    LeaderStrategy { player: Seat, leader: Leader, card: StrategyCard },
    /// Executive Order (Keleres): the owner, and the speaker whose seat is restored after the vote.
    ExecutiveOrder { owner: Seat, speaker: Seat },
    /// Agendas retained while the just-resolved agenda's scoring occurrence is open.
    AgendaQueueAfterEventScoring(SmallList<Agenda, 8>),
    /// The open 81.1 scoring window of the status phase.
    Scoring(Scoring),
    /// The window of an action- or agenda-timed secret.
    EventScoring(Scoring),
    /// A kind with no typed payload yet.
    Untyped { kind: WindowKind, actor: Option<Seat>, data: [u32; 3] },
}

impl Window {
    #[must_use]
    pub const fn kind(&self) -> WindowKind {
        match self {
            Self::StrategySecondary(_) => WindowKind::StrategySecondary,
            Self::StrategySecondaryAfterTactical(_) => WindowKind::StrategySecondaryAfterTactical,
            Self::LeaderStrategy { .. } => WindowKind::LeaderStrategy,
            Self::ExecutiveOrder { .. } => WindowKind::ExecutiveOrder,
            Self::AgendaQueueAfterEventScoring(_) => WindowKind::AgendaQueueAfterEventScoring,
            Self::Scoring(_) => WindowKind::Scoring,
            Self::EventScoring(_) => WindowKind::EventScoring,
            Self::Untyped { kind, .. } => *kind,
        }
    }
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

impl Pending {
    /// The open window of `kind`.
    #[must_use]
    pub fn window(&self, kind: WindowKind) -> Option<&Window> {
        self.stack.iter().find(|w| w.kind() == kind)
    }

    /// The open window of `kind`, mutably.
    pub fn window_mut(&mut self, kind: WindowKind) -> Option<&mut Window> {
        self.stack.iter_mut().find(|w| w.kind() == kind)
    }

    /// Open `window`, replacing an open window of its kind (a kind is open at most once).
    pub fn open(&mut self, window: Window) {
        match self.window_mut(window.kind()) {
            Some(slot) => *slot = window,
            None => self.stack.push(window),
        }
    }

    /// Close the window of `kind`; returns it.
    pub fn close(&mut self, kind: WindowKind) -> Option<Window> {
        let at = self.stack.iter().position(|w| w.kind() == kind)?;
        Some(self.stack.remove(at))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pending {
    /// Events staged by effects that had no resolver in reach (`kernel::fx::stage`), announced in order
    /// by `fx::announce_staged` (the string engine keeps them as `private:#staged:*` marks).
    pub staged: Vec<super::EventRec>,
    /// The open windows, the most recently opened last. At most one per kind (the string holds one `Option` per kind).
    pub stack: Vec<Window>,
    pub top: Option<PendingAsk>,
    pub tag: Option<PendingTag>,
    pub driver: DriverFlags,
}
