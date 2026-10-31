//! The neutral force's reference card (Thunder's Edge rule 7): "when a hit is assigned against a group of neutral units, it is assigned to the
//! unit that is lowest on the neutral unit reference card". The neutral owner is an owner without a seat (`seats::is_player` is false for it); the
//! areas that assign hits (the combat's casualties, the invasion's ground hits) ask here which unit goes.

use crate::generated::Unit;
use crate::state::Piece;

/// The neutral unit reference card, lowest first (`neutral_units::roster`, TE rule 7: a hit assigned against neutral units goes to the
/// unit lowest on the card). The engine pins this order against the transcribed records (`neutral_units::tests`).
pub const NEUTRAL_LOWEST_FIRST: [Unit; 11] = [
    Unit::NeutralSpaceDock,
    Unit::NeutralPDS,
    Unit::NeutralInfantry,
    Unit::NeutralMech,
    Unit::NeutralFighter,
    Unit::NeutralDestroyer,
    Unit::NeutralCruiser,
    Unit::NeutralCarrier,
    Unit::NeutralDreadnought,
    Unit::NeutralWarSun,
    Unit::NeutralFlagship,
];

/// The index of the candidate a neutral side loses to a hit (`neutral_units::next_casualty` over the roster, else the first):
/// the first candidate of the type lowest on the reference card. `None` for no candidate.
#[must_use]
pub fn neutral_casualty(kinds: &[Unit]) -> Option<usize> {
    NEUTRAL_LOWEST_FIRST
        .iter()
        .find_map(|rank| kinds.iter().position(|k| k == rank))
        .or_else(|| (!kinds.is_empty()).then_some(0))
}

/// The piece a neutral side loses to a hit among `targets` (rule 7): see [`neutral_casualty`].
#[must_use]
pub fn neutral_pick(targets: &[Piece]) -> Option<Piece> {
    let kinds: Vec<Unit> = targets.iter().map(|p| p.kind()).collect();
    neutral_casualty(&kinds).map(|at| targets[at])
}
