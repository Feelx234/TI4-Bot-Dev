//! Cargo facts for the product (copy of the reads of `transit.rs` of the legacy crate): what a ship leaving a system could take aboard, and the
//! capacity of a unit. The structure transport of Ral Nel (a dropped faction, refused at creation) is not carried.

use ti4_content::ContentStore;
use ti4_content::units::{UnitType, catalogue};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId};
use ti4_model::state::GameState;
use ti4_model::units::Unit;

pub use crate::kinds::transit::{LOAD_KIND, RIFT_DESTROYS_ON};

/// One unit in a ship's hold, paired with where it was picked up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cargo {
    pub unit: Unit,
    pub source: CargoSource,
    /// The system this unit was picked up from.
    pub system: SystemId,
}

/// Where a carried unit came from, so a lost ship can put it back.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CargoSource {
    /// The space area of the origin system.
    Space,
    /// A planet in the origin system.
    Planet(PlanetId),
}

/// Every unit in `origin` this player could load, in a stable order: the space area first, then planets in id order.
#[must_use]
pub fn loadable(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, origin: &SystemId) -> Vec<Cargo> {
    let types = catalogue(content, sources);
    let consumes = |unit: &Unit| types.get(unit.type_id.as_str()).is_some_and(UnitType::consumes_capacity);
    let system = state.system_state(origin);
    // 95.5: fighters and ground forces cannot be picked up from a system that holds one of their faction's command tokens other than
    // the active system.
    if state.active_system.as_ref() != Some(origin) && system.command_tokens.contains(player) {
        return Vec::new();
    }
    let mut found: Vec<Cargo> = system
        .units_of(player)
        .into_iter()
        .filter(|unit| consumes(unit))
        .map(|unit| Cargo { unit: unit.clone(), source: CargoSource::Space, system: origin.clone() })
        .collect();
    for planet in system.planet_units.keys() {
        found.extend(
            system
                .on_planet_of(planet, player)
                .into_iter()
                .filter(|unit| consumes(unit))
                .map(|unit| Cargo { unit: unit.clone(), source: CargoSource::Planet(planet.clone()), system: origin.clone() }),
        );
    }
    found
}

/// The capacity of one ship.
#[must_use]
pub fn capacity_of(content: &ContentStore, sources: SourceSet, unit: &Unit) -> i64 {
    catalogue(content, sources).get(unit.type_id.as_str()).map_or(0, UnitType::capacity)
}
