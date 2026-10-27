//! `AgendaState`: laws, the agenda in play, votes, choices, override slots, logs.

use crate::containers::{Log, SmallList, SortedMap};
use crate::generated::Agenda;
pub use crate::keys::{Outcome, Prediction};
use crate::ids::Seat;

use super::Code;

/// `AgendaRecord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgendaRec {
    pub round: u32,
    pub agenda: Agenda,
    pub outcome: Option<Outcome>,
    pub votes: SortedMap<Outcome, i64, 6>,
    pub speaker_decided: bool,
    pub law: bool,
    pub enacted: bool,
    pub discarded: bool,
    pub redirected_to: Option<Outcome>,
    pub discard_reason: Option<Code>,
}

/// `AgendaCast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgendaCastRec {
    pub player: Seat,
    pub outcome: Option<Outcome>,
    pub votes: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgendaState {
    /// `GameState.laws`: law -> its recorded choice.
    pub laws: SortedMap<Agenda, Outcome, 16>,
    pub in_play: Option<Agenda>,
    pub choices: SmallList<Outcome, 8>,
    pub veto_replacement: Option<Agenda>,
    pub covert: Option<Agenda>,
    pub elected_override: Option<Seat>,
    pub votes: SortedMap<Seat, Outcome, 8>,
    pub predictions: SortedMap<Seat, Prediction, 8>,
    pub log: Log<AgendaRec>,
    pub casts: Log<AgendaCastRec>,
}
