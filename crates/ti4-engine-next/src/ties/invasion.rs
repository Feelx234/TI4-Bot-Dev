//! From `invasion.rs` of the legacy crate (copies).

use ti4_content::ContentStore;

/// The choice kind for choosing which of your own ground forces dies.
pub const GROUND_CASUALTY_KIND: &str = "ground_casualty";

/// Display only: a breakthrough as an offer-card header: its name and the first sentence of its
/// printed text (the ability that is being offered).
pub fn breakthrough_card(content: &ContentStore, card: &str) -> serde_json::Value {
    let record = content.get(ti4_model::content_types::ContentType::Breakthroughs, card);
    let field = |key: &str| record.as_ref().and_then(|record| record.text(key));
    let first_sentence = field("text").map(|text| match text.find(". ") {
        Some(end) => &text[..=end],
        None => text,
    });
    crate::wire::choice::offer_card(
        field("name").unwrap_or(card),
        "breakthrough",
        None,
        first_sentence,
    )
}
