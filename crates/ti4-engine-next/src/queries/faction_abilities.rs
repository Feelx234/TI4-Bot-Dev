//! `faction_abilities.rs` of the legacy crate, as far as the product surface uses it: whether a seat has an ability (the core's
//! `rules::abilities`). An id the tables do not know is no ability of anyone's faction; a position the core cannot hold has none.

use ti4_content::ContentStore;
use ti4_engine_core::generated::Ability;
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use super::view::Loaded;
use crate::ties::core_adapter::typed;

/// Whether this player has a particular ability, through their faction.
#[must_use]
pub fn has(state: &GameState, _content: &ContentStore, player: &PlayerId, ability: &str) -> bool {
    let Some(view) = Loaded::of(state, None, ti4_model::content_types::FULL, SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Cards])) else { return false };
    let Some(seat) = view.seat(player) else { return false };
    typed::<Ability>(ability).is_some_and(|a| ti4_engine_core::rules::abilities::has(view.core(), seat, a))
}
