//! Decision types of the `technology` area: the free technology windows of a turn (package W12): Psychoarchaeology at the
//! start of a turn, Predictive Intelligence and Bio-Stims at its end. (The research questions are the strategy area's.)
//! The adapter side is `ti4-engine/src/render/technology.rs`.

use super::agenda::Shown;
use super::strategy::Pool;
use crate::generated::{Planet, Technology};

/// What the area asks. Parameters only; the options are the window's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// "Psychoarchaeology: exhaust a specialty for 1 trade good".
    Psychoarchaeology,
    /// "Predictive Intelligence: redistribute command tokens".
    Restack,
    /// "Bio-Stims".
    BioStims,
    /// "AI Development Algorithm: exhaust to reduce this use's combined cost by {upgrades}".
    Aida { upgrades: u8 },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// A specialty planet to exhaust (`exhaust {planet}`) or, for Bio-Stims, to ready; `Shown::system` is its place on the map.
    Planet(Planet, Shown),
    /// Move one token from a pool to another (`{from}|{to}`).
    Move { from: Pool, to: Pool },
    /// Bio-Stims readies an exhausted technology.
    ReadyTechnology(Technology),
    /// Exhaust AI Development Algorithm for the discount.
    Exhaust,
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    PsychoarchaeologyExhaustSpecialty,
    PredictiveIntelligenceRedistribute,
    BioStimsReady,
    AidaExhaustForProductionDiscount,
}
