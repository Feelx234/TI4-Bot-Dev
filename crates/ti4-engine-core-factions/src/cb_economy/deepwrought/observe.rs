//! deepwrought: tier O: observers and bookkeeping (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CB). Owned by CB only.
//!
//! | string call | tier | target |
//! |---|---|---|
//! | `note_coexistence_began` (Oceanbound) | O | `direct.deepwrought_note_coexistence_began` |
//! | `begin_coexisting` (2 call sites in `invasion`) | O, BLOCKED | `direct.deepwrought_begin_coexisting` is a `call` that runs `coexistence::begin` (a board edit with errors) first |

use ti4_engine_core::generated::{Faction, Planet};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Cx;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::state::{CoreState, PlanetFlags};

use crate::common::is_faction;

/// The five ocean cards in id order (`ocean1` .. `ocean5`).
pub const OCEANS: [Planet; 5] = [Planet::CoralReef, Planet::LostFleet, Planet::DeepAbyss, Planet::BrinePool, Planet::IceShelf];

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.direct.deepwrought_note_coexistence_began.push(note_coexistence_began);
}

fn holder_of(s: &CoreState, planet: Planet) -> Option<Seat> {
    match s.marks().tail.get(&MarkKey::DeepwroughtOcean(planet)) {
        Some(MarkVal::Atom(Atom::Seat(holder))) => Some(*holder),
        _ => None,
    }
}

fn stored(s: &CoreState, who: Seat) -> Vec<Planet> {
    s.marks()
        .tail
        .iter()
        .filter_map(|(key, value)| match (key, value) {
            (MarkKey::DeepwroughtOcean(planet), MarkVal::Atom(Atom::Seat(holder))) if *holder == who => Some(*planet),
            _ => None,
        })
        .collect()
}

fn ready(s: &mut CoreState, planet: Planet) {
    s.board_mut().planet_mut(planet).flags.set(PlanetFlags::EXHAUSTED, false);
}

/// Oceanbound's discard, made real: delete every row beyond the cap.
fn prune_oceans(s: &mut CoreState, who: Seat) {
    let keep = coexisting(s, who);
    for planet in stored(s, who).into_iter().skip(keep) {
        s.marks_mut().tail.edit().remove(&MarkKey::DeepwroughtOcean(planet));
        ready(s, planet);
    }
}

fn coexisting(s: &CoreState, who: Seat) -> usize {
    s.board().planets().filter(|(_, cell)| cell.coexist.contains(who) && cell.units.as_slice().iter().any(|u| u.owner() == who)).count()
}

/// Gain the lowest free ocean card, readied. `None` when all five are taken or the cap would discard it at once.
pub fn gain_ocean(s: &mut CoreState, who: Seat) -> Option<Planet> {
    prune_oceans(s, who);
    if coexisting(s, who) <= stored(s, who).len() {
        return None;
    }
    let next = OCEANS.into_iter().find(|p| holder_of(s, *p).is_none() && !s.marks().tail.contains_key(&MarkKey::DeepwroughtOcean(*p)))?;
    s.marks_mut().tail.edit().insert(MarkKey::DeepwroughtOcean(next), MarkVal::Atom(Atom::Seat(who)));
    ready(s, next);
    Some(next)
}

/// Oceanbound: "When your units begin coexisting on a planet: gain an ocean card and ready it." A no-op for every
/// other faction.
pub fn note_coexistence_began(cx: &mut Cx<'_>, who: Seat) {
    if is_faction(cx.state, who, Faction::TheDeepwroughtScholarate) {
        gain_ocean(cx.state, who);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cb_economy::testkit::table;
    use ti4_engine_core::generated::Unit;
    use ti4_engine_core::state::Piece;

    fn coexist(s: &mut CoreState, who: Seat, planet: Planet) {
        let cell = s.board_mut().planet_mut(planet);
        cell.coexist.insert(who);
        cell.units.push(Piece::new(Unit::InfantryI, who));
    }

    #[test]
    fn oceanbound_gains_the_lowest_free_card_up_to_the_cap() {
        let (mut s, k) = table(&[Faction::TheDeepwroughtScholarate, Faction::TheFederationOfSol]);
        let mut ask = ti4_engine_core::kernel::FirstOption;
        note_coexistence_began(&mut Cx::new(&mut s, &k, &mut ask), Seat(0));
        assert!(holder_of(&s, Planet::CoralReef).is_none(), "nothing coexists yet: the card would be discarded at once");
        coexist(&mut s, Seat(0), Planet::Abaddon);
        note_coexistence_began(&mut Cx::new(&mut s, &k, &mut ask), Seat(0));
        assert_eq!(holder_of(&s, Planet::CoralReef), Some(Seat(0)));
        note_coexistence_began(&mut Cx::new(&mut s, &k, &mut ask), Seat(0));
        assert!(holder_of(&s, Planet::LostFleet).is_none(), "one coexisting planet, one card");
        coexist(&mut s, Seat(0), Planet::Abyz);
        note_coexistence_began(&mut Cx::new(&mut s, &k, &mut ask), Seat(0));
        assert_eq!(holder_of(&s, Planet::LostFleet), Some(Seat(0)));
        // Sol never gains one.
        coexist(&mut s, Seat(1), Planet::Accoen);
        note_coexistence_began(&mut Cx::new(&mut s, &k, &mut ask), Seat(1));
        assert!(stored(&s, Seat(1)).is_empty());
    }
}
