//! Decision types of the `relic` area: the questions a relic puts to its holder.
//!
//! Owner: package B5 (docs/ENGINE_CORE_STATE.md 4.4). Ported: the Dominus Orb's purge offer and the Crown of
//! Emphidia's choice of a planet to explore. The adapter side is `ti4-engine/src/render/relic.rs`.

use super::agenda::Shown;
use crate::generated::Planet;

/// What the area asks. Parameters only; the options are the window's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// "The Dominus Orb: purge to move out of your own command tokens".
    DominusOrb,
    /// "The Crown of Emphidia: exhaust to explore a planet".
    CrownOfEmphidia,
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Purge the Dominus Orb.
    Purge,
    /// A planet to explore, with the system the map shows it in (the `system` payload of the option).
    Planet(Planet, Shown),
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    DominusOrbPurgeToMove,
    CrownOfEmphidiaChoosePlanet,
}
