//! Typed readers the faction modules share: who is which faction, technology text, flagship text,
//! the board, and the typed marks.

use ti4_engine_core::SysSet;
use ti4_engine_core::generated::{Breakthrough, Faction, System, Technology, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Word};
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, Piece, Player};
use ti4_engine_core::tables::Scope;

/// The source scope of the game, as the tables' mask.
#[must_use]
pub fn scope(k: &Kernel) -> Scope {
    Scope(u32::from(k.sources.0))
}

/// The player in this seat.
#[must_use]
pub fn player(s: &CoreState, who: Seat) -> Option<&Player> {
    s.players().by_seat(who)
}

/// Whether the seat plays this faction.
#[must_use]
pub fn is_faction(s: &CoreState, who: Seat, faction: Faction) -> bool {
    player(s, who).is_some_and(|p| p.faction == faction)
}

/// `breakthroughs::holds`.
#[must_use]
pub fn holds_breakthrough(s: &CoreState, who: Seat, bt: Breakthrough) -> bool {
    player(s, who).is_some_and(|p| p.cards.breakthrough == Some(bt))
}

/// `technology::has_technology_text`.
#[must_use]
pub fn has_tech_text(s: &CoreState, who: Seat, tech: Technology) -> bool {
    ti4_engine_core::query::reach::technology_text(s, who, tech)
}

/// The flagship of a faction's roster (`flagship_of`: `<faction>_flagship`, `keleres_flagship`).
#[must_use]
pub fn flagship_of(faction: Faction) -> Option<Unit> {
    faction.data().units.iter().copied().find(|u| u.data().base_type == ti4_engine_core::generated::UnitBaseType::Flagship)
}

/// `nekro::z_assimilated_factions`.
fn z_assimilated(s: &CoreState, who: Seat) -> impl Iterator<Item = Faction> + '_ {
    let ok = is_faction(s, who, Faction::TheNekroVirus) && holds_breakthrough(s, who, Breakthrough::ValefarAssimilatorZ);
    let list = if ok { mark(s, MarkKey::NekroToken(who)) } else { None };
    let items: &[MarkVal] = match list {
        Some(MarkVal::List(items)) => items,
        _ => &[],
    };
    items.iter().filter_map(|v| match v {
        MarkVal::Atom(Atom::Faction(f)) => Some(*f),
        _ => None,
    })
}

/// `factions::flagship_has_text`: the unit is that flagship, or a Nekro flagship whose owner has placed the Z
/// token on that faction.
#[must_use]
pub fn flagship_has_text(s: &CoreState, owner: Seat, unit: Unit, flagship: Unit) -> bool {
    unit == flagship
        || (unit == Unit::TheAlastor && z_assimilated(s, owner).any(|f| flagship_of(f) == Some(flagship)))
}

/// A mark's value.
#[must_use]
pub fn mark(s: &CoreState, key: MarkKey) -> Option<&MarkVal> {
    s.marks().tail.get(&key)
}

/// A number atom.
#[must_use]
pub fn num(v: &MarkVal) -> Option<u32> {
    match v {
        MarkVal::Atom(Atom::Num(n)) => Some(*n),
        _ => None,
    }
}

/// A seat atom.
#[must_use]
pub fn seat(v: &MarkVal) -> Option<Seat> {
    match v {
        MarkVal::Atom(Atom::Seat(x)) => Some(*x),
        _ => None,
    }
}

/// A system atom.
#[must_use]
pub fn system(v: &MarkVal) -> Option<System> {
    match v {
        MarkVal::Atom(Atom::System(x)) => Some(*x),
        _ => None,
    }
}

/// A unit atom.
#[must_use]
pub fn unit(v: &MarkVal) -> Option<Unit> {
    match v {
        MarkVal::Atom(Atom::Unit(x)) => Some(*x),
        _ => None,
    }
}

/// The fields of a record value.
#[must_use]
pub fn rec(v: &MarkVal) -> Option<&[MarkVal]> {
    match v {
        MarkVal::Rec(f) => Some(f),
        _ => None,
    }
}

/// `<seq>|<seat>` stamped with this activation and naming this seat (Aetherstream, the Firmament agent).
#[must_use]
pub fn stamped_for(s: &CoreState, key: MarkKey, who: Seat) -> bool {
    mark(s, key).and_then(rec).is_some_and(|f| {
        matches!(f, [a, b] if num(a) == Some(s.meta().activation_seq) && seat(b) == Some(who))
    })
}

/// Whether `word` is the kind of a [`MarkKey::PrivateSeat`] key.
#[must_use]
pub fn private(s: &CoreState, who: Seat, kind: Word) -> Option<&MarkVal> {
    mark(s, MarkKey::PrivateSeat(who, kind))
}

/// Systems where some piece (space or planet) satisfies `pred`.
pub fn systems_with_piece(s: &CoreState, mut pred: impl FnMut(Piece) -> bool) -> SysSet {
    let mut out = SysSet::EMPTY;
    for (system, cell) in s.board().systems() {
        if cell.units.as_slice().iter().any(|p| pred(*p)) {
            out.insert(system.index());
        }
    }
    for (planet, cell) in s.board().planets() {
        if cell.units.as_slice().iter().any(|p| pred(*p))
            && let Some(system) = board::home_system(s, planet)
        {
            out.insert(system.index());
        }
    }
    out
}

/// Whether a piece in `system` (space or on one of its planets) satisfies `pred`.
pub fn any_piece_in(s: &CoreState, system: System, mut pred: impl FnMut(Piece) -> bool) -> bool {
    s.board().sys(system).units.as_slice().iter().any(|p| pred(*p))
        || board::planets_of(s, system).any(|planet| s.board().planet(planet).units.as_slice().iter().any(|p| pred(*p)))
}
