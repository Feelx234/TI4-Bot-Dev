//! The outcome of an agenda: what a vote is cast for, what a law stands for.

use crate::generated::{Agenda, Planet, SecretObjective, StrategyCard};
use crate::ids::Seat;

/// An agenda outcome as `GameState::laws`, `agenda_votes`, `agenda_choices` and
/// `agenda_veto_replacement` hold it (LRR 8). Which variants an agenda allows is its
/// `AgendaElects` row; the text of an outcome does not say, so the adapter's parse takes that row
/// as a hint and otherwise tries the kinds in the order of this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Outcome {
    /// The empty text: a law enacted without an elected thing (and the cleared vote).
    Empty,
    For,
    Against,
    /// A player (`Elect Player`).
    Seat(Seat),
    /// A planet (`Elect Planet`, `Elect Non-Home Planet Other Than Mecatol Rex`).
    Planet(Planet),
    /// A strategy card.
    StrategyCard(StrategyCard),
    /// A law (`Elect Law`: the alias of a law in play).
    Law(Agenda),
    /// A scored secret objective.
    Secret(SecretObjective),
}
