//! `strategy_cards.rs` of the legacy crate, as far as the product surface reads it: the kind of a printed strategy card.

use ti4_content::ContentStore;
use ti4_content::StrategyCardKind;
use ti4_model::content_types::ContentType;

/// The kind of a strategy card by its alias. Every printing of one card maps to one kind, as the printed name did: base and Codex Diplomacy
/// are both `Diplomacy`.
#[must_use]
pub fn card_kind(content: &ContentStore, card: &str) -> Option<StrategyCardKind> {
    content.get(ContentType::StrategyCards, card).and_then(ti4_content::Record::strategy_card_kind)
}
