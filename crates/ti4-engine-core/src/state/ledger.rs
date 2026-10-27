//! `Ledger`: append-only typed logs (events, dice history, the victory point ledger, relic gains).
//! A clone shares every prefix. The decision log (`DecisionRecord`: prompts, offered options) stays in
//! the adapter: it is the rendered, hashed form.

use crate::containers::{Log, SmallList};
use crate::generated::{Planet, Relic, System, Unit};
pub use crate::keys::LedgerReason;
use crate::ids::Seat;

use super::Code;

/// Kind of an emitted event; numbered by package A5 (`event_type` texts live in the adapter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventKind(pub u16);

/// `Event { id, event_type, payload, cancelled }` with a typed kind and up to three small arguments
/// (seats, ids by index, counts); A5 defines the argument layout per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventRec {
    pub id: u64,
    pub kind: EventKind,
    pub args: [u32; 3],
    pub cancelled: bool,
}

/// Why dice were rolled (`Roll.reason`, about 25 literals). B1 extends the closed list; `Other` is an
/// adapter-interned reason text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RollPurpose {
    SpaceCombat,
    GroundCombat,
    AntiFighterBarrage,
    SpaceCannon,
    Bombardment,
    Reroll,
    Other(Code),
}

/// One roll of a group of dice (`dice::Roll`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollRec {
    pub purpose: RollPurpose,
    pub owner: Option<Seat>,
    pub unit: Option<Unit>,
    pub planet: Option<Planet>,
    pub scene: Option<System>,
    pub hits_on: Option<u8>,
    pub faces: SmallList<u8, 8>,
    /// Bit `i`: die `i` replaced by a reroll.
    pub rerolled: u64,
}

/// `vp_ledger` row: (player, change, reason).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VpRec {
    pub player: Seat,
    pub delta: i32,
    pub reason: LedgerReason,
}

/// `RelicGainRecord`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelicRec {
    pub round: u32,
    pub player: Seat,
    pub relic: Relic,
    pub victory_points: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ledger {
    pub events: Log<EventRec>,
    pub dice: Log<RollRec>,
    pub vp: Log<VpRec>,
    pub relics: Log<RelicRec>,
}
