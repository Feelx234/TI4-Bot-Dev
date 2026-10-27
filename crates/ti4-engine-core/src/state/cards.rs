//! `CardZones`: discard piles, the salvage yard, the deck reserve and the promissory note table.

use crate::containers::{Cow, SortedMap};
use crate::generated::{ActionCard, Agenda, Explore, ExploreType, Faction, PromissoryNote, PublicObjective, Relic, SecretObjective};
use crate::ids::Seat;

/// A promissory note instance: the printed note and the faction it belongs to
/// (`GameState.promissory_notes` keys are `"{alias}:{faction}"`). The typed order is
/// `(note, faction)`, not the text order of the id; the string side re-sorts on store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoteKey {
    pub note: PromissoryNote,
    pub faction: Faction,
}

/// The table fits 8 players with their own notes inline; larger games spill.
pub type NoteMap<V> = SortedMap<NoteKey, V, 64>;

/// `deck_reserve::ReservedDraw.deck`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReserveDeck {
    Action,
    Secret,
    Agenda,
    Relic,
    Objective,
    Explore(ExploreType),
}

/// `deck_reserve::ReservedDraw.card`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardRef {
    Action(ActionCard),
    Secret(SecretObjective),
    Agenda(Agenda),
    Relic(Relic),
    Objective(PublicObjective),
    Explore(Explore),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReservedDraw {
    pub tag: u32,
    pub deck: ReserveDeck,
    pub card: CardRef,
    pub recipient: Option<Seat>,
}

/// Marks of the reserve cursor (the string `deck_reserve::{IDLE, LIVE, OPEN}`).
pub const RESERVE_IDLE: u32 = u32::MAX;
pub const RESERVE_LIVE: u32 = u32::MAX - 1;
pub const RESERVE_OPEN: u32 = u32::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckSegment {
    pub from: u32,
    pub until: u32,
    pub reserved: Vec<ReservedDraw>,
}

/// `deck_reserve::DeckReserve` (`#[serde(skip)]` in `GameState`; the replay harness forces draws).
/// The string cursor is a shared atomic; here it is a plain value that the adapter syncs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeckReserve {
    pub cursor: u32,
    pub pending: Vec<DeckSegment>,
    pub draws: Vec<ReservedDraw>,
    pub failure: Option<ReservedDraw>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CardZones {
    pub discarded_action: Cow<Vec<ActionCard>>,
    pub salvage_yard: Cow<Vec<ActionCard>>,
    pub reserve: Option<Cow<DeckReserve>>,
    /// `GameState.promissory_notes`: note -> holder.
    pub promissory: Cow<NoteMap<Seat>>,
    /// `GameState.promissory_faceup`.
    pub promissory_faceup: Cow<NoteMap<()>>,
}
