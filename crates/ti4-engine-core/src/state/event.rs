//! Typed event records: what an emitted event carries (the string `Event` has `id`, `event_type`, a
//! `BTreeMap<String, Value>` payload and a `cancelled` bit).
//!
//! The typed payload is a short list of `(Key, Val)`: the keys are the 74 payload names found at the emit
//! sites of the string engine (`Key::Other` for a name outside the set, adapter-interned), the values are
//! typed (seat, generated id, number, flag). Insertion order is kept; the wire order of the string payload
//! is the key TEXT order, which the adapter restores when it renders (the typed code never depends on the
//! order). Inline up to six entries.

use crate::containers::SmallList;
use crate::generated::{ActionCard, Agenda, Leader, Planet, StrategyCard, System, Technology, Unit};
use crate::ids::Seat;

use super::{Code, Outcome};

/// A payload key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    Player,
    System,
    Planet,
    Unit,
    Attacker,
    Defender,
    Winner,
    Cause,
    Card,
    Breakthrough,
    Hits,
    DuringSpaceCombat,
    Target,
    ShipsMoved,
    Galvanized,
    To,
    Technology,
    Partner,
    Leader,
    Count,
    Amount,
    Wormholes,
    Source,
    RoundSeq,
    Round,
    Owner,
    OtherParty,
    Origin,
    Objective,
    Last,
    Goods,
    DirectHittable,
    Component,
    Commodities,
    CancelledByRule,
    Agenda,
    Damaged,
    Cost,
    PreviousOwner,
    Reason,
    Promissory,
    Kind,
    CurrentVotes,
    Worth,
    Proposer,
    Pool,
    Note,
    From,
    Destination,
    ActionCard,
    UnitsRemovedAfter,
    Turn,
    TheirNet,
    Secret,
    Rounds,
    Relic,
    Producer,
    PickupSystem,
    Path,
    Net,
    MovesTo,
    MahactInfantry,
    Id,
    Gunner,
    Gift,
    FromSystem,
    FromPlanet,
    Fragment,
    Event,
    Destroyed,
    Deck,
    Controller,
    ControlChanged,
    ElectedPlayer,
    /// A key outside the set (adapter-interned text).
    Other(Code),
}

impl Key {
    /// The closed keys in declaration order.
    pub const ALL: [Self; 74] = [
        Self::Player,
        Self::System,
        Self::Planet,
        Self::Unit,
        Self::Attacker,
        Self::Defender,
        Self::Winner,
        Self::Cause,
        Self::Card,
        Self::Breakthrough,
        Self::Hits,
        Self::DuringSpaceCombat,
        Self::Target,
        Self::ShipsMoved,
        Self::Galvanized,
        Self::To,
        Self::Technology,
        Self::Partner,
        Self::Leader,
        Self::Count,
        Self::Amount,
        Self::Wormholes,
        Self::Source,
        Self::RoundSeq,
        Self::Round,
        Self::Owner,
        Self::OtherParty,
        Self::Origin,
        Self::Objective,
        Self::Last,
        Self::Goods,
        Self::DirectHittable,
        Self::Component,
        Self::Commodities,
        Self::CancelledByRule,
        Self::Agenda,
        Self::Damaged,
        Self::Cost,
        Self::PreviousOwner,
        Self::Reason,
        Self::Promissory,
        Self::Kind,
        Self::CurrentVotes,
        Self::Worth,
        Self::Proposer,
        Self::Pool,
        Self::Note,
        Self::From,
        Self::Destination,
        Self::ActionCard,
        Self::UnitsRemovedAfter,
        Self::Turn,
        Self::TheirNet,
        Self::Secret,
        Self::Rounds,
        Self::Relic,
        Self::Producer,
        Self::PickupSystem,
        Self::Path,
        Self::Net,
        Self::MovesTo,
        Self::MahactInfantry,
        Self::Id,
        Self::Gunner,
        Self::Gift,
        Self::FromSystem,
        Self::FromPlanet,
        Self::Fragment,
        Self::Event,
        Self::Destroyed,
        Self::Deck,
        Self::Controller,
        Self::ControlChanged,
        Self::ElectedPlayer,
    ];
}

/// A payload value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Val {
    Seat(Seat),
    System(System),
    Planet(Planet),
    Unit(Unit),
    Action(ActionCard),
    Strategy(StrategyCard),
    Agenda(Agenda),
    Leader(Leader),
    Tech(Technology),
    /// An agenda outcome (`AGENDA_RESOLVED`'s `player`: a seat, `for`/`against`, a law, a planet).
    Outcome(Outcome),
    Int(i64),
    Flag(bool),
    /// A text outside every closed set (adapter-interned).
    Code(Code),
}

/// One `{owner, unit_type, count}` entry of an event's `units` list (a tally of units in a system).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tally {
    pub owner: Seat,
    pub unit: Unit,
    pub count: u32,
}

/// The facts an event carries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Payload {
    pub items: SmallList<(Key, Val), 6>,
    /// The `units` array of events that list units (`UNITS_COMMITTED`, `SHIP_DESTROYED`, ...).
    pub units: SmallList<Tally, 4>,
    /// List-valued entries (`SPACE_COMBAT_ENDED`'s `losers`, `retreated`, `destroyed`), in insertion order.
    pub lists: Vec<(Key, List)>,
}

/// A list-valued payload entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum List {
    /// An array of players (the seat texts).
    Seats(SmallList<Seat, 4>),
    /// An array of `{player, unit}` objects.
    Units(SmallList<(Seat, Unit), 8>),
    /// An array of planet ids (`GROUND_COMMITMENT_FINISHED`'s `planets`).
    Planets(SmallList<Planet, 8>),
}

macro_rules! typed_get {
    ($($name:ident, $variant:ident, $ty:ty;)*) => {$(
        #[must_use]
        pub fn $name(&self, key: Key) -> Option<$ty> {
            match self.get(key)? {
                Val::$variant(x) => Some(x),
                _ => None,
            }
        }
    )*};
}

impl Payload {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a field (the string `BTreeMap::insert`: a second insert of a key replaces the first).
    pub fn set(&mut self, key: Key, val: Val) {
        if let Some(slot) = self.items.as_mut_slice().iter_mut().find(|(k, _)| *k == key) {
            slot.1 = val;
        } else {
            self.items.push((key, val));
        }
    }

    #[must_use]
    pub fn with(mut self, key: Key, val: Val) -> Self {
        self.set(key, val);
        self
    }

    /// Add a list entry (a second insert of a key replaces the first).
    pub fn set_list(&mut self, key: Key, list: List) {
        if let Some(slot) = self.lists.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = list;
        } else {
            self.lists.push((key, list));
        }
    }

    #[must_use]
    pub fn with_list(mut self, key: Key, list: List) -> Self {
        self.set_list(key, list);
        self
    }

    #[must_use]
    pub fn list(&self, key: Key) -> Option<&List> {
        self.lists.iter().find(|(k, _)| *k == key).map(|(_, l)| l)
    }

    #[must_use]
    pub fn get(&self, key: Key) -> Option<Val> {
        self.items.as_slice().iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }

    typed_get! {
        seat, Seat, Seat;
        system, System, System;
        planet, Planet, Planet;
        unit, Unit, Unit;
        action, Action, ActionCard;
        strategy, Strategy, StrategyCard;
        agenda, Agenda, Agenda;
        leader, Leader, Leader;
        tech, Tech, Technology;
        outcome, Outcome, Outcome;
        int, Int, i64;
        flag, Flag, bool;
        code, Code, Code;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_replaces_like_a_map_and_get_reads_typed() {
        let mut p = Payload::new().with(Key::Player, Val::Seat(Seat(2))).with(Key::Hits, Val::Int(3));
        assert_eq!(p.seat(Key::Player), Some(Seat(2)));
        assert_eq!(p.int(Key::Hits), Some(3));
        assert_eq!(p.seat(Key::Hits), None, "wrong type reads as absent");
        p.set(Key::Hits, Val::Int(4));
        assert_eq!(p.int(Key::Hits), Some(4));
        assert_eq!(p.items.len(), 2);
    }
}
