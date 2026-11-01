//! The standing-law reads of the surface (copy of the text half of `laws.rs`; the typed half is the same read of the core's agenda slice,
//! equal on every position by the dual runs).

use ti4_model::state::GameState;

const BLANKED_TURN: &str = "keleres|laws_blank";

/// Whether all laws are blank right now: Law's Order was used this turn and the turn has not ended.
#[must_use]
pub fn blanked(state: &GameState) -> bool {
    state.phase == ti4_model::state::Phase::Action && state.faction_marks.get(BLANKED_TURN).is_some_and(|turn| *turn == state.turn_seq.to_string())
}

/// Whether a law is in play. Never while [`blanked`].
#[must_use]
pub fn active(state: &GameState, alias: &str) -> bool {
    !blanked(state) && state.laws.contains_key(alias)
}

/// Regulated Conscription: a fighter or infantry costs its price for one unit, not two.
#[must_use]
pub fn single_unit_production(state: &GameState) -> bool {
    active(state, "conscription")
}

/// Fleet Regulations caps the fleet pool at four.
#[must_use]
pub fn fleet_pool_cap(state: &GameState, base: i32) -> i32 {
    if active(state, "regulations") { base.min(4) } else { base }
}
