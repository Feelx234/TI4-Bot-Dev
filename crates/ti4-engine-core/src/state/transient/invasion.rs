use crate::containers::{Cow, SortedMap};
use crate::generated::{Planet, System};
use crate::ids::Seat;

use super::super::piece::Piece;
use super::super::Code;

/// `InvasionStep.kind` (`"bombardment"`, `"space_cannon_defense"`, `"ground_round"`, ...): interned.
pub type InvasionKind = Code;
/// `ActiveInvasion.phase`: interned.
pub type InvasionPhase = Code;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvasionDie {
    pub planet: Planet,
    pub player: Seat,
    /// The dice group text (`"ground"`, `"fleet"`, ...): interned.
    pub group: Code,
    pub face: u32,
    pub target: u32,
    pub hit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvasionStep {
    pub planet: Planet,
    pub kind: InvasionKind,
    pub round: u32,
    pub before: Vec<Piece>,
    pub after: Vec<Piece>,
    pub dice: Vec<InvasionDie>,
    pub hits: SortedMap<Seat, u32, 4>,
    pub harrow_hits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveInvasion {
    pub system: System,
    pub invader: Seat,
    pub seq: u64,
    pub phase: InvasionPhase,
    pub planet: Option<Planet>,
    pub defender: Option<Seat>,
    pub ground_round: u32,
    pub last_step: Option<InvasionStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvasionScratch {
    pub active: Option<Cow<ActiveInvasion>>,
}
