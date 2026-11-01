//! `promissory.rs` of the legacy crate, as far as the product surface reads it: the ids of notes and who holds them. Plain reads of the public
//! state, copied. (The writes, `deal`, `take`, `receive` and `support`, build positions: lane C's `builders::promissory`, which also has the
//! reads below under the same names; take one at the merge.)

use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::kinds::promissory::SUPPORT_PREFIX;

/// A note id: the printed alias, and whose copy it is, the owner's **faction name**.
#[must_use]
pub fn note_id(alias: &str, owner_name: &str) -> String {
    format!("{alias}:{owner_name}")
}

/// The printed alias a note id carries.
#[must_use]
pub fn alias_of(note: &str) -> &str {
    note.split_once(':').map_or(note, |(alias, _)| alias)
}

/// The faction name a note belongs to, or `None` if it is not a note id at all.
#[must_use]
pub fn owner_of(note: &str) -> Option<String> {
    if let Some(owner) = note.strip_prefix(SUPPORT_PREFIX) {
        return Some(owner.to_owned());
    }
    note.split_once(':').map(|(_, owner)| owner.to_owned())
}

/// The faction a player plays: the identity embedded in every note id that player owns.
#[must_use]
pub fn faction_name(state: &GameState, player: &PlayerId) -> String {
    state.player(player).map(|seat| seat.faction.as_str().to_owned()).unwrap_or_default()
}

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

/// Every note this player holds, their own or received, in a stable order.
#[must_use]
pub fn held_by(state: &GameState, player: &PlayerId) -> Vec<String> {
    state.promissory_notes.iter().filter(|(_, holder)| *holder == player).map(|(note, _)| note.clone()).collect()
}

/// Whether `receiver` may be given `note`: Mahact's Hubris says "Other players cannot give you their 'Alliance' promissory note."
#[must_use]
pub fn may_receive(state: &GameState, receiver: &PlayerId, note: &str) -> bool {
    !(alias_of(note) == "an" && state.player(receiver).is_some_and(|seat| seat.faction.as_str() == "mahact"))
}
