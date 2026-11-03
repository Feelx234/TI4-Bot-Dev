//! From `laws.rs` of the legacy crate (copies).

use ti4_model::state::GameState;

const BLANKED_TURN: &str = "keleres|laws_blank";


/// Whether all laws are blank right now: Law's Order was used this turn and the turn has not ended.
///
/// "You may spend 1 trade good or 1 commodity to treat all laws as blank until the end of that
/// turn." Scoped by `turn_seq` (it moves when the turn passes) and to the action phase, so the
/// mark cannot leak into the status or agenda phase that follows the last turn.
#[must_use]
pub fn blanked(state: &GameState) -> bool {
    state.phase == ti4_model::state::Phase::Action
        && state
            .faction_marks
            .get(BLANKED_TURN)
            .is_some_and(|turn| *turn == state.turn_seq.to_string())
}
