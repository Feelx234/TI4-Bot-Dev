//! The typed payload of an open tactical action (package A2b; the `Pending` window of D1b): what `Game.tactical`
//! holds, without text. The hold's candidates are not stored: they are a function of the position
//! ([`crate::query::transit::Hold::for_ship`]), only what is aboard is.

use crate::containers::SmallList;
use crate::generated::System;
use crate::ids::Seat;

use super::{NoteKey, Piece};

/// Where an open tactical action has reached (the string `TacticalStage`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TStage {
    /// Choosing the system to activate (89.1).
    Activating,
    /// Choosing a ship to move, or finishing (89.2).
    Moving,
    /// Filling the hold of `ship`, which moves from `origin` along `path` (origin first).
    Loading { origin: System, ship: Piece, path: SmallList<System, 8>, loaded: SmallList<u16, 12>, closed: bool },
}

/// An open tactical action (`TacticalWindow`): whose, where it stands, and the promissory notes held when it began
/// (`combat::note_holdings`: the "after combat" secrets read them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tactical {
    pub player: Seat,
    pub stage: TStage,
    pub notes_at_start: SmallList<(NoteKey, Seat), 12>,
}
