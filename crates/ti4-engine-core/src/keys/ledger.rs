//! Why a victory point moved: the reason column of `GameState::vp_ledger`.

use crate::generated::{Agenda, Relic};

/// The source of a ledger row (`objectives::adjust_victory_points` and its callers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LedgerReason {
    Custodians,
    ImperialPrimary,
    ImperialRider,
    Objective,
    /// The host's preset starting points.
    PresetStart,
    SecretObjective,
    Styx,
    SupportForTheThrone,
    /// An agenda (`agenda:<alias>`): the law that gave or took the point.
    Agenda(Agenda),
    /// A relic that scores by itself (its id).
    Relic(Relic),
}
