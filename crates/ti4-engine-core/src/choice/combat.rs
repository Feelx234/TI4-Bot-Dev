//! Decision types of the `combat` area: space and ground combat rounds, hits, retreats, bombardment.
//!
//! Owner: package B1 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far: the offer to cancel a hit with
//! SUSTAIN DAMAGE (LRR 87). The adapter side is `ti4-engine/src/render/combat.rs`.

use crate::generated::{Planet, System, Unit};

/// Where a unit that may sustain stands: in the space area (its index in the system's unit list) or on
/// a planet (the Eidolon Maximum, or a ground force an Alastor brought into the battle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SustainSite {
    Space(u16),
    /// `variant`: the unit is named by its index on the planet (`planet:{p}:variant:{i}`) rather than
    /// by the planet alone.
    Planet { planet: Planet, index: u16, variant: bool },
}

/// What the area asks. Parameters only; the option list is derived by `opts(state, ask)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// "cancel a hit at {system}": `hits` are still owed.
    CancelHit {
        system: System,
        hits: u16,
        /// The loss is bound to non-fighter ships and one exists (Graviton Laser System): display flag
        /// of the decline option.
        non_fighters_first: bool,
        /// `supply::staging_enabled`: planet-standing sustainers are named by planet and each is offered.
        staging: bool,
        /// The hit is a unit ability's (Naaz Maximum cannot be assigned it).
        ability: bool,
        /// The hit is part of a space combat (not SPACE CANNON OFFENSE).
        in_combat: bool,
    },
    /// "assign a hit": the owner picks which of the listed units is lost. The list is the caller's
    /// (ships, a group, a planet's units); the options are derived from it by `casualty_opts`.
    AssignHit,
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Sustain damage on this unit of the given type; the decline is the generic one ("take the hit").
    Sustain { site: SustainSite, unit: Unit, galvanized: bool },
    /// Destroy the unit at `index` of the caller's list (the first of its type and damage).
    Destroy { index: u16, unit: Unit, damaged: bool },
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    SustainDamage,
    /// The caller's subtype text (interned) of a casualty question.
    Casualty(crate::state::Code),
}
