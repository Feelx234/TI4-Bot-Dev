//! The diplomacy recency reads of the surface (`diplomacy/relations.rs`): plain reads of the journal of the state. The feature is dropped
//! from new games; positions that carry a journal still answer.

use std::collections::BTreeMap;

use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

fn recent(rounds: &BTreeMap<PlayerId, BTreeMap<PlayerId, u32>>, observer: &PlayerId, subject: &PlayerId, round: u32) -> bool {
    rounds.get(observer).and_then(|row| row.get(subject)).is_some_and(|event_round| round <= event_round.saturating_add(1))
}

/// Whether `subject` attacked `observer` this round or the one before.
#[must_use]
pub fn recent_attack(state: &GameState, observer: &PlayerId, subject: &PlayerId) -> bool {
    recent(&state.diplomacy.last_attacks, observer, subject, state.round)
}

/// Whether `subject` breached `observer`'s agreement this round or the one before.
#[must_use]
pub fn recent_breach(state: &GameState, observer: &PlayerId, subject: &PlayerId) -> bool {
    recent(&state.diplomacy.last_breaches, observer, subject, state.round)
}
