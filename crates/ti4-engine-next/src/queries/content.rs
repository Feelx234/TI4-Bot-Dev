//! Content-only and score-board reads of the surface: `exploration::{traits_of, attachment_for, resolution}`, `objectives::{leader, winner,
//! points_for}`, `secrets::scored_count` (copies of the text halves of the legacy functions: the printed records answer them).

use ti4_content::ContentStore;
use ti4_model::content_types::{ContentType, SourceSet};
use ti4_model::id::{PlanetId, PlayerId};
use ti4_model::state::GameState;

pub use crate::builders::objectives::points_for;
/// The four exploration decks in the oracle's construction order: defined once, in `crate::deck` (lane C).
pub use crate::deck::EXPLORATION_TRAITS;

const FRONTIER: &str = "FRONTIER";

/// Every exploration deck a planet could be explored into, in the planet's own printed trait order (35.2b).
#[must_use]
pub fn traits_of(content: &ContentStore, sources: SourceSet, planet: &PlanetId) -> Vec<String> {
    let catalogue = ti4_content::galaxy::all_planets(content, sources);
    let Some(record) = catalogue.get(planet.as_str()) else {
        return Vec::new();
    };
    record
        .planet_types()
        .into_iter()
        .filter_map(|kind| {
            let trait_name = kind.to_ascii_uppercase();
            EXPLORATION_TRAITS.iter().find(|known| **known == trait_name && **known != FRONTIER).map(|known| (*known).to_owned())
        })
        .collect()
}

/// The attachment an `Attach` explore card puts on `planet`.
#[must_use]
pub fn attachment_for(content: &ContentStore, card: &str, planet: &PlanetId) -> String {
    let id = content.get(ContentType::Explores, card).and_then(|record| record.text("attachmentId")).unwrap_or(card).to_owned();
    let stat = format!("{id}stat");
    if content.get(ContentType::Attachments, &stat).is_none() {
        return id;
    }
    let has_specialty = ti4_content::galaxy::planet(content, planet.as_str(), ti4_model::content_types::FULL).is_some_and(|record| !record.tech_specialties().is_empty());
    if has_specialty { stat } else { id }
}

/// How a card resolves, from the corpus.
#[must_use]
pub fn resolution(content: &ContentStore, card: &str) -> Option<String> {
    content.get(ContentType::Explores, card).and_then(|record| record.text("resolution")).map(ToOwned::to_owned)
}

fn first_in_initiative(state: &GameState, candidates: impl Iterator<Item = PlayerId>) -> Option<PlayerId> {
    let order = state.initiative_order();
    candidates.min_by_key(|id| order.iter().position(|seated| seated == id).unwrap_or(usize::MAX))
}

/// 98.8, 61.15a: most victory points, ties broken by initiative order.
#[must_use]
pub fn leader(state: &GameState) -> Option<PlayerId> {
    let best = state.players.iter().map(|p| p.victory_points).max()?;
    first_in_initiative(state, state.players.iter().filter(|p| p.victory_points == best).map(|p| p.id.clone()))
}

/// A winner only exists once somebody reaches the target (98).
#[must_use]
pub fn winner(state: &GameState) -> Option<PlayerId> {
    first_in_initiative(state, state.players.iter().filter(|p| p.victory_points >= crate::kinds::objectives::VICTORY_TARGET).map(|p| p.id.clone()))
}

/// How many of this player's scored objectives were secrets (45.4 counts them). A secret scored as a plot is not in `scored_objectives`.
#[must_use]
pub fn scored_count(state: &GameState, content: &ContentStore, player: &PlayerId) -> usize {
    state.scored_by(player).iter().filter(|alias| content.get(ContentType::SecretObjectives, alias.as_str()).is_some()).count()
}

/// Systems with nothing to land on, which is where a frontier token goes (35.5). Deep space and planetless anomalies both qualify; a tile
/// whose only "planets" are space stations counts as planetless (space stations rule 14).
#[must_use]
pub fn frontier_systems(content: &ContentStore, sources: SourceSet, galaxy: &ti4_content::galaxy::Galaxy) -> Vec<ti4_model::id::SystemId> {
    let mut systems: Vec<ti4_model::id::SystemId> = galaxy
        .system_ids_holding_things()
        .into_iter()
        .filter(|id| {
            ti4_content::galaxy::system(content, id, sources)
                .is_none_or(|record| record.planets().into_iter().all(|planet| ti4_content::galaxy::is_space_station(content, planet, sources)))
        })
        .map(ti4_model::id::SystemId::new)
        .collect();
    systems.sort();
    systems
}
