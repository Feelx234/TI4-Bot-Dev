//! The consumer of the card moves an ability staged without a resolver in reach (lane G5; the string's
//! `reactions::announce_staged_card_events` with `hooks_cards::{discard_chosen, take_revealed_action_card}`).
//!
//! An effect that moves an action card (Spy Net, So Ata, Kyver, Mageon Implants, the discards of a Scheming draw) changes the hands at once
//! and writes the event it owes as a `CardsStaged` mark ([`fx::discard_action_card_staged`], [`fx::take_action_card_staged`]); the game step
//! announces them oldest first: a discard goes on the discard pile and becomes `ACTION_CARD_DISCARDED` (`announce_discard`), a take becomes
//! `ACTION_CARD_TAKEN` with the taker and the previous holder and deliberately no card (it is private to the two seats). The string runs it
//! after every typed event of the game ([`Sx::emit_mirrored`] calls [`announce_staged_cards`] here) and after a component action. With
//! nothing staged this does nothing at all.

use crate::generated::{ActionCard, Breakthrough};
use crate::ids::Seat;
use crate::kernel::ability::{Runner, Unknown};
use crate::kernel::fx;
use crate::kernel::resolver::TimingError;
use crate::kernel::stage::Sx;
use crate::keys::{Atom, MarkKey, MarkVal, Word};
use crate::state::{CoreState, EventKind, Key, Payload, Phase, Val};

/// One card move waiting for the game step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Staged {
    /// `who` discarded `card` from their hand.
    Discarded { who: Seat, card: ActionCard },
    /// `taker` took `card` out of `from`'s hand.
    Taken { taker: Seat, from: Seat, card: ActionCard },
}

/// Whether a card move is staged.
#[must_use]
pub fn has_staged_cards(s: &CoreState) -> bool {
    // A footprint without the marks cannot have staged a row (the windows that stage declare the slice).
    s.loaded.has(crate::state::Slice::Marks)
        && s.marks().tail.iter().any(|(k, _)| matches!(k, MarkKey::CardsStaged(_)))
}

/// The row of a `CardsStaged` mark: `[word, seat, from or absent, card]`.
fn decode(row: &MarkVal) -> Option<Staged> {
    let MarkVal::Rec(parts) = row else { return None };
    let [MarkVal::Atom(Atom::Word(word)), MarkVal::Atom(Atom::Seat(who)), from, MarkVal::Atom(Atom::Card(card))] = parts.as_slice() else { return None };
    match (word, from) {
        (Word::Discarded, MarkVal::Atom(Atom::Absent)) => Some(Staged::Discarded { who: *who, card: *card }),
        (Word::Taken, MarkVal::Atom(Atom::Seat(from))) => Some(Staged::Taken { taker: *who, from: *from, card: *card }),
        _ => None,
    }
}

/// Take the staged card moves out of the marks, oldest first (`hooks_cards::drain_staged`).
pub fn drain_staged_cards(s: &mut CoreState) -> Vec<Staged> {
    let mut rows: Vec<(u32, Option<Staged>)> =
        s.marks().tail.iter().filter_map(|(k, v)| if let MarkKey::CardsStaged(n) = k { Some((*n, decode(v))) } else { None }).collect();
    rows.sort_by_key(|(n, _)| *n);
    for (n, _) in &rows {
        s.marks_mut().tail.edit().remove(&MarkKey::CardsStaged(*n));
    }
    rows.into_iter().filter_map(|(_, staged)| staged).collect()
}

/// Whether the Data Skimmer would take a card `who` discards (`ralnel_cards::skimmer_takes`): its holder is another seat that has not passed,
/// during the action phase. The core has no rule for the breakthrough (Thunder's Edge): the discard is the string's (a counted bail).
fn skimmer_could_take(s: &CoreState, who: Seat) -> bool {
    s.meta().phase == Phase::Action
        && s.players().iter().any(|p| p.seat != who && !p.econ.passed && p.cards.breakthrough == Some(Breakthrough::DataSkimmer))
}

/// `reactions::announce_discard` of a staged discard: the card is on the pile (the Data Skimmer aside), `last_action_discarded` names it,
/// and the discard is announced.
fn announce_discard(sx: &mut Sx<'_>, who: Seat, card: ActionCard) -> Result<(), TimingError> {
    if skimmer_could_take(sx.state, who) {
        sx.unknown(Unknown::DataSkimmer);
    }
    sx.state.cards_mut().discarded_action.edit().push(card);
    sx.state.transient_mut().combat.last_action_discarded = Some((who, card));
    let payload = Payload::new().with(Key::Player, Val::Seat(who)).with(Key::Card, Val::Action(card));
    sx.emit(fx::event(EventKind::ActionCardDiscarded, payload))?;
    Ok(())
}

/// Announce every staged card move, oldest first, and mirror them into the emission queue as one call (`Live::announce_staged_cards`).
/// Returns how many were announced. A window that stages more while its event is announced leaves them for the next call, as in the string.
///
/// # Errors
/// What the resolver refuses with; the moves not yet announced are gone (the string drains them first).
pub fn announce_staged_cards(sx: &mut Sx<'_>) -> Result<usize, TimingError> {
    if !has_staged_cards(sx.state) {
        return Ok(0);
    }
    let staged = drain_staged_cards(sx.state);
    let count = staged.len();
    let mark = fx::journal_mark(sx.resolver);
    for event in staged {
        match event {
            Staged::Discarded { who, card } => announce_discard(sx, who, card)?,
            Staged::Taken { taker, from, .. } => {
                // No card: which one moved is known to the taker and the owner only.
                let payload = Payload::new().with(Key::Player, Val::Seat(taker)).with(Key::From, Val::Seat(from));
                sx.emit(fx::event(EventKind::ActionCardTaken, payload))?;
            }
        }
    }
    fx::mirror_journal(sx.state, sx.resolver, mark);
    Ok(count)
}
