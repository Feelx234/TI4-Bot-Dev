//! Promissory notes in hand and in play areas (`promissory.rs`).
//!
//! Package B5. Pure reads over `&CoreState` (slices `Meta`, `Seats`, `Players`, `Board`, `Cards`).
//! A note is a [`NoteKey`] (the printed note and the faction that owns it); the string engine's
//! `"{alias}:{faction}"` ids map onto it one to one. The typed order of keys is `(note, faction)`, not
//! the text order of the ids, so a function that returns several keys returns them in the typed
//! order and the adapter re-sorts by text where the string engine's order is observable.
//!
//! Not here: dealing, giving back, playing a note (mutations), `available_notes` and the commander
//! checks (they read leaders), the Support for the Throne table (its own string field).

use crate::generated::{Faction, PromissoryNote, System};
use crate::ids::Seat;
use crate::state::{CoreState, NoteKey};

use super::board;

/// Rules version from which the Mentak's Promise of Protection is an ACTION note
/// (`ti4_model::state::RULES_FACTION_EFFECTS`).
pub const RULES_FACTION_EFFECTS: u32 = 16;

/// The faction a seat plays.
#[must_use]
pub fn faction_of(core: &CoreState, seat: Seat) -> Option<Faction> {
    core.players().by_seat(seat).map(|p| p.faction)
}

/// Whether the note is placed faceup by its own ACTION rather than on receipt
/// (`promissory::is_action_placed`).
#[must_use]
pub fn is_action_placed(core: &CoreState, note: PromissoryNote) -> bool {
    matches!(
        note,
        PromissoryNote::TradeConvoys
            | PromissoryNote::BloodPact
            | PromissoryNote::DarkPact
            | PromissoryNote::ShareKnowledge
            | PromissoryNote::Sever
    ) || (note == PromissoryNote::PromiseOfProtection && core.meta().rules_at_least(RULES_FACTION_EFFECTS))
}

/// `promissory::is_play_area`: the printed note goes faceup in a play area, for this owner (a faction
/// record binds to its own faction, a generic record applies to every owner).
#[must_use]
pub fn is_play_area(note: PromissoryNote, owner: Faction) -> bool {
    let d = &crate::generated::PROMISSORY_NOTE_DATA[note as usize];
    match d.faction {
        Some(tag) => d.play_area && matches!(tag.factions(), [only] if *only == owner),
        None => d.play_area,
    }
}

/// `promissory::held_by`: every note the seat holds, in typed order.
#[must_use]
pub fn held_by(core: &CoreState, seat: Seat) -> Vec<NoteKey> {
    core.cards().promissory.iter().filter(|(_, h)| **h == seat).map(|(k, _)| *k).collect()
}

/// `promissory::holder_of`: who holds `owner`'s copy of the note, `None` when the owner (or a seat of
/// the same faction) has it.
#[must_use]
pub fn holder_of(core: &CoreState, note: PromissoryNote, owner: Seat) -> Option<Seat> {
    let faction = faction_of(core, owner)?;
    let holder = *core.cards().promissory.get(&NoteKey { note, faction })?;
    (faction_of(core, holder) != Some(faction)).then_some(holder)
}

/// `promissory::held_foreign`: how many of other factions' copies of the note the seat holds.
#[must_use]
pub fn held_foreign(core: &CoreState, seat: Seat, note: PromissoryNote) -> usize {
    let own = faction_of(core, seat);
    core.cards().promissory.iter().filter(|(k, h)| **h == seat && k.note == note && Some(k.faction) != own).count()
}

/// `promissory::action_notes_in_hand`: notes in hand, not faceup, whose ACTION places them in the
/// play area; not the seat's own copies. Typed order.
#[must_use]
pub fn action_notes_in_hand(core: &CoreState, seat: Seat) -> Vec<NoteKey> {
    let own = faction_of(core, seat);
    let cards = core.cards();
    cards
        .promissory
        .iter()
        .filter(|(k, h)| {
            **h == seat
                && is_action_placed(core, k.note)
                && !cards.promissory_faceup.contains_key(k)
                && Some(k.faction) != own
        })
        .map(|(k, _)| *k)
        .collect()
}

/// `promissory::reaches_anyone`: the seat holds a faceup Trade Convoys.
#[must_use]
pub fn reaches_anyone(core: &CoreState, seat: Seat) -> bool {
    let cards = core.cards();
    cards
        .promissory
        .iter()
        .any(|(k, h)| *h == seat && k.note == PromissoryNote::TradeConvoys && cards.promissory_faceup.contains_key(k))
}

/// `promissory::may_receive`: Mahact's Hubris, no other player gives you their Alliance.
#[must_use]
pub fn may_receive(core: &CoreState, receiver: Seat, note: PromissoryNote) -> bool {
    !(note == PromissoryNote::Alliance && faction_of(core, receiver) == Some(Faction::TheMahactGeneSorcerers))
}

/// `promissory::denies_movement_into`: the mover's Ceasefire is lent to a seat that has units in the
/// system.
#[must_use]
pub fn denies_movement_into(core: &CoreState, mover: Seat, system: System) -> bool {
    let Some(faction) = faction_of(core, mover) else { return false };
    let Some(holder) = core.cards().promissory.get(&NoteKey { note: PromissoryNote::Ceasefire, faction }) else {
        return false;
    };
    let holder = *holder;
    if holder == mover || faction_of(core, holder) == Some(faction) {
        return false;
    }
    board::has_space_units_of(core, system, holder)
        || board::planets_of(core, system).any(|p| board::planet_units(core, p).iter().any(|u| u.owner() == holder))
}
