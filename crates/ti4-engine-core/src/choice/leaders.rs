//! Decision types of the `leaders` area: leaders, faction abilities, hero and agent plumbing.
//!
//! Owner: package B7 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far, by the driver package D1 as the
//! worked example of a window (docs/ENGINE_CORE_DRIVER.md): the Rear Admiral Farran offer. The adapter
//! side is `ti4-engine/src/render/leaders.rs`.

use crate::generated::{Leader, Planet, Unit};
use crate::ids::Seat;

/// The unit a round agent (Evelyn De Louis, Viscount Unlenn) gives an extra combat die: its owner, where it
/// stands (`None`: in the fleet), its type, its ordinal in the owner's roll roster there and the two marks the
/// signature checks (the string `UnitTarget`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoundTarget {
    pub owner: Seat,
    pub planet: Option<Planet>,
    pub unit: Unit,
    pub index: u8,
    pub sustained: bool,
    pub galvanized: bool,
}

/// What the area asks. Parameters only; the option list is derived by `opts(state, ask)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Rear Admiral Farran (Letnev commander): "After 1 of your units uses SUSTAIN DAMAGE: you may gain 1
    /// trade good". The actor is the commander's holder.
    SustainCommander,
    /// A round agent: "choose 1 unit in the active system: it rolls 1 additional die during this combat round".
    RoundAgentUnit { agent: Leader },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Gain the trade good.
    GainTradeGood,
    /// This unit rolls the additional die.
    RoundAgentUnit(RoundTarget),
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    SustainCommander,
    RoundAgentUnit,
}
