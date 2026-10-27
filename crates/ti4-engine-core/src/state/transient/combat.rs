use crate::containers::{Cow, Log, SmallList, SortedMap};
use crate::generated::{ActionCard, Planet, System, Unit};
use crate::ids::Seat;

use super::super::piece::Piece;
use super::super::Code;

/// `CombatRollRecord`: a hit is `roll >= target`. The unit text is a unit type, or a pooled group such as
/// `carrier+fighter` (dice of several types rolled together): adapter-interned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatDie {
    pub player: Seat,
    pub unit: Code,
    pub roll: u32,
    pub target: u32,
    pub hit: bool,
}

/// `CombatPresentation.phase` (presentation only): adapter-interned text until B1 pins the list.
pub type CombatStage = Code;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CombatPresentation {
    pub battle_seq: u32,
    pub phase: Code,
    pub round: u32,
    pub round_start: Vec<Piece>,
    pub barrage_start: Vec<Piece>,
    pub barrage_hits: SortedMap<Seat, u32, 4>,
    pub barrage_dice: Vec<CombatDie>,
    pub remaining_hits: SortedMap<Seat, u32, 4>,
}

/// Which reroll window staged the dice (Fire Team, Scramble Frequency, Aglnlan Oln): adapter-interned.
pub type RerollKind = Code;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RerollEntry {
    pub unit: Code,
    pub planet: Option<Planet>,
    pub hits_on: Option<u8>,
    pub faces: Vec<u8>,
    /// Indices of dice rerolled, as a bit mask.
    pub rerolled: u64,
    pub deltas: SortedMap<u8, i8, 4>,
    pub unit_types: SortedMap<Code, u32, 4>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RerollSet {
    pub kind: RerollKind,
    pub system: System,
    pub rolls: Vec<RerollEntry>,
}

/// Why a unit was destroyed (`pending_destructions` cause text, e.g. `"space_combat"`).
pub type DestroyReason = Code;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingDestruction {
    pub system: System,
    pub owner: Seat,
    pub unit: Unit,
    pub reason: DestroyReason,
    pub during_space_combat: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CombatScratch {
    /// `reroll_staging`: sorted by seat.
    pub reroll_staging: Cow<Vec<(Seat, RerollSet)>>,
    pub last_reroll_player: Option<Seat>,
    /// `last_committed_unit`: (player, system, planet, unit).
    pub last_committed_unit: Option<(Seat, System, Planet, Piece)>,
    /// `last_sustain`: (system, owner, unit type, producer, flag).
    pub last_sustain: Option<(System, Seat, Unit, Seat, bool)>,
    pub pending_destructions: SmallList<PendingDestruction, 2>,
    pub last_ship_destroyed: Option<(System, Seat, Unit)>,
    pub last_control_gained: Option<(System, Planet, Seat, Option<Seat>)>,
    /// `last_combat_sides`: the system and the sides in order.
    pub last_combat_sides: Option<(System, SmallList<Seat, 4>)>,
    pub active_space_combat: Option<(System, Seat, Seat)>,
    pub combat_round_hits: SortedMap<Seat, u32, 4>,
    pub combat_round_dice: Log<CombatDie>,
    pub presentation: Cow<CombatPresentation>,
    pub pending_reflective_hits: Option<(System, Seat, u32)>,
    pub last_action_discarded: Option<(Seat, ActionCard)>,
}
