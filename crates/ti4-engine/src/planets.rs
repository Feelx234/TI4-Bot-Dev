//! Which planets are in a system, printed and placed.
//!
//! The corpus answers this for the 200-odd planets printed on tiles. Twelve are not: Mirage,
//! Custodia Vigilia and the ocean planets have a null `tileId` because they arrive from a deck
//! during play. `GameState::placed_planets` records where those went, and this module is the union
//! of the two — the one place a caller should ask, so a card that places a planet does not have to
//! find every reader and teach it about the overlay.

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, SystemId};
use ti4_model::state::GameState;

/// Every planet in this system: printed on the tile, plus any placed there during play.
#[must_use]
pub fn in_system(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
) -> Vec<PlanetId> {
    let mut found: Vec<PlanetId> =
        ti4_content::galaxy::planets_in(content, system.as_str(), sources)
            .into_iter()
            .map(|planet| PlanetId::new(planet.id()))
            .collect();
    found.extend(
        state
            .placed_planets
            .iter()
            .filter(|(_, where_it_went)| *where_it_went == system)
            .map(|(planet, _)| planet.clone()),
    );
    found
}

/// The system a planet sits in: a placed planet's recorded tile, else its printed tile, else
/// whichever board system has it in `planet_units` / `planet_control`. `None` when none knows.
///
/// Used to locate a planet answer on the map (`payload.system`); it decides nothing.
#[must_use]
pub fn system_of(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    planet: &str,
) -> Option<SystemId> {
    let id = PlanetId::new(planet);
    if let Some(system) = state.placed_planets.get(&id) {
        return Some(system.clone());
    }
    if let Some(system) =
        ti4_content::galaxy::planet(content, planet, sources).and_then(|record| record.system_id())
    {
        return Some(SystemId::new(system));
    }
    state
        .board
        .iter()
        .find(|(_, board)| {
            board.planet_units.contains_key(&id) || board.planet_control.contains_key(&id)
        })
        .map(|(system, _)| system.clone())
}

/// Put a planet that has no printed tile onto one, and give its card to a player.
///
/// The planet arrives readied and controlled (LRR: a planet card gained this way is gained
/// readied). Returns `false` if it is already on the board, so a card cannot place it twice.
pub fn place(
    state: &mut GameState,
    system: &SystemId,
    planet: &PlanetId,
    player: &ti4_model::id::PlayerId,
) -> bool {
    if state.placed_planets.contains_key(planet) {
        return false;
    }
    state.placed_planets.insert(planet.clone(), system.clone());
    state.board.entry(system.clone()).or_default();
    if let Some(here) = state.board.get_mut(system) {
        here.set_control(planet.clone(), player.clone());
    }
    state.exhausted_planets.remove(planet);
    true
}

/// A planet's technology specialties as they now stand: those printed on its card plus any an
/// attachment gave it (a research facility on a planet without one, LRR 35.8), lower-case.
///
/// Every rules question about specialties — prerequisites (90.8), specialty-exhausting abilities,
/// "control planets with specialties" — asks here, so an attached specialty is never invisible.
#[must_use]
pub fn tech_specialties_now(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    planet: &PlanetId,
) -> Vec<String> {
    let mut found: Vec<String> = ti4_content::galaxy::planet(content, planet.as_str(), sources)
        .map(|record| {
            record
                .tech_specialties()
                .into_iter()
                .map(str::to_ascii_lowercase)
                .collect()
        })
        .unwrap_or_default();
    for id in state.planet_attachments.get(planet).into_iter().flatten() {
        let Some(record) = content.get(ti4_model::content_types::ContentType::Attachments, id)
        else {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A placed planet is in its system, and the printed ones are still there too.
    #[test]
    fn a_placed_planet_joins_the_printed_ones() {
        let content = ContentStore::embedded();
        let sources = ti4_model::content_types::DEFAULT;
        let player = ti4_model::id::PlayerId::new("a");
        let mut state = crate::fixtures::game(&["a"]);

        let (system, printed) = crate::fixtures::a_placed_planet();
        let before = in_system(&state, content, sources, &system);
        assert!(before.contains(&printed), "the printed planet is there");

        let mirage = PlanetId::new("mirage");
        assert!(
            !before.contains(&mirage),
            "and Mirage is not, until it is placed"
        );

        assert!(place(&mut state, &system, &mirage, &player));
        let after = in_system(&state, content, sources, &system);
        assert!(after.contains(&mirage), "now it is");
        assert!(after.contains(&printed), "and the printed one still is");
        assert!(
            !place(&mut state, &system, &mirage, &player),
            "and it cannot be placed twice"
        );
    }

    /// `system_of`: placed planets by their recorded tile, printed ones by the corpus, and a
    /// planet nobody knows about by nothing at all.
    #[test]
    fn system_of_reads_placement_then_corpus_then_board() {
        let content = ContentStore::embedded();
        let sources = ti4_model::content_types::POK;
        let player = ti4_model::id::PlayerId::new("a");
        let mut state = crate::fixtures::game(&["a"]);
        assert_eq!(
            system_of(&state, content, sources, "lodor"),
            Some(SystemId::new("26"))
        );
        assert_eq!(system_of(&state, content, sources, "not_a_planet"), None);

        let (system, _) = crate::fixtures::a_placed_planet();
        assert!(place(
            &mut state,
            &system,
            &PlanetId::new("mirage"),
            &player
        ));
        assert_eq!(system_of(&state, content, sources, "mirage"), Some(system));

        let board_only = SystemId::new("board_only_system");
        state
            .system_mut(&board_only)
            .set_control(PlanetId::new("board_only_planet"), player);
        assert_eq!(
            system_of(&state, content, sources, "board_only_planet"),
            Some(board_only)
        );
    }
}
