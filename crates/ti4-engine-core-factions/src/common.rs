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
    let plays = player(s, who).is_some_and(|p| p.faction == faction);
    if plays {
        // Last Bastion, Crimson, Keleres, Deepwrought, Firmament, Obsidian and Ral Nel are switched off
        // (docs/DISABLE_XF123.md): a panic under `Policy::Panic`, dead code after the flip (D2-8), deleted in D4.
        ti4_engine_core::unsupported::faction_in_play(faction);
    }
    plays
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

/// The one test kit of the faction packages (their per-package `testkit` re-exports it).
#[cfg(test)]
pub(crate) mod testkit {
    use ti4_engine_core::generated::Faction;
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::kernel::Kernel;
    use ti4_engine_core::state::{CoreState, Player, Players};

    /// A state with one player per faction (seat i plays `factions[i]`), every slice loaded, and a kernel with no modules.
    pub fn table(factions: &[Faction]) -> (CoreState, Kernel) {
        let mut s = CoreState::all_loaded();
        *s.players_mut() = Players::from_vec(factions.iter().enumerate().map(|(i, f)| Player::new(Seat(i as u8), *f)).collect());
        let seats: Vec<Seat> = (0..factions.len() as u8).map(Seat).collect();
        *s.seats_mut() = ti4_engine_core::state::Seats::new(&seats, &seats, factions.len() as u8);
        (s, Kernel::plain())
    }
}

/// `technology::apply_unit_upgrades` on the core state: the units of `who` on the board take the form the seat is entitled to
/// (the upgrade it owns, else its faction's own unit, else the plain one). The faction override of the form (Mentak's Corsair)
/// is the caller's to exclude.
pub fn apply_unit_upgrades(s: &mut CoreState, scope: ti4_engine_core::tables::Scope, who: Seat) {
    use ti4_engine_core::generated::{Requirement, System};
    use ti4_engine_core::query::tech;
    let roster = crate::cd_registry::yin::timing::roster(scope);
    let Some(p) = s.players().by_seat(who) else { return };
    let (faction, owned) = (p.faction, p.tech.owned.clone());
    let chosen = |kind: Unit| -> Unit {
        let d = kind.data();
        tech::unlocked_upgrade(scope, d.base_type, faction, &owned)
            .or_else(|| if d.faction.is_none() && d.required_technology == Requirement::None { tech::faction_unit(&roster, faction, d.base_type) } else { None })
            .unwrap_or(kind)
    };
    let swap = |units: &mut [ti4_engine_core::state::Piece]| {
        for piece in units.iter_mut().filter(|p| p.owner() == who && scope.unit(p.kind()).is_some()) {
            let better = chosen(piece.kind());
            if better != piece.kind() {
                *piece = piece.with_kind(better);
            }
        }
    };
    for system in System::ALL {
        swap(s.board_mut().sys_mut(system).units.as_mut_slice());
        let planets: Vec<_> = board::planets_of(s, system).collect();
        for planet in planets {
            swap(s.board_mut().planet_mut(planet).units.as_mut_slice());
        }
    }
}

#[cfg(test)]
mod upgrade_tests {
    use super::*;
    use ti4_engine_core::generated::Planet;

    #[test]
    fn an_owned_upgrade_reaches_the_seats_units_on_the_board_and_only_those() {
        let (mut s, _) = testkit::table(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]);
        let (system, planet) = (System::ALL[0], Planet::ALL[0]);
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::CruiserI, Seat(0)));
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::CruiserI, Seat(1)));
        s.board_mut().planet_mut(planet).units.push(Piece::new(Unit::InfantryI, Seat(0)));
        let scope = Scope(u32::MAX);
        apply_unit_upgrades(&mut s, scope, Seat(0));
        assert_eq!(s.board().sys(system).units.as_slice().iter().map(|p| (p.owner(), p.kind())).collect::<Vec<_>>(), vec![(Seat(0), Unit::CruiserI), (Seat(1), Unit::CruiserI)], "no upgrade owned: nothing changes");
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::CruiserII);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::InfantryII);
        apply_unit_upgrades(&mut s, scope, Seat(0));
        assert_eq!(s.board().sys(system).units.as_slice().iter().map(|p| (p.owner(), p.kind())).collect::<Vec<_>>(), vec![(Seat(0), Unit::CruiserII), (Seat(1), Unit::CruiserI)], "the owner's only, in place");
        assert_eq!(s.board().planet(planet).units.as_slice()[0].kind(), Unit::InfantryII, "a planet's units too");
    }
}

#[cfg(test)]
mod unsupported_tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::unsupported::{Policy, with_policy};

    fn panics_with(name: &str, f: impl FnOnce()) -> bool {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        std::panic::set_hook(prev);
        r.err().is_some_and(|e| {
            e.downcast_ref::<String>().is_some_and(|t| t.contains(name))
                || e.downcast_ref::<&str>().is_some_and(|t| t.contains(name))
        })
    }

    #[test]
    fn is_faction_panics_for_each_switched_off_faction_and_only_for_those() {
        for (f, name) in [
            (Faction::LastBastion, "LastBastion"),
            (Faction::TheCrimsonRebellion, "Crimson"),
            (Faction::TheCouncilKeleresArgent, "Keleres"),
            (Faction::TheDeepwroughtScholarate, "Deepwrought"),
            (Faction::TheFirmament, "Firmament"),
            (Faction::TheObsidian, "Obsidian"),
            (Faction::TheRalNelConsortium, "RalNel"),
        ] {
            let (s, _) = table(&[Faction::TheArborec, f]);
            assert!(with_policy(Policy::Off, || is_faction(&s, Seat(1), f)), "{f:?} off");
            assert!(panics_with(name, || with_policy(Policy::Panic, || { is_faction(&s, Seat(1), f); })), "{f:?}");
            // asking about another faction at the same table does not touch the switched-off seat
            assert!(!with_policy(Policy::Panic, || is_faction(&s, Seat(0), Faction::TheEmiratesOfHacan)));
        }
    }
}
