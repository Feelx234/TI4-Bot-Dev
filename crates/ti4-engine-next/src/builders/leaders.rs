//! Giving a seat its faction's leaders (legacy `leaders::{deploy, starting_states, for_faction,
//! kind_of}`, the string bodies).

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

pub use ti4_engine_core_setup::leaders::{AGENT, for_faction, kind_of, starting_states};

/// Give a player their faction's leaders (`leaders::deploy`).
pub fn deploy(
    state: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) {
    let Some(faction) = state.player(player).map(|seat| seat.faction.to_string()) else {
        return;
    };
    let starting = starting_states(content, sources, &faction);
    if let Some(seat) = state.player_mut(player) {
        for (leader, status) in starting {
            seat.leaders.insert(leader, status);
        }
    }
}
