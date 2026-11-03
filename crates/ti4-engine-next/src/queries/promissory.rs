//! `promissory.rs` of the legacy crate, as far as the product surface reads it: the ids of notes and who holds them. Plain reads of the public
//! state, copied. (The writes, `deal`, `take`, `receive` and `support`, build positions: lane C's `builders::promissory`, which also owns the reads re-exported below:
//! ONE definition each, integration 18.)

use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

pub use crate::builders::promissory::{alias_of, faction_name, held_by, may_receive, note_id, owner_of};

/// Who holds a particular note, or `None` if its owner still has it (compared by faction, not seat).
#[must_use]
pub fn holder_of(state: &GameState, alias: &str, owner: &PlayerId) -> Option<PlayerId> {
    let name = faction_name(state, owner);
    let note = note_id(alias, &name);
    match state.promissory_notes.get(&note) {
        Some(holder) if faction_name(state, holder) != name => Some(holder.clone()),
        _ => None,
    }
}

