//! The bound seat's own cards (`SeatObservation::{held_action_cards, held_secrets, held_promissory_notes}`).
//! No accessor here takes a seat: they answer for [`Seen::me`] only.

use super::Seen;
use crate::generated::{ActionCard, SecretObjective};
use crate::query::promissory;
use crate::state::NoteKey;

impl Seen<'_> {
    /// The action cards in the bound seat's hand, in hand order (`SeatObservation::held_action_cards`).
    #[must_use]
    pub fn my_action_cards(&self) -> &[ActionCard] {
        self.st().players().by_seat(self.me()).map_or(&[], |p| p.cards.hand.as_slice())
    }

    /// The secret objectives in the bound seat's hand (`SeatObservation::held_secrets`).
    #[must_use]
    pub fn my_secrets(&self) -> &[SecretObjective] {
        self.st().players().by_seat(self.me()).map_or(&[], |p| p.cards.secrets.as_slice())
    }

    /// The promissory notes in the bound seat's hand, not yet faceup (`SeatObservation::held_promissory_notes`).
    #[must_use]
    pub fn my_notes(&self) -> Vec<NoteKey> {
        let faceup = &self.st().cards().promissory_faceup;
        promissory::held_by(self.st(), self.me()).into_iter().filter(|k| !faceup.contains_key(k)).collect()
    }
}
