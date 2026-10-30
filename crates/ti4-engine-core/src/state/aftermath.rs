//! The typed payload of an open aftermath (package D1b; the `Pending` window of the steps after a movement step): what
//! `Game.aftermath` holds between steps, without text. The live sub-window (the space combat, the invasion, the production
//! step) is the window of its own area; the payload holds only where the chain stands.

use crate::containers::SmallList;
use crate::generated::System;
use crate::ids::Seat;

/// Whose promissory notes each seat held when the tactical action began (`combat::NoteHoldings`): `(holder, issuer)`,
/// holders in seating order, a holder's issuers in seating order.
pub type NoteHold = SmallList<(Seat, Seat), 24>;

/// Where the post-movement chain has reached (the string `Aftermath`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AStage {
    /// Space cannon offense has fired; the space combat is running.
    Fighting,
    /// The active seat may negotiate before the next step (a contact is possible); `invade` says which step follows.
    Negotiating { invade: bool },
    Invading,
    Producing,
    Done,
}

/// How an event scoring pause of the chain is limited (`EventScoreLimit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseLimit {
    OnePerPlayer,
    AnyPerPlayer,
}

/// The system before the fight (`combat::BeforeCombat`): the facts the secrets ask for that are gone when the fight ends.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Before {
    /// The seats with ships when the fight opened, in seating order.
    pub sides: SmallList<Seat, 8>,
    /// War suns and flagships each side had.
    pub capitals: SmallList<(Seat, u16), 8>,
    /// The note holdings of the tactical action's start.
    pub notes: NoteHold,
}

/// An open aftermath (`AftermathWindow`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aftermath {
    pub player: Seat,
    pub system: System,
    pub stage: AStage,
    pub before_combat: Before,
    /// Whether the combat's feats were recorded, so a settle that loops does not record them twice.
    pub feats_noted: bool,
    /// A scoring pause emitted by a concrete tactical event: the feat occurrence and its limit.
    pub event_scoring: Option<(u64, PauseLimit)>,
    pub notes_at_start: NoteHold,
}

/// One queued hit of a space combat (string `Pending`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatPend {
    pub player: Seat,
    pub hits: u32,
    pub producer: Seat,
    pub non_fighters_only: bool,
    pub producer_assigns: bool,
}

/// Where a combat stands (string `Stage`; `Done` stands for a concluded fight, the outcome is in [`CombatFields`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatStep {
    Opening { round: u32 },
    Announcing { round: u32, asking: Seat, announced: Vec<Seat> },
    Rolling { round: u32 },
    /// The barrage scored a feat: the round resumes at the announcement once the scoring window has closed.
    AfterBarrage { round: u32 },
    Queue { assigning: bool, round: u32, queue: Vec<CombatPend> },
    Retreating { round: u32, leaving: Vec<Seat> },
    Done,
}

/// What the hit queue at the front is made of (string `HitPhase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitPhase {
    Round,
    CombatStart,
    RoundEnd,
}

/// The window fields the string `CombatWindow` keeps between its stages (the payload of `Window::Combat`; the machine of
/// `rules::combat::machine` is rebuilt from it and the facts of the position at each step).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatFields {
    pub step: CombatStep,
    pub phase: HitPhase,
    pub resuming: bool,
    pub pending_retreats: Vec<Seat>,
    pub offered: Vec<(u32, Seat)>,
    pub damaged: Vec<(Seat, Vec<crate::generated::Unit>)>,
    pub fled: Vec<(Seat, Vec<crate::generated::Unit>)>,
    pub at_start: Vec<(Seat, Vec<crate::generated::Unit>)>,
    /// The anti-fighter barrage scored a feat: the secret's scoring window opens before the round resumes.
    pub scoring: bool,
    /// `Stage::Done(outcome)`: the winner and the rounds fought.
    pub outcome: Option<(Option<Seat>, u32)>,
    pub end_announced: bool,
}
