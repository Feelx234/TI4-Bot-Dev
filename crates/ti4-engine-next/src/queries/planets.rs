//! `planets.rs` of the legacy crate, as far as the product surface uses it: the planets of a system and the traits and specialties a planet has
//! now (printed, plus its attachments). Plain reads of the public state and the printed records, copied.

use ti4_content::ContentStore;
use ti4_model::content_types::{ContentType, SourceSet};
use ti4_model::id::{PlanetId, SystemId};
use ti4_model::state::GameState;

/// Every planet in this system: printed on the tile, plus any placed there during play.
#[must_use]
pub fn in_system(state: &GameState, content: &ContentStore, sources: SourceSet, system: &SystemId) -> Vec<PlanetId> {
    let mut found: Vec<PlanetId> = ti4_content::galaxy::planets_in(content, system.as_str(), sources).into_iter().map(|planet| PlanetId::new(planet.id())).collect();
    found.extend(state.placed_planets.iter().filter(|(_, where_it_went)| *where_it_went == system).map(|(planet, _)| planet.clone()));
    found
}

/// The technology specialties printed on a planet, lower-case.
fn printed_specialties(content: &ContentStore, sources: SourceSet, planet: &PlanetId) -> Vec<String> {
    ti4_content::galaxy::planet(content, planet.as_str(), sources).map(|record| record.tech_specialties().into_iter().map(str::to_ascii_lowercase).collect()).unwrap_or_default()
}

/// The technology specialties a planet has right now: the printed ones, then the ones its attachments give (a specialty already there is
/// not listed twice). Every rules question about specialties asks here, so an attached specialty is never invisible.
#[must_use]
pub fn tech_specialties_now(state: &GameState, content: &ContentStore, sources: SourceSet, planet: &PlanetId) -> Vec<String> {
    let mut found = printed_specialties(content, sources, planet);
    for id in state.planet_attachments.get(planet).into_iter().flatten() {
        let Some(record) = content.get(ContentType::Attachments, id) else {
            continue;
        };
        for specialty in record.strings("techSpeciality") {
            let specialty = specialty.to_ascii_lowercase();
            if !found.contains(&specialty) {
                found.push(specialty);
            }
        }
    }
    found
}

/// The exploration traits a planet has right now: the printed ones, then the ones its attachments give (a frontier is no trait of a planet).
#[must_use]
pub fn traits_now(state: &GameState, content: &ContentStore, sources: SourceSet, planet: &PlanetId) -> Vec<String> {
    let mut found = super::content::traits_of(content, sources, planet);
    for id in state.planet_attachments.get(planet).into_iter().flatten() {
        let Some(record) = content.get(ContentType::Attachments, id) else {
            continue;
        };
        for kind in record.strings("planetTypes") {
            let kind = kind.to_ascii_uppercase();
            if super::content::EXPLORATION_TRAITS.contains(&kind.as_str()) && kind != "FRONTIER" && !found.contains(&kind) {
                found.push(kind);
            }
        }
    }
    found
}
