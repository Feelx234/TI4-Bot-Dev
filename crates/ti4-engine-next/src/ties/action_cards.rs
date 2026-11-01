//! From `action_cards.rs` of the legacy crate (copies).

use ti4_content::ContentStore;
use ti4_model::content_types::ContentType;
use ti4_model::id::ActionCardId;

/// The choice kind for discarding down to the limit.
pub const DISCARD_KIND: &str = "discard";

/// The choice kind for taking a card out of a hand you were shown.
pub const TAKE_REVEALED_KIND: &str = "take_revealed_card";

/// The choice kind for picking a card out of your own hand for a faction effect.
pub const OWN_HAND_KIND: &str = "own_hand_card";

/// A card's printed name, falling back to its alias.
///
/// The alias is not the card. A card printed four times has four aliases — Morale Boost is
/// `mb1` to `mb4`, and so are Flank Speed, Skilled Retreat and Maneuvering Jets — so two copies
/// in one hand are two *different* aliases carrying identical text.
#[must_use]
pub fn name_of(content: &ContentStore, card: &ActionCardId) -> String {
    content
        .get(ContentType::ActionCards, card.as_str())
        .and_then(|record| record.text("name"))
        .unwrap_or_else(|| card.as_str())
        .to_owned()
}

/// The options the inner card choice offers: one per *card*, not per alias.
///
/// Mirrors the oracle's `{name: a for a in reversed(available)}.values()` (engine/reactions.py:
/// 319–324): a card printed four times has four aliases, and holding two copies must not offer
/// it twice — a bot that samples per option would draw extra weight from the duplicate. The
/// reverse walk fixes each name's slot; the alias kept is the first one in hand order.
pub fn reaction_card_options(
    content: &ContentStore,
    options: &[ActionCardId],
) -> Vec<crate::wire::choice::ChoiceOption> {
    let mut slots: Vec<(String, ActionCardId)> = Vec::new();
    for alias in options.iter().rev() {
        let name = crate::ties::action_cards::name_of(content, alias);
        match slots.iter_mut().find(|(slot_name, _)| *slot_name == name) {
            Some(slot) => slot.1 = alias.clone(),
            None => slots.push((name, alias.clone())),
        }
    }
    slots
        .into_iter()
        .map(|(_, alias)| {
            crate::wire::choice::ChoiceOption::labelled(
                alias.to_string(),
                ACTION_CARD_KIND,
                format!("play {}", crate::ties::action_cards::name_of(content, &alias)),
            )
            .with("card", alias.to_string())
            .with("card_name", crate::ties::action_cards::name_of(content, &alias))
        })
        .collect()
}

/// The choice kind the oracle offers reaction cards under (engine/reactions.py:320–324).
pub const ACTION_CARD_KIND: &str = "action_card";
