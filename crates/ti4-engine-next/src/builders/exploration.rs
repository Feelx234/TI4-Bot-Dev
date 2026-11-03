//! Exploration state written while a position is built or replayed (legacy
//! `exploration::{gain_fragment, mark_legacy_device_ids}`, the string bodies).

use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

/// Marks a play-area exploration card's action apart from every other option id.
const PLAY_AREA_PREFIX: &str = "play_area:";

/// Add one relic fragment of a trait to a player's play area.
pub fn gain_fragment(state: &mut GameState, player: &PlayerId, trait_name: &str) {
    if let Some(seat) = state.player_mut(player) {
        *seat
            .relic_fragments
            .entry(trait_name.to_ascii_uppercase())
            .or_insert(0) += 1;
    }
}

/// Whether recorded decisions use the old per-technology Enigmatic Device ids (`play_area:ed2:ws`),
/// in an answer or among the offered options. If so, marks the state so the replay offers them
/// again and the recorded offers and hashes reproduce. A new game never contains one.
pub fn mark_legacy_device_ids(state: &mut GameState, records: &[crate::choice::DecisionRecord]) {
    let legacy = |id: &String| {
        id.strip_prefix(PLAY_AREA_PREFIX)
            .is_some_and(|rest| rest.contains(':'))
    };
    if records
        .iter()
        .any(|r| legacy(&r.chosen) || r.offered.iter().any(legacy))
    {
        state.legacy_device_ids = true;
    }
}
