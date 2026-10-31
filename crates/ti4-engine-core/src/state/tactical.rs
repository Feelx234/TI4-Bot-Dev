//! The typed payload of an open tactical action (package A2b; the `Pending` window of D1b): what `Game.tactical`
//! holds, without text. The hold's candidates are not stored: they are a function of the position
//! ([`crate::query::transit::Hold::for_ship`]), only what is aboard is.

use crate::containers::SmallList;
use crate::generated::System;
use crate::ids::Seat;

use super::{NoteHold, Piece};

/// Where an open tactical action has reached (the string `TacticalStage`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TStage {
    /// Choosing the system to activate (89.1).
    Activating,
    /// Choosing a ship to move, or finishing (89.2).
    Moving,
    /// Filling the hold of `ship`, which moves from `origin` along `path` (origin first).
    Loading { origin: System, ship: Piece, path: SmallList<System, 8>, loaded: SmallList<u16, 12>, closed: bool },
    /// A free tactical action (Thunder's Edge Warfare, Z'eu) whose activation of the system has not run yet. LIVE only.
    Free(System),
    /// The activation has happened (`SYSTEM_ACTIVATED` is out): the hook chain of the activation runs before the movement step. LIVE only.
    Hooks,
    /// The movement step is over; its close (`MOVEMENT_FINISHED`, the hero's skip, the blockade) runs on the next step. LIVE only:
    /// the string window is gone by then (`finish_tactical_inner` takes it).
    Closing,
    /// The head of the aftermath (`AftermathWindow::head`): the fleet enforcement, then the space cannon offense. LIVE only.
    Heading(THead),
    /// The aftermath is open above this entry on the stack (`Window::Aftermath`); this entry waits for it to leave and then runs the
    /// tail. LIVE only.
    Aftermath,
    /// The tactical tail (`Game::close_tactical`): the Crown of Emphidia, Dark Energy Tap, the listeners of the end of the action,
    /// the closing writes. LIVE only.
    Tail,
}

/// Where the head of the aftermath has reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct THead {
    /// The action skips to the Commit Ground Forces step (the Sardakk hero): no space cannon, no fight.
    pub skip_to_commit: bool,
    pub step: THeadStep,
}

/// The suspension points of the head. The enforcement derives what it asks from the position; the cannon is not asked
/// mid-absorption (a bail of the live path, `Gap::CannonHits`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum THeadStep {
    /// The active seat removes the units over its fleet supply, then over capacity.
    Enforcing,
    /// The space cannon offense (it runs in one step unless a hit needs a question).
    Firing,
}

/// An open tactical action (`TacticalWindow`): whose, where it stands, and the promissory notes held when it began
/// (`combat::note_holdings`: the "after combat" secrets read them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tactical {
    pub player: Seat,
    pub stage: TStage,
    pub notes_at_start: NoteHold,
}
