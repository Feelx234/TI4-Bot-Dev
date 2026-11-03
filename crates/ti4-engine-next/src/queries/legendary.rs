//! `legendary.rs` of the legacy crate, as far as the product surface uses it. Plain reads of the public state, copied.

use ti4_model::id::{PlanetId, PlayerId};
use ti4_model::state::GameState;

/// Tempesta, whose card is spent from inside the movement step rather than from a window.
const IONIAN: &str = "tempesta";

/// Whether this seat holds the planet and has not spent its ability card this round.
#[must_use]
pub fn available(state: &GameState, player: &PlayerId, planet: &PlanetId) -> bool {
    state.controlled_planets(player).into_iter().any(|(_, held)| held == planet) && state.player(player).is_some_and(|seat| !seat.exhausted_legendary.contains(planet))
}

/// Whether the Ionian Fuel Refinery could add +1 to one ship's move right now: the card is the extra reach, so it is offered as the move it
/// makes possible.
#[must_use]
pub fn ionian_available(state: &GameState, player: &PlayerId) -> bool {
    available(state, player, &PlanetId::new(IONIAN))
}
