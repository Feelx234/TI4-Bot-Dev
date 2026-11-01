//! From `planets.rs` of the legacy crate.
use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, SystemId};
use ti4_model::state::GameState;

/// The system a planet sits in: placed by a card, else its printed tile, else wherever the board holds it. Used to locate a planet
/// answer on the map (`payload.system`); it decides nothing. (The legacy's `system_of_text`; the typed twin answered the same.)
#[must_use]
pub fn system_of(state: &GameState, content: &ContentStore, sources: SourceSet, planet: &str) -> Option<SystemId> {
    let id = PlanetId::new(planet);
    if let Some(system) = state.placed_planets.get(&id) {
        return Some(system.clone());
    }
    if let Some(system) = ti4_content::galaxy::planet(content, planet, sources).and_then(|record| record.system_id()) {
        return Some(SystemId::new(system));
    }
    state
        .board
        .iter()
        .find(|(_, board)| board.planet_units.contains_key(&id) || board.planet_control.contains_key(&id))
        .map(|(system, _)| system.clone())
}
