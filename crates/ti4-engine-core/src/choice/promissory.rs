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
    /// ???_ERROR_ERROR_???, the PDS: "choose a planet you control" (the planets under the structure cap).
    C4rPds,
    /// Orbital Drop (Sol): "onto which planet" (any planet the seat controls).
    OrbitalPlanet,
    /// Orbital Drop: the optional mech for 3 resources on the planet (its system, the planet and the mech form the string names).
    OrbitalDeploy { system: System, planet: Planet, mech: Unit },
    /// Star Forge / The Nucleus (Muaat): "place which units, and where" (a war sun's system, two fighters or one destroyer).
    StarForge,
    /// Ember Colossus (Muaat mech): "place 1 infantry with this mech" (the mech's system and planet, `None`: its space area).
    EmberColossus { system: System, planet: Option<Planet> },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Place the units on this planet of this system.
    Spot(System, Planet),
    /// Orbital Drop: deploy the mech for 3 resources.
    Deploy,
    /// Star Forge: place the units in this system: two fighters (`fighters`) or one destroyer.
    Forge { system: System, fighters: bool },
    /// Ember Colossus: place the infantry with the mech (the one option the question has besides the decline).
    Ember,
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// `place_units_from_reinforcements`.
    PlaceUnitsFromReinforcements,
    /// `c4r_planet`.
    C4rPlanet,
    /// `orbital_drop_choose_planet`.
    OrbitalPlanet,
    /// `orbital_drop_deploy_mech`.
    OrbitalDeploy,
    /// `star_forge`.
    StarForge,
    /// `ember_colossus`.
    EmberColossus,
}
