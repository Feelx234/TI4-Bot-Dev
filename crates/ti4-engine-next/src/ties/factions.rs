//! From `factions/` of the legacy crate (copies): the constants of the faction modules the renderers name, and the two facts of the
//! module table (`factions::MODULES`) that the glue reads. `tests/ties_parity.rs` compares the tables with the legacy ones.

use serde::{Deserialize, Serialize};
use ti4_model::id::{PlanetId, PlayerId, SystemId, UnitTypeId};

/// The aliases of the factions that have a string module, in `factions::MODULES` order: defined once, in `crate::builders::factions` (lane C).
pub use crate::builders::factions::MODULE_ALIASES;

/// The modules with the `secondary_waivers` hook, in module order: the position rides in the id of a waiver option.
pub const WAIVER_MODULES: [&str; 4] = ["mahact", "naalu", "obsidian", "winnu"];

/// Whether a faction has a string module (`factions::MODULES`).
#[must_use]
pub fn has_module(alias: &str) -> bool {
    MODULE_ALIASES.contains(&alias)
}

/// The position of the module `alias` among the modules that have the `secondary_waivers` hook: the index that rides in the id of a waiver
/// option (`follow|waived|<index>|<waiver id>`). `None`: the module has no such hook.
#[must_use]
pub fn waiver_module_index(alias: &str) -> Option<usize> {
    WAIVER_MODULES.iter().position(|module| *module == alias)
}

/// Revenant (crimson), the mech.
pub const CRIMSON_MECH: &str = "crimson_mech";
/// The pseudo-system of a placement spot that puts a produced ship onto the hero card.
pub const CRIMSON_CARD_SYSTEM: &str = "crimsoncard";
/// The follow option that performs Leadership's primary ability instead of its secondary (Enervate).
pub const PRIMARY_INSTEAD_ID: &str = "follow|enervate_primary";
/// Prefix of the Mahact agent's waiver ids: `agent|<system>`.
pub const MAHACT_AGENT_WAIVER_PREFIX: &str = "agent|";
/// Prefix of the Naalu Mindsieve waiver ids.
pub const NAALU_MINDSIEVE_PREFIX: &str = "ms|";

/// A Ssruu / round agent bonus (`borrowed_round_agents::RoundBonus`), as `faction_marks` holds it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoundBonus {
    pub borrower: PlayerId,
    pub source_owner: PlayerId,
    pub source_agent: String,
    pub system: SystemId,
    /// Ground-combat planet named by the triggering event (the actual battle).
    pub combat_planet: Option<PlanetId>,
    /// Location of the selected unit. This can differ from `combat_planet` for Sol.
    pub unit_planet: Option<PlanetId>,
    pub unit_owner: PlayerId,
    pub unit_type: UnitTypeId,
    pub sustained_damage: bool,
    pub galvanized: bool,
    /// Ordinal in the owner's complete eligible roll roster at this location.
    pub unit_index: usize,
    pub round_seq: u32,
}

/// What a player calls the system in a prompt: its planets ("Mecatol Rex", "Bakal and Alio Prima"), else the tile number. The tile
/// number follows, so two systems never read alike. (`mahact::system_name_of`: the render of the core's Jae Mir Kan waiver.)
#[must_use]
pub fn mahact_system_name_of(system: &str) -> String {
    let content = ti4_content::ContentStore::embedded();
    let planets: Vec<String> = ti4_content::galaxy::planets_in(content, system, ti4_model::content_types::POK)
        .iter()
        .filter_map(|planet| planet.name().map(str::to_owned))
        .collect();
    if planets.is_empty() {
        format!("system {system}")
    } else {
        format!("{} (system {system})", planets.join(" and "))
    }
}
