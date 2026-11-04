//! Technology facts for the product (`technology.rs` of the legacy crate): printed data by alias, answered on the typed core's tables and,
//! for an alias the tables do not know, from the content records exactly as the legacy does. The research-position queries are the core's.

use std::collections::BTreeSet;

use ti4_content::ContentStore;
use ti4_model::content_types::{ContentType, SourceSet};
use ti4_model::id::{PlayerId, TechnologyId};
use ti4_model::state::GameState;

use crate::ties::technology_core;

pub use crate::ties::technology::{COLOURS, colour_of, colour_type, is_unit_upgrade, name, prerequisites};

/// Technologies in the authoritative current PoK/Codex deck (plus the Thunder's Edge faction technologies when that is in play).
#[must_use]
pub fn active_aliases(_content: &ContentStore, sources: SourceSet) -> BTreeSet<TechnologyId> {
    technology_core::active_aliases(sources)
}

/// The faction a technology belongs to, if it is faction-specific (90.11).
#[must_use]
pub fn faction_of<'a>(content: &'a ContentStore, alias: &TechnologyId) -> Option<&'a str> {
    technology_core::faction_of(alias).unwrap_or_else(|| content.get(ContentType::Technologies, alias.as_str()).and_then(|record| record.text("faction")).filter(|faction| !faction.is_empty()))
}

/// Whether this faction has its own upgrade standing in for `alias`, making `alias` unresearchable for them (90.11).
#[must_use]
pub fn replaced_for_faction(content: &ContentStore, faction: &str, alias: &TechnologyId) -> bool {
    technology_core::replaced_for_faction(faction, alias).unwrap_or_else(|| {
        !faction.is_empty()
            && content.records(ContentType::Technologies).iter().any(|record| record.text("faction") == Some(faction) && record.text("baseUpgrade") == Some(alias.as_str()))
    })
}

/// Whether Gravity Drive may still raise one ship's move in this tactical action.
#[must_use]
pub fn gravity_drive_available(state: &GameState, player: &PlayerId) -> bool {
    let Some(seat) = state.player(player) else {
        return false;
    };
    seat.technologies.contains(ti4_model::id::text(&"gd")) && seat.gravity_drive_used_activation != Some(state.activation_seq)
}

/// The technologies `player` could research with the prerequisites met, for a seat that has played nothing (the pick of a starting technology,
/// `StartingTechSource::Researchable`: the Winnu). Empty when the seat holds something the typed tables do not know.
#[must_use]
pub fn researchable_by_prerequisites(state: &GameState, _content: &ContentStore, sources: SourceSet, player: &PlayerId) -> Vec<TechnologyId> {
    technology_core::setup_researchable_by_prerequisites(state, sources, player).unwrap_or_default()
}
