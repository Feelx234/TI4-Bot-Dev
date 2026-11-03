//! From `draft.rs` of the legacy crate (copies).

/// The constants live in `crate::kinds` (lane B); this module re-exports them.
pub use crate::kinds::draft::{STRATEGY_CARD_KIND};

use ti4_content::ContentStore;
use ti4_model::content_types::ContentType;


pub fn strategy_card_label(content: &ContentStore, card_id: &str) -> String {
    let Some(card) = content.get(ContentType::StrategyCards, card_id) else {
        return card_id.to_owned();
    };
    let initiative = card
        .int("initiative")
        .map_or_else(|| "?".to_owned(), |number| number.to_string());
    let name = card.text("name").unwrap_or(card_id);
    format!("{initiative}. {name}")
}
