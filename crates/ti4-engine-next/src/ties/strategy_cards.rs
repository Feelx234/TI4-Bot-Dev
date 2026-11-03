//! From `strategy_cards.rs` of the legacy crate.

use ti4_content::ContentStore;
use ti4_model::content_types::ContentType;

/// Display only: the agenda card being placed, as printed.
pub fn agenda_details(content: &ContentStore, alias: &str) -> serde_json::Value {
    let record = content.get(ContentType::Agendas, alias);
    let field = |key: &str| record.as_ref().and_then(|record| record.text(key)).unwrap_or_default().to_owned();
    let name = Some(field("name")).filter(|name| !name.is_empty()).unwrap_or_else(|| alias.to_owned());
    serde_json::json!({
        "id": alias,
        "name": name,
        "type": field("type"),
        "target": field("target"),
        "text1": field("text1"),
        "text2": field("text2"),
    })
}

/// The printed name of a strategy card. Display only: the rules read [`card_kind`].
#[must_use]
pub fn card_name(content: &ContentStore, card: &str) -> Option<String> {
    content
        .get(ContentType::StrategyCards, card)
        .and_then(|record| record.text("name"))
        .map(ToOwned::to_owned)
}

pub const RESEARCH_KIND: &str = "research";

/// Display only: a leader's printed card (name, ability window and text) as an offer-card header.
pub fn leader_card(content: &ContentStore, id: &str, tag: &str) -> serde_json::Value {
    let record = content.get(ContentType::Leaders, id);
    let field = |key: &str| record.as_ref().and_then(|record| record.text(key));
    crate::wire::choice::offer_card(
        field("name").unwrap_or(id),
        tag,
        field("abilityWindow"),
        field("abilityText"),
    )
}

/// Display only: a commander's printed card (name, ability window and text) as an offer-card
/// header. See [`Choice::offered`].
pub fn commander_card(content: &ContentStore, id: &str) -> serde_json::Value {
    leader_card(content, id, "commander")
}
