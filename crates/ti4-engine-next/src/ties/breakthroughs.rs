//! From `breakthroughs.rs` of the legacy crate (copies).

use ti4_content::ContentStore;
use ti4_model::id::BreakthroughId;


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

/// Whether a content record's `faction` tag belongs to `alias`.
///
/// A record normally names its faction by alias. Keleres is the exception: its shared cards are
/// tagged with the family `keleres` (technologies, leaders, breakthrough) or with the first variant
/// `keleresm` (units shared by all three), while each variant's Rider is tagged with the variant.
/// The variant's own faction sheet still decides which leaders and units it actually has.
#[must_use]
pub fn tag_belongs_to(tag: &str, alias: &str, unit: bool) -> bool {
    tag.eq_ignore_ascii_case(alias)
        || (is_keleres_faction(alias)
            && (tag.eq_ignore_ascii_case("keleres")
                || (unit && tag.eq_ignore_ascii_case("keleresm"))))
}

/// The Keleres variants.
pub const VARIANTS: [&str; 3] = ["keleresm", "keleresx", "keleresa"];

/// Whether `faction` is any Keleres variant.
#[must_use]
pub fn is_keleres_faction(faction: &str) -> bool {
    VARIANTS.contains(&faction)
}
