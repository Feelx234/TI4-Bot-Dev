//! From `trade_write.rs` of the legacy crate (copy).

use ti4_engine_core::generated::Faction;
use ti4_engine_core::state::NoteKey;

use super::core_adapter::typed;
use crate::core_sync::keys::text::parse_note;

/// The typed key of a `alias:faction` note id.
#[must_use]
pub fn note_key(text: &str) -> Option<NoteKey> {
    let (alias, faction) = text.split_once(':')?;
    Some(NoteKey { note: parse_note(alias)?, faction: typed::<Faction>(faction)? })
}
