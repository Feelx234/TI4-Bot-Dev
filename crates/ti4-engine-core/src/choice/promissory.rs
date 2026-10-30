//! Decision types of the `promissory` area: promissory notes, and the placement of units from the reinforcements that a note or an
//! ability asks (`action_cards::place_units_choosing`; package W14, lent-bodies lane 5).
//!
//! The adapter side is `ti4-engine/src/render/promissory.rs`.

use crate::generated::{Planet, System, Unit};

/// What the area asks. Parameters only; the option list is derived by the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// "place up to {fit} {unit}": where the units from the reinforcements go (the planets the placer controls). `unit` is the form the
    /// seat places (the prompt and the facts read its base type); `fit` is how many go to the one spot chosen.
    PlaceUnits { unit: Unit, fit: u8 },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Place the units on this planet of this system.
    Spot(System, Planet),
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// `place_units_from_reinforcements`.
    PlaceUnitsFromReinforcements,
}
