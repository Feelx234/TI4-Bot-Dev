//! The core state (docs/ENGINE_CORE_STATE.md 1.4): one `CoreState` of 17 slices, each its own struct
//! and file. Small fixed-size structs, bitsets for sets over generated enums, inline lists for
//! ordered lists, a chunked copy-on-write board and `Cow<Player>`s, append-only `Log`s and `Deck`s:
//! a clone is a few dozen refcount bumps and a memcpy of the inline part, no allocation.
//!
//! No rules live here. Every slice accessor of [`CoreState`] does `debug_assert!(loaded)`: the
//! adapter marks the slices a lend loaded (`Slice` / `SliceSet`), and a package that reads a slice
//! missing from its footprint fails in debug and in the dual run, not silently in production.

mod aftermath;
mod agenda;
mod board;
mod cards;
mod decks;
mod event;
mod explore;
mod ledger;
mod map;
mod marks;
mod meta;
mod objectives;
mod pending;
mod pending_windows;
mod plain;
mod piece;
mod player;
mod players;
mod rng;
mod seats;
mod strategy;
mod tactical;
mod trade;
mod transient;

pub use aftermath::{AStage, Aftermath, Before, NoteHold, PauseLimit};
pub use agenda::{AgendaCastRec, AgendaRec, AgendaState, Outcome, Prediction};
pub use board::{Board, PlanetCell, PlanetFlags, SysCell, SysFlags, UnitList};
pub use cards::{CardRef, CardZones, DeckReserve, DeckSegment, NoteKey, NoteMap, RESERVE_IDLE, RESERVE_LIVE, RESERVE_OPEN, ReserveDeck, ReservedDraw};
pub use decks::Decks;
pub use event::{Key, List, Payload, Tally, Val};
pub use explore::{Expedition, ExploreRec, ExploreState};
pub use ledger::{EventKind, EventRec, Ledger, LedgerReason, RelicRec, RollPurpose, RollRec, VpRec};
pub use map::MapSlice;
pub use marks::{Family, MarkKey, MarkMap, MarkVal, Marks};
pub use meta::{Meta, MetaFlags, Phase};
pub use objectives::{ObjectiveRef, ObjectiveSet, Objectives};
pub use pending_windows::{LeaderFollowers, StatusReport, Tokens, Trade, Vote, VoteStage, Voting};
pub use pending::{DriverFlags, Pending, PendingAsk, PendingTag, Scoring, ScoringTiming, Secondary, SecondaryHow, Window, WindowKind};
pub use piece::Piece;
#[cfg(test)]
pub(crate) use plain::kinds as queue_kinds;
pub use plain::{Emit, PlainArg, PlainKind, PlainRec, Shape, Slot as PlainSlot};
pub use player::{
    CardTimers, CombatTimers, ExtraVotes, Feat, FragmentCounts, LeaderStatus, Plot, PlotCard, Player, PlayerCards, PlayerEcon, RoundKind, RoundMarks,
    PlayerLeaders, PlayerMisc, PlayerTech, ProductionTimers, ReactionMode, TechSet, Timers,
};
pub use players::Players;
pub use rng::{Rng, RngDomain, SeedSource};
pub use seats::{MAX_PLAYERS, Seats};
pub use strategy::StrategyState;
pub use tactical::{TStage, Tactical};
pub use trade::*;
pub use transient::*;

use crate::containers::Cow;

/// An open-ended text of the string state that has no closed set yet (reason texts, presentation
/// phases, promise texts): an index into a table the adapter owns. Each use is listed in
/// `docs/ENGINE_CORE_STATE_FIELDS.md`; package X1 replaces a `Code` by an enum when the set closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Code(pub u16);

/// The slices of [`CoreState`], in the order of `Footprint` bits (F2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Slice {
    Meta,
    Seats,
    Players,
    Board,
    Map,
    Decks,
    Cards,
    Objectives,
    Agenda,
    Strategy,
    Explore,
    Trade,
    Marks,
    Transient,
    Pending,
    Rng,
    Ledger,
}

impl Slice {
    pub const COUNT: usize = 17;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Meta,
        Self::Seats,
        Self::Players,
        Self::Board,
        Self::Map,
        Self::Decks,
        Self::Cards,
        Self::Objectives,
        Self::Agenda,
        Self::Strategy,
        Self::Explore,
        Self::Trade,
        Self::Marks,
        Self::Transient,
        Self::Pending,
        Self::Rng,
        Self::Ledger,
    ];
}

/// A set of slices (a footprint's reads or writes, or the loaded set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SliceSet(pub u32);

impl SliceSet {
    pub const EMPTY: Self = Self(0);
    pub const ALL: Self = Self((1 << Slice::COUNT) - 1);

    #[must_use]
    pub const fn of(slices: &[Slice]) -> Self {
        let mut bits = 0;
        let mut i = 0;
        while i < slices.len() {
            bits |= 1 << slices[i] as u32;
            i += 1;
        }
        Self(bits)
    }

    #[must_use]
    pub const fn has(self, slice: Slice) -> bool {
        self.0 & (1 << slice as u32) != 0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn insert(&mut self, slice: Slice) {
        self.0 |= 1 << slice as u32;
    }
}

/// The whole game state. `Clone` is the checkpoint.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CoreState {
    pub meta: Meta,
    pub seats: Seats,
    pub players: Players,
    pub board: Board,
    pub map: MapSlice,
    pub decks: Decks,
    pub cards: CardZones,
    pub objectives: Objectives,
    pub agenda: AgendaState,
    pub strategy: StrategyState,
    pub explore: ExploreState,
    pub trade: TradeState,
    pub marks: Marks,
    pub transient: Transient,
    pub pending: Pending,
    pub rng: Rng,
    pub ledger: Ledger,
    /// Which slices hold real data (the others are defaults). Not part of the game: excluded from `==`
    /// would be wrong for tests, so it is compared too; checkpoints keep it.
    pub loaded: SliceSet,
}

macro_rules! accessors {
    ($($field:ident, $field_mut:ident, $slice:ident, $ty:ty;)*) => {
        impl CoreState {$(
            #[inline]
            #[must_use]
            pub fn $field(&self) -> &$ty {
                debug_assert!(self.loaded.has(Slice::$slice));
                &self.$field
            }

            #[inline]
            pub fn $field_mut(&mut self) -> &mut $ty {
                debug_assert!(self.loaded.has(Slice::$slice));
                &mut self.$field
            }
        )*}
    };
}

accessors! {
    meta, meta_mut, Meta, Meta;
    seats, seats_mut, Seats, Seats;
    players, players_mut, Players, Players;
    board, board_mut, Board, Board;
    map, map_mut, Map, MapSlice;
    decks, decks_mut, Decks, Decks;
    cards, cards_mut, Cards, CardZones;
    objectives, objectives_mut, Objectives, Objectives;
    agenda, agenda_mut, Agenda, AgendaState;
    strategy, strategy_mut, Strategy, StrategyState;
    explore, explore_mut, Explore, ExploreState;
    trade, trade_mut, Trade, TradeState;
    marks, marks_mut, Marks, Marks;
    transient, transient_mut, Transient, Transient;
    pending, pending_mut, Pending, Pending;
    rng, rng_mut, Rng, Rng;
    ledger, ledger_mut, Ledger, Ledger;
}

impl CoreState {
    /// A default state with every slice marked loaded (tests, new games built in the core).
    #[must_use]
    pub fn all_loaded() -> Self {
        Self { loaded: SliceSet::ALL, ..Self::default() }
    }

    /// A default state with only `slices` marked loaded (what a lend starts from).
    #[must_use]
    pub fn with_loaded(slices: SliceSet) -> Self {
        Self { loaded: slices, ..Self::default() }
    }

    pub fn mark_loaded(&mut self, slice: Slice) {
        self.loaded.insert(slice);
    }

    /// The galaxy of the map slice, if the adapter loaded one.
    #[must_use]
    pub fn galaxy(&self) -> Option<&Cow<crate::galaxy::CoreGalaxy>> {
        debug_assert!(self.loaded.has(Slice::Map));
        self.map.galaxy.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{Faction, System, Unit};
    use crate::ids::Seat;

    fn state() -> CoreState {
        let mut s = CoreState::all_loaded();
        let players = (0..6u8).map(|i| Player::new(Seat(i), Faction::ALL[usize::from(i)])).collect();
        s.players = Players::from_vec(players);
        s
    }

    #[test]
    fn clone_shares_players_board_logs_and_rng() {
        let mut a = state();
        a.ledger.events.extend((0..70).map(|id| Emit::Typed(EventRec { id, kind: EventKind::TurnBegan, payload: Payload::new(), cancelled: false })));
        let b = a.clone();
        assert_eq!(a.players.shared_with(&b.players), 6);
        assert_eq!(a.board.shared_chunks(&b.board), 25);
        assert!(a.ledger.events.shares_prefix_with(&b.ledger.events));
        // A write copies one player and one board chunk only.
        a.players_mut().edit(2).econ.trade_goods = 3;
        a.board_mut().sys_mut(System::ALL[40]).units.push(Piece::new(Unit::ALL[0], Seat(1)));
        assert_eq!(a.players.shared_with(&b.players), 5);
        assert_eq!(a.board.shared_chunks(&b.board), 24);
        assert_eq!(b.players.get(2).econ.trade_goods, 0);
        assert!(b.board.sys(System::ALL[40]).units.is_empty());
        assert_ne!(a, b);
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn reading_an_unloaded_slice_fails_in_debug() {
        let s = CoreState::with_loaded(SliceSet::of(&[Slice::Meta]));
        let _ = s.meta();
        let _ = s.board();
    }
}
