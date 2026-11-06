//! `Pending`: the open windows and the question (skeleton; the model is section 2, packages F4 and
//! D1). Replaces the private window fields of `Game` and `GameState.pending`.

use super::pending_windows::{LeaderFollowers, Tokens, Trade, Voting};
use super::{Aftermath, CombatFields, Invasion, ProductionUse, Tactical};
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
    /// The space combat of an aftermath, above its `Aftermath` entry (package D1b2).
    Combat,
    /// The invasion of an aftermath (`Window::Invasion`, a stub payload until lane M3).
    Invasion,
    /// One use of PRODUCTION (`Window::Production`, a stub payload until lane M2).
    Production,
    /// A job of the turn flow suspended on a question (`Window::TurnFlow`, lane B).
    TurnFlow,
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
    /// A disposable one-follower window on a planning fork (`StrategySecondaryWindow::preview`): completing it announces
    /// `SECONDARY_PREVIEW_COMPLETE` and neither finishes the action nor advances the turn.
    pub preview: bool,
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
    /// Mathis Mathinus (Winnu hero): the follower selection still running (package D1a remainder).
    LeaderFollowers(LeaderFollowers),
    /// The command token gain of the status phase with the status report it carries.
    Tokens(Box<Tokens>),
    /// The agenda phase's vote and the agendas still to come.
    Voting(Box<Voting>),
    /// An open transaction between two seats.
    Trade(Trade),
    /// A tactical action (package D1b): activation, movement step, a hold being filled.
    Tactical(Tactical),
    /// The steps after a movement step: space combat, invasion, production (package D1b).
    Aftermath(Box<Aftermath>),
    /// The space combat of an aftermath: the window fields of the combat machine (package D1b2).
    Combat(Box<CombatFields>),
    /// The invasion of an aftermath (M0 stub payload: lane M3 types it).
    Invasion(Box<Invasion>),
    /// One use of PRODUCTION (M0 stub payload: lane M2 types it).
    Production(Box<ProductionUse>),
    /// A job of the turn flow (lane B): the prelude, the end of an action or a turn, a phase change.
    TurnFlow(super::TurnJob),
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
            Self::LeaderFollowers(_) => WindowKind::LeaderFollowers,
            Self::Tokens(_) => WindowKind::Tokens,
            Self::Voting(_) => WindowKind::Voting,
            Self::Trade(_) => WindowKind::Trade,
            Self::Tactical(_) => WindowKind::Tactical,
            Self::Aftermath(_) => WindowKind::Aftermath,
            Self::Combat(_) => WindowKind::Combat,
            Self::Invasion(_) => WindowKind::Invasion,
            Self::Production(_) => WindowKind::Production,
            Self::TurnFlow(_) => WindowKind::TurnFlow,
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
    /// Remember that the component action `code` failed this turn (once).
    pub fn note_failed_component(&mut self, code: super::Code) {
        if !self.failed_component_actions.as_slice().contains(&code) {
            self.failed_component_actions.push(code);
        }
    }

    /// Remember that the menu option with `key` failed this turn (once): the live menu does not offer it again.
    pub fn note_failed_key(&mut self, key: u32) {
        if !self.failed_keys.as_slice().contains(&key) {
            self.failed_keys.push(key);
        }
    }

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

/// What the driver was doing when a question from INSIDE an emission (a reaction window, an ability) suspended the step: the
/// operation to re-run and the answers the tape already holds. The machine payload on the stack is the window as the driver
/// rolled it back (before `op`), so `from_pending` + `Driver::restore_nested` + the next answer continues exactly where the
/// uninterrupted run would have gone (package M0, `driver::live`). `None` between steps of a blocking table.
#[derive(Clone, PartialEq, Eq)]
pub struct Nested {
    pub op: NestedOp,
    pub answers: SmallList<crate::choice::Pick, 8>,
    /// The question the step was suspended on, as the driver put it (package nestq-1, `driver::live`): an answer is checked against it
    /// and resumes the step without running it again to rebuild the question. Derived, not game state: see [`Memo`].
    pub asked: Memo,
}

impl core::fmt::Debug for Nested {
    // (Printed as before the memo: a hash of a printed position must not see it.)
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Nested").field("op", &self.op).field("answers", &self.answers).finish()
    }
}

/// A value derived from the position and kept with it so that it is not derived again (a cache the driver fills with a type of its own,
/// layer L5; this layer only carries it). Shared by `Clone` (one reference count), equal to every other memo and never printed: the
/// equality, the hashes and the codec of the position do not see it. Whoever reads it checks that it still belongs to the position.
#[derive(Clone, Default)]
pub struct Memo(Option<std::sync::Arc<dyn core::any::Any + Send + Sync>>);

impl Memo {
    #[must_use]
    pub fn of<T: core::any::Any + Send + Sync>(value: T) -> Self {
        Self(Some(std::sync::Arc::new(value)))
    }

    /// The value, when there is one of type `T`.
    #[must_use]
    pub fn get<T: core::any::Any>(&self) -> Option<&T> {
        self.0.as_deref().and_then(|v| v.downcast_ref())
    }

    /// The value as a reference count of its own (it outlives a borrow of the position).
    #[must_use]
    pub fn shared<T: core::any::Any + Send + Sync>(&self) -> Option<std::sync::Arc<T>> {
        self.0.clone().and_then(|v| v.downcast().ok())
    }
}

impl PartialEq for Memo {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Memo {}

impl core::fmt::Debug for Memo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Memo")
    }
}

/// The operation a window step was running (`Driver`'s own two operations).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NestedOp {
    /// `Window::step`.
    Step,
    /// `Window::answer(pick)` of the window's own question.
    Answer(crate::choice::Pick),
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
    /// `Game.turn_closing`: the seat in the closing menu of its turn (package D1b).
    pub turn_closing: Option<Seat>,
    /// `Game.actions_this_turn`: the actions the active seat has taken this turn.
    pub actions_this_turn: u8,
    /// `Game.prepared_turn_seq`: the turn whose start-of-turn work has run.
    pub prepared_turn_seq: Option<u32>,
    /// What the timing resolver carries from window to window (D2-4b): spent frequency keys, the initiative
    /// order it was made with, the open emissions, the next trace id.
    pub resolver: super::ResolverState,
    /// `Game.failed_component_actions`: the component actions that failed this turn (their menu ids, interned), which the turn menu
    /// does not offer again; cleared by `driver::turn_flow::forget_failed_components` when the turn ends and in a hypothetical turn.
    pub failed_component_actions: SmallList<super::Code, 8>,
    /// The same, as the typed options (`choice::turn::Opt::key`) the live menu filters on; cleared with it.
    pub failed_keys: SmallList<u32, 8>,
    /// The suspended nested question of the live window (see [`Nested`]); written by `driver::live::advance` only.
    pub nested: Option<Nested>,
}
