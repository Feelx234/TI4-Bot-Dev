//! Plot cards (Thunder's Edge: The Firmament and The Obsidian).
//!
//! A plot card sits in its owner's play area with one or more control tokens of players on it. There
//! are exactly five plot cards ([`CARDS`]: Enervate, Siphon, Seethe, Assail, Extract); a card is in
//! play at most once. While a card is **facedown** (the Firmament side) its identity is hidden from
//! the other players, but the control tokens on it are public; once flipped (the Obsidian side,
//! `puppetsoftheblade`) the whole card is public. `Player::plots` stores one string per card, so the
//! schema is unchanged; this module is the typed reading of that string:
//!
//! * `d:<card>:<token>[,<token>...]` facedown, tokens in id order;
//! * `u:<card>:<token>[,<token>...]` faceup;
//! * `<card>` is the card's alias, or `?` where the identity is hidden from the reader;
//! * the earlier two-part forms (`d:<tokens>`, `u:<tokens>`) and a bare `<token>` still read, as a
//!   card whose identity is unknown (it has no effect and occupies no named card).
//!
//! A card can carry several tokens (the Firmament hero adds one to a card already in play) but never
//! two of the same player's. What each card does belongs to The Obsidian's own text.

use std::collections::BTreeSet;

use crate::id::PlayerId;

/// What a viewer who is not the owner sees in place of a facedown card's identity.
pub const HIDDEN_PLOT: &str = "?";

/// The five plot cards, in printed order (`genericcards.json`, `cardType` plot).
pub const CARDS: [&str; 5] = ["enervate", "siphon", "seethe", "assail", "extract"];

/// One plot card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plot {
    /// Which of the [`CARDS`] this is; `None` when the reader may not know (or an old stored form).
    pub card: Option<String>,
    /// Players whose control tokens are on this card (the "puppeted" players once it is faceup).
    pub tokens: BTreeSet<PlayerId>,
    /// Whether the card is faceup (flipped to The Obsidian's side).
    pub faceup: bool,
}

impl Plot {
    /// A new facedown card, `card`, carrying one player's token.
    #[must_use]
    pub fn facedown(card: &str, token: &PlayerId) -> Self {
        Self {
            card: Some(card.to_owned()),
            tokens: BTreeSet::from([token.clone()]),
            faceup: false,
        }
    }

    /// Whether this is the plot card `card`.
    #[must_use]
    pub fn is(&self, card: &str) -> bool {
        self.card.as_deref() == Some(card)
    }

    /// The stored form.
    #[must_use]
    pub fn encode(&self) -> String {
        let tokens: Vec<&str> = self.tokens.iter().map(PlayerId::as_str).collect();
        format!(
            "{}:{}:{}",
            if self.faceup { 'u' } else { 'd' },
            self.card.as_deref().unwrap_or(HIDDEN_PLOT),
            tokens.join(",")
        )
    }

    /// Read a stored card; `None` for the hidden marker and for an empty string.
    #[must_use]
    pub fn decode(text: &str) -> Option<Self> {
        if text.is_empty() || text == HIDDEN_PLOT {
            return None;
        }
        let parts: Vec<&str> = text.splitn(3, ':').collect();
        let (faceup, card, list) = match parts.as_slice() {
            ["u", card, list] => (true, Some(*card), *list),
            ["d", card, list] => (false, Some(*card), *list),
            ["u", list] => (true, None, *list),
            ["d", list] => (false, None, *list),
            _ => (false, None, text),
        };
        let tokens: BTreeSet<PlayerId> = list
            .split(',')
            .filter(|token| !token.is_empty())
            .map(PlayerId::new)
            .collect();
        let card = card
            .filter(|card| *card != HIDDEN_PLOT && !card.is_empty())
            .map(ToOwned::to_owned);
        (!tokens.is_empty()).then_some(Self {
            card,
            tokens,
            faceup,
        })
    }

    /// What a player other than the owner is shown for this card: the whole card once faceup; while
    /// facedown the tokens on it (public) but not which card it is.
    #[must_use]
    pub fn shown_to_others(stored: &str) -> String {
        match Self::decode(stored) {
            Some(plot) if plot.faceup => plot.encode(),
            Some(plot) => Self { card: None, ..plot }.encode(),
            None => HIDDEN_PLOT.to_owned(),
        }
    }

    /// Whether `stored` is a facedown card whose identity is readable (a leak to anyone but the
    /// owner).
    #[must_use]
    pub fn leaks_identity(stored: &str) -> bool {
        Self::decode(stored).is_some_and(|plot| !plot.faceup && plot.card.is_some())
    }
}

/// The plot cards of [`CARDS`] that no card in `in_play` is: the ones a placement may take.
///
/// `in_play` is every stored plot string of every player. A card of unknown identity (an old stored
/// form) still occupies one of the five places, so with five cards in play nothing remains.
#[must_use]
pub fn remaining<'a>(in_play: impl IntoIterator<Item = &'a String>) -> Vec<&'static str> {
    let cards: Vec<Plot> = in_play
        .into_iter()
        .filter_map(|text| Plot::decode(text))
        .collect();
    if cards.len() >= CARDS.len() {
        return Vec::new();
    }
    CARDS
        .iter()
        .copied()
        .filter(|card| !cards.iter().any(|plot| plot.is(card)))
        .collect()
}

/// Every card of `stored`, skipping any that does not read.
#[must_use]
pub fn read(stored: &[String]) -> Vec<Plot> {
    stored
        .iter()
        .filter_map(|text| Plot::decode(text))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_round_trips_and_the_old_forms_read_as_a_card_of_unknown_identity() {
        let mut plot = Plot::facedown("siphon", &PlayerId::new("b"));
        plot.tokens.insert(PlayerId::new("a"));
        assert_eq!(plot.encode(), "d:siphon:a,b");
        assert_eq!(Plot::decode("d:siphon:a,b"), Some(plot.clone()));
        plot.faceup = true;
        assert_eq!(Plot::decode(&plot.encode()), Some(plot));
        let old = Plot {
            card: None,
            tokens: BTreeSet::from([PlayerId::new("c")]),
            faceup: false,
        };
        assert_eq!(Plot::decode("c"), Some(old.clone()));
        assert_eq!(Plot::decode("d:c"), Some(old.clone()));
        assert_eq!(Plot::decode("d:?:c"), Some(old));
        assert_eq!(Plot::decode("?"), None);
        assert_eq!(Plot::decode(""), None);
    }

    #[test]
    fn others_see_the_tokens_on_a_facedown_card_but_not_which_card_it_is() {
        assert_eq!(Plot::shown_to_others("d:seethe:b,c"), "d:?:b,c");
        assert_eq!(Plot::shown_to_others("u:seethe:b"), "u:seethe:b");
        assert!(Plot::leaks_identity("d:seethe:b"));
        assert!(!Plot::leaks_identity("d:?:b"));
        assert!(!Plot::leaks_identity("u:seethe:b"));
    }

    #[test]
    fn there_are_five_cards_and_a_placed_card_leaves_the_remaining_ones() {
        let none: Vec<String> = Vec::new();
        assert_eq!(remaining(&none), CARDS.to_vec());
        let placed = vec!["d:assail:a".to_owned(), "u:seethe:b".to_owned()];
        assert_eq!(remaining(&placed), ["enervate", "siphon", "extract"]);
        let all: Vec<String> = CARDS.iter().map(|card| format!("d:{card}:a")).collect();
        assert!(remaining(&all).is_empty());
        let old_form: Vec<String> = vec!["d:a".to_owned(); 5];
        assert!(
            remaining(&old_form).is_empty(),
            "an unknown card still holds its place"
        );
    }
}
