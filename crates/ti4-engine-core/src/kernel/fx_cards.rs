//! The card moves an effect staged without a resolver in reach, announced right after a typed event (`Live::announce_staged_cards`,
//! `reactions::announce_staged_card_events`): the string's agenda reveal and close do it after `AGENDA_REVEALED` and `AGENDA_RESOLVED`.

use super::stage::Sx;
use super::TimingError;
use crate::keys::{Atom, MarkKey, MarkVal, Word};
use crate::state::{EventKind, Key, Payload, Val};

/// Announce the staged card moves, oldest first, and drop their marks. A discard reaches the pile and `last_action_discarded` first, as a
/// played card's does; a take names the taker and the seat it came from, never the card.
///
/// # Errors
/// What the resolver refuses with.
pub fn announce_staged_cards(sx: &mut Sx<'_>) -> Result<(), TimingError> {
    loop {
        let first = sx.state.marks().tail.iter().find_map(|(k, v)| if let MarkKey::CardsStaged(n) = k { Some((*n, v.clone())) } else { None });
        let Some((n, row)) = first else { return Ok(()) };
        sx.state.marks_mut().tail.edit().remove(&MarkKey::CardsStaged(n));
        let MarkVal::Rec(row) = row else { continue };
        let [MarkVal::Atom(Atom::Word(word)), MarkVal::Atom(Atom::Seat(who)), from, MarkVal::Atom(Atom::Card(card))] = row.as_slice() else { continue };
        match word {
            Word::Discarded => {
                sx.state.cards_mut().discarded_action.edit().push(*card);
                sx.state.transient_mut().combat.last_action_discarded = Some((*who, *card));
                sx.emit_mirrored(super::fx::event(EventKind::ActionCardDiscarded, Payload::new().with(Key::Player, Val::Seat(*who)).with(Key::Card, Val::Action(*card))))?;
            }
            Word::Taken => {
                let mut payload = Payload::new().with(Key::Player, Val::Seat(*who));
                if let MarkVal::Atom(Atom::Seat(from)) = from {
                    payload = payload.with(Key::From, Val::Seat(*from));
                }
                sx.emit_mirrored(super::fx::event(EventKind::ActionCardTaken, payload))?;
            }
            _ => {}
        }
    }
}
