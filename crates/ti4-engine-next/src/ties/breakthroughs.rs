//! From `breakthroughs.rs` of the legacy crate (copies).

use ti4_content::ContentStore;
use ti4_model::id::BreakthroughId;

/// The Keleres variants and the tag test: defined once, in `crate::builders::factions` (lane C).
pub use crate::builders::factions::{KELERES_VARIANTS as VARIANTS, is_keleres_faction, tag_belongs_to};


/// This faction's breakthrough alias, if the corpus lists one.
///
/// Looked up by alias across every source rather than through [`ContentStore::from_sources`]: a
/// player only ever holds a breakthrough because Thunder's Edge is already in the game (nothing
/// else grants one), so filtering this particular lookup by the active source scope would just
/// risk a false negative for a card resolved under a narrower scope than the one that seated
/// the player. `crate::thunders_edge::breakthrough_for` makes the identical choice when a
/// breakthrough is first granted.
#[must_use]
pub fn for_faction(content: &ContentStore, faction: &str) -> Option<BreakthroughId> {
    content
        .records(ti4_model::content_types::ContentType::Breakthroughs)
        .iter()
        .find(|record| {
            record
                .text("faction")
                .is_some_and(|tag| tag_belongs_to(tag, faction, false))
        })
        .and_then(|record| record.text("alias"))
        .map(BreakthroughId::new)
}


