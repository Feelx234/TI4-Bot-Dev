//! Exploration, legendary planets and the Fracture (`exploration.rs`, `legendary.rs`, `fracture.rs`).
//!
//! Package B5. Printed data (no state): the exploration traits of a planet, the Fracture systems.
//! Position reads over `&CoreState`: purgeable fragment traits, legendary ability cards. What is not
//! here: draws, the deck order and every effect that mutates (reads only, like A1/A3/A4).

use crate::containers::SmallList;
use crate::generated::{ActionCard, Attachment, Explore, ExploreResolution, ExploreType, Planet, PlanetTrait, System, TileBack, Token};
use crate::ids::Seat;
use crate::state::CoreState;
use crate::tables::Scope;

use super::board;

/// `exploration::FRAGMENTS_PER_RELIC`.
pub const FRAGMENTS_PER_RELIC: i32 = 3;

/// The exploration deck a printed planet trait feeds (frontier is not a planet trait).
#[must_use]
pub const fn deck_of(t: PlanetTrait) -> Option<ExploreType> {
    match t {
        PlanetTrait::Cultural => Some(ExploreType::Cultural),
        PlanetTrait::Hazardous => Some(ExploreType::Hazardous),
        PlanetTrait::Industrial => Some(ExploreType::Industrial),
        _ => None,
    }
}

/// `exploration::traits_of`: the planet's printed exploration traits in printed order; empty when
/// the planet is outside the source scope.
#[must_use]
pub fn traits_of(scope: Scope, planet: Planet) -> SmallList<ExploreType, 3> {
    let mut out = SmallList::new();
    let data = planet.data();
    if crate::tables::source_bit(data.source) & scope.0 == 0 {
        return out;
    }
    for t in data.traits {
        if let Some(d) = deck_of(*t) {
            out.push(d);
        }
    }
    out
}

/// `exploration::purgeable`: the traits the seat could purge three fragments of for a relic, in key
/// order (cultural, hazardous, industrial). Frontier fragments stand in for any trait.
#[must_use]
pub fn purgeable(core: &CoreState, seat: Seat) -> SmallList<ExploreType, 3> {
    let mut out = SmallList::new();
    let Some(p) = core.players().by_seat(seat) else { return out };
    let f = &p.cards.fragments;
    let frontier = i32::from(f.get(ExploreType::Frontier).unwrap_or(0));
    for kind in [ExploreType::Cultural, ExploreType::Hazardous, ExploreType::Industrial] {
        if f.get(kind).is_some_and(|n| i32::from(n) + frontier >= FRAGMENTS_PER_RELIC) {
            out.push(kind);
        }
    }
    out
}

// --- exploration cards (printed data) ---------------------------------------------------------

/// `exploration::resolution`: how the card resolves.
#[must_use]
pub fn resolution(card: Explore) -> ExploreResolution {
    card.data().resolution
}

/// What an exploration card puts on the board: its `attachmentId`, an attachment or a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placed {
    Attachment(Attachment),
    Token(Token),
}

/// The record `attachmentId` names, `None` when the card names none.
#[must_use]
pub fn placed(card: Explore) -> Option<Placed> {
    let d = card.data();
    d.attachment.map(Placed::Attachment).or(d.token.map(Placed::Token))
}

/// Whether the planet prints a technology specialty (the research facility's two forms).
#[must_use]
pub fn has_specialty(planet: Planet) -> bool {
    !planet.data().specialties.is_empty()
}

// --- the Fracture (printed data) ---------------------------------------------------------------

/// `fracture::is_fracture_system`.
#[must_use]
pub fn is_fracture_system(scope: Scope, system: System) -> bool {
    scope.system(system).is_some_and(|d| d.tile_back == Some(TileBack::Fracture))
}

/// `fracture::systems`: every Fracture system in the scope, in system order.
#[must_use]
pub fn fracture_systems(scope: Scope) -> SmallList<System, 8> {
    let mut out = SmallList::new();
    for s in System::ALL {
        if is_fracture_system(scope, s) {
            out.push(s);
        }
    }
    out
}

/// `fracture::egress_systems`: the Fracture systems printed as an egress.
#[must_use]
pub fn egress_systems(scope: Scope) -> SmallList<System, 2> {
    let mut out = SmallList::new();
    for s in [System::FractureEgressLeft, System::FractureEgressRight] {
        if is_fracture_system(scope, s) {
            out.push(s);
        }
    }
    out
}

// --- legendary abilities -----------------------------------------------------------------------

/// `legendary::available`: the seat controls the planet and has not exhausted its ability card.
#[must_use]
pub fn legendary_available(core: &CoreState, seat: Seat, planet: Planet) -> bool {
    board::controls(core, seat, planet)
        && core.players().by_seat(seat).is_some_and(|p| !p.cards.legendary_exhausted.contains_key(&planet))
}

/// `legendary::ionian_available` (Tempesta's Ionian Fuel Refinery).
#[must_use]
pub fn ionian_available(core: &CoreState, seat: Seat) -> bool {
    legendary_available(core, seat, Planet::Tempesta)
}

/// `legendary::salvaged`: the salvage yard while the seat controls Garbozia, else nothing.
#[must_use]
pub fn salvaged(core: &CoreState, seat: Seat) -> &[ActionCard] {
    if board::controls(core, seat, Planet::Garbozia) { core.cards().salvage_yard.as_slice() } else { &[] }
}
