//! Plot cards (Thunder's Edge: The Firmament and The Obsidian).
//!
//! A plot card sits in its owner's play area with one or more control tokens of other players on
//! it. While it is **facedown** (the Firmament side) only its owner knows which tokens it carries;
//! once flipped (the Obsidian side, `puppetsoftheblade`) it is public. `Player::plots` stores one
//! string per card, so the schema is unchanged; this module is the typed reading of that string:
//!
//! * `d:<token>[,<token>...]` facedown, tokens in id order;
//! * `u:<token>[,<token>...]` faceup;
//! * a bare `<token>` is the original one-token facedown form and still reads.
//!
//! A card can carry several tokens (the Firmament hero adds one to a card already in play) but never
//! two of the same player's. Which kind of plot a card is (Enervate, Siphon, ...) belongs to The
//! Obsidian's own text and is added by that package; nothing here depends on it.

use std::collections::BTreeSet;

use crate::id::PlayerId;

/// What a viewer who is not the owner sees in place of a facedown card.
pub const HIDDEN_PLOT: &str = "?";

/// One plot card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plot {
    /// Players whose control tokens are on this card (the "puppeted" players once it is faceup).
    pub tokens: BTreeSet<PlayerId>,
    /// Whether the card is faceup (flipped to The Obsidian's side).
    pub faceup: bool,
}

impl Plot {
    /// A new facedown card carrying one player's token.
    #[must_use]
    pub fn facedown(token: &PlayerId) -> Self {
        Self {
            tokens: BTreeSet::from([token.clone()]),
            faceup: false,
        }
    }

    /// The stored form.
    #[must_use]
    pub fn encode(&self) -> String {
        let tokens: Vec<&str> = self.tokens.iter().map(PlayerId::as_str).collect();
        format!(
            "{}:{}",
            if self.faceup { 'u' } else { 'd' },
            tokens.join(",")
        )
    }

    /// Read a stored card; `None` for the hidden marker and for an empty string.
    #[must_use]
    pub fn decode(text: &str) -> Option<Self> {
        if text.is_empty() || text == HIDDEN_PLOT {
            return None;
        }
        let (faceup, list) = match text.split_once(':') {
            Some(("u", list)) => (true, list),
            Some(("d", list)) => (false, list),
            _ => (false, text),
        };
        let tokens: BTreeSet<PlayerId> = list
            .split(',')
            .filter(|token| !token.is_empty())
            .map(PlayerId::new)
            .collect();
        (!tokens.is_empty()).then_some(Self { tokens, faceup })
    }

    /// What a player other than the owner is shown for this card: the card itself once faceup, the
    /// hidden marker while facedown (the count survives, the tokens do not).
    #[must_use]
    pub fn shown_to_others(stored: &str) -> String {
        match Self::decode(stored) {
            Some(plot) if plot.faceup => plot.encode(),
            _ => HIDDEN_PLOT.to_owned(),
        }
    }
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
    fn a_card_round_trips_and_the_bare_form_reads_as_facedown() {
        let mut plot = Plot::facedown(&PlayerId::new("b"));
        plot.tokens.insert(PlayerId::new("a"));
        assert_eq!(plot.encode(), "d:a,b");
        assert_eq!(Plot::decode("d:a,b"), Some(plot.clone()));
        plot.faceup = true;
        assert_eq!(Plot::decode(&plot.encode()), Some(plot));
        assert_eq!(Plot::decode("c"), Some(Plot::facedown(&PlayerId::new("c"))));
        assert_eq!(Plot::decode("?"), None);
        assert_eq!(Plot::decode(""), None);
    }

    #[test]
    fn others_see_a_facedown_card_as_a_marker_and_a_faceup_card_as_it_is() {
        assert_eq!(Plot::shown_to_others("d:b"), HIDDEN_PLOT);
        assert_eq!(Plot::shown_to_others("u:b"), "u:b");
    }
}
