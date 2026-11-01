//! The bound seat's own cards (`SeatObservation::{held_action_cards, held_secrets, held_promissory_notes}`).
//! No accessor here takes a seat: they answer for [`Seen::me`] only.

use super::Seen;
use crate::generated::{ActionCard, SecretObjective};
use crate::hooks::RevealKind;
use crate::ids::Seat;
use crate::keys::{Atom, MarkKey, MarkVal, Word};
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

// -- the hidden cards other seats have shown the bound seat (lane G5; `SeatObservation::revealed_cards`) ------------------------------------

/// Hidden cards of one kind another seat has shown the bound seat (`hooks_cards::Revealed`), only those the owner still holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revealed {
    /// The holder whose hand was shown.
    pub owner: Seat,
    pub shown: Shown,
}

/// The shown cards, in the order they were revealed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shown {
    ActionCards(Vec<ActionCard>),
    PromissoryNotes(Vec<NoteKey>),
    SecretObjectives(Vec<SecretObjective>),
}

impl Shown {
    /// Which kind of card (`RevealKind`).
    #[must_use]
    pub const fn kind(&self) -> RevealKind {
        match self {
            Self::ActionCards(_) => RevealKind::ActionCards,
            Self::PromissoryNotes(_) => RevealKind::PromissoryNotes,
            Self::SecretObjectives(_) => RevealKind::SecretObjectives,
        }
    }

    const fn empty(kind: RevealKind) -> Self {
        match kind {
            RevealKind::ActionCards => Self::ActionCards(Vec::new()),
            RevealKind::PromissoryNotes => Self::PromissoryNotes(Vec::new()),
            RevealKind::SecretObjectives => Self::SecretObjectives(Vec::new()),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::ActionCards(v) => v.is_empty(),
            Self::PromissoryNotes(v) => v.is_empty(),
            Self::SecretObjectives(v) => v.is_empty(),
        }
    }

    /// Add what `item` names unless it is shown already (the kind decides what an item can be).
    fn add(&mut self, item: Item) {
        match (self, item) {
            (Self::ActionCards(v), Item::Card(c)) if !v.contains(&c) => v.push(c),
            (Self::PromissoryNotes(v), Item::Note(n)) if !v.contains(&n) => v.push(n),
            (Self::SecretObjectives(v), Item::Secret(x)) if !v.contains(&x) => v.push(x),
            _ => {}
        }
    }
}

/// One card of a reveal row.
#[derive(Debug, Clone, Copy)]
enum Item {
    Card(ActionCard),
    Note(NoteKey),
    Secret(SecretObjective),
}

fn item_of(v: &MarkVal) -> Option<Item> {
    match v {
        MarkVal::Atom(Atom::Card(c)) => Some(Item::Card(*c)),
        MarkVal::Atom(Atom::Secret(x)) => Some(Item::Secret(*x)),
        MarkVal::Rec(parts) => match parts.as_slice() {
            [MarkVal::Atom(Atom::Note(note)), MarkVal::Atom(Atom::Faction(faction))] => Some(Item::Note(NoteKey { note: *note, faction: *faction })),
            _ => None,
        },
        _ => None,
    }
}

impl Seen<'_> {
    /// Whether `owner` holds `item` as a hidden card now (`hooks_cards::held`).
    fn holds_hidden(&self, owner: Seat, item: Item) -> bool {
        let s = self.st();
        match item {
            Item::Card(c) => s.players().by_seat(owner).is_some_and(|p| p.cards.hand.as_slice().contains(&c)),
            Item::Secret(x) => s.players().by_seat(owner).is_some_and(|p| p.cards.secrets.as_slice().contains(&x)),
            Item::Note(n) => s.cards().promissory.get(&n) == Some(&owner) && !s.cards().promissory_faceup.contains_key(&n),
        }
    }

    /// What other seats have shown the bound seat of their hidden cards, and nothing of anyone else's (`SeatObservation::revealed_cards`): the
    /// rows a reveal recorded, in the order of (owner seat, kind), the ids in reveal order, a card its owner no longer holds dropped; then the
    /// standing permissions of the hooks (M'aban: the promissory notes in the hands of the neighbours), merged into the owner's row or added
    /// after the recorded rows in seating order. The string orders the recorded rows by the owner's id text; a caller that compares sorts
    /// by it. The notes of a standing permission are in the order of the note table (the string's is the text order of the ids).
    #[must_use]
    pub fn revealed_cards(&self) -> Vec<Revealed> {
        let (s, me) = (self.st(), self.me());
        let mut merged: Vec<((Seat, RevealKind), Shown)> = Vec::new();
        for (key, row) in s.marks().tail.iter() {
            let MarkKey::CardsReveal(_, _, viewer, owner, deck) = key else { continue };
            let kind = match deck {
                Word::ActionCards => RevealKind::ActionCards,
                Word::PromissoryNotes => RevealKind::PromissoryNotes,
                Word::SecretObjectives => RevealKind::SecretObjectives,
                _ => continue,
            };
            if *viewer != me {
                continue;
            }
            let MarkVal::List(items) = row else { continue };
            let at = merged.iter().position(|(k, _)| *k == (*owner, kind)).unwrap_or_else(|| {
                merged.push(((*owner, kind), Shown::empty(kind)));
                merged.len() - 1
            });
            let shown = &mut merged[at].1;
            for item in items.iter().filter_map(item_of) {
                if self.holds_hidden(*owner, item) {
                    shown.add(item);
                }
            }
        }
        merged.sort_by_key(|(k, _)| *k);
        let mut rows: Vec<Revealed> = merged.into_iter().filter(|(_, shown)| !shown.is_empty()).map(|((owner, _), shown)| Revealed { owner, shown }).collect();
        // Standing hand permissions are read from the position each time (M'aban), never stored.
        for owner in s.seats().seating_order() {
            if *owner == me || !self.kernel().hooks.cards.may_view_hand(s, self.kernel(), me, *owner, RevealKind::PromissoryNotes) {
                continue;
            }
            let mut notes = Shown::empty(RevealKind::PromissoryNotes);
            for (key, holder) in s.cards().promissory.iter() {
                if holder == owner && !s.cards().promissory_faceup.contains_key(key) {
                    notes.add(Item::Note(*key));
                }
            }
            if notes.is_empty() {
                continue;
            }
            match rows.iter_mut().find(|row| row.owner == *owner && row.shown.kind() == RevealKind::PromissoryNotes) {
                Some(Revealed { shown: Shown::PromissoryNotes(have), .. }) => {
                    if let Shown::PromissoryNotes(more) = notes {
                        for note in more {
                            if !have.contains(&note) {
                                have.push(note);
                            }
                        }
                    }
                }
                _ => rows.push(Revealed { owner: *owner, shown: notes }),
            }
        }
        rows
    }
}
