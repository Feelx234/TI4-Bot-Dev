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

/// Move a placed planet together with its controller, units and coexistence records.
/// Planet-keyed exhaustion, attachments and legendary-card state keep their identities.
///
/// # Panics
/// If the destination system disappears from the board between the check here and the move.
pub fn move_placed(state: &mut GameState, planet: &PlanetId, destination: &SystemId) -> bool {
    let Some(origin) = state.placed_planets.get(planet).cloned() else {
        return false;
    };
    if origin == *destination || !state.board.contains_key(destination) {
        return false;
    }
    let Some(here) = state.board.get_mut(&origin) else {
        return false;
    };
    if here.purged_planets.contains(planet) {
        return false;
    }
    let control = here.planet_control.remove(planet);
    let units = here.planet_units.remove(planet);
    let coexist = here.coexisting.remove(planet);
    let there = state
        .board
        .get_mut(destination)
        .expect("destination checked");
    if let Some(owner) = control {
        there.planet_control.insert(planet.clone(), owner);
    }
    if let Some(units) = units {
        there.planet_units.insert(planet.clone(), units);
    }
    if let Some(owners) = coexist {
        there.coexisting.insert(planet.clone(), owners);
    }
    state
        .placed_planets
        .insert(planet.clone(), destination.clone());
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

    #[test]
    fn relocating_a_planet_keeps_every_planet_record_and_rejects_invalid_moves() {
        let mut state = crate::fixtures::game(&["a", "b"]);
        let (from, to) = (SystemId::new("19"), SystemId::new("20"));
        let planet = PlanetId::new("avernus");
        let owner = ti4_model::id::PlayerId::new("a");
        place(&mut state, &from, &planet, &owner);
        crate::fixtures::put_on_planet(&mut state, &from, &planet, "infantry", &owner, 1);
        state
            .system_mut(&from)
            .coexisting
            .entry(planet.clone())
            .or_default()
            .insert(ti4_model::id::PlayerId::new("b"));
        state.exhausted_planets.insert(planet.clone());
        state
            .player_mut(&owner)
            .unwrap()
            .exhausted_legendary
            .insert(planet.clone());
        assert!(!move_placed(&mut state, &planet, &to));
        state.board.entry(to.clone()).or_default();
        assert!(move_placed(&mut state, &planet, &to));
        assert_eq!(state.board[&to].planet_control.get(&planet), Some(&owner));
        assert_eq!(state.board[&to].on_planet(&planet).len(), 1);
        assert!(state.board[&to].coexisting[&planet].contains(&ti4_model::id::PlayerId::new("b")));
        assert!(state.exhausted_planets.contains(&planet));
        assert!(
            state
                .player(&owner)
                .unwrap()
                .exhausted_legendary
                .contains(&planet)
        );
        assert!(!move_placed(&mut state, &planet, &to));
    }

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
}
