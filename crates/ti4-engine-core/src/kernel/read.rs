//! `Rd`: the read bundle of the combat and invasion rules (packages B1 and B2 each had a copy; an L4 area
//! does not import another, so the one shared definition sits here in L3).

use crate::containers::SmallList;
use crate::generated::{Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::seam::SeatSystemSite;
use crate::state::Piece;
use crate::kernel::seam::Q;
use crate::kernel::Kernel;
use crate::state::CoreState;
use crate::tables::Scope;

/// What a combat or invasion rule reads: the state and the kernel (content scope, hook table, seams).
#[derive(Clone, Copy)]
pub struct Rd<'a> {
    pub s: &'a CoreState,
    pub k: &'a Kernel,
    /// Law's Order: every law is blank this turn (`laws::blanked`; the caller computes it).
    pub blanked: bool,
    /// Invasion only: the leaders package's answer to `has_commander_ability(l1z1xcommander)` for the invader
    /// (it needs the leaders `Env`: Imperia, Alliance notes); `None`: read the leader slice and the grant mark only.
    pub l1z1x_commander: Option<bool>,
}

impl<'a> Rd<'a> {
    #[must_use]
    pub fn new(s: &'a CoreState, k: &'a Kernel) -> Self {
        Self { s, k, blanked: false, l1z1x_commander: None }
    }

    /// The same read with the invader's L1Z1X commander ability decided by the leaders package.
    #[must_use]
    pub fn with_l1z1x_commander(mut self, held: bool) -> Self {
        self.l1z1x_commander = Some(held);
        self
    }

    /// The seam reader over the same state and kernel.
    #[must_use]
    pub fn q(&self) -> Q<'a> {
        Q::new(self.s, self.k)
    }

    /// The same read with laws blank or not.
    #[must_use]
    pub fn with_blanked(mut self, blanked: bool) -> Self {
        self.blanked = blanked;
        self
    }

    /// The same read over another state (a played-out copy).
    #[must_use]
    pub fn over(self, s: &'a CoreState) -> Self {
        Self { s, ..self }
    }

    /// The source scope as the tables read it.
    #[must_use]
    pub fn scope(&self) -> Scope {
        Scope(u32::from(self.k.sources.0))
    }

    /// The unit's table row when the unit is in the game's scope (the string `catalogue(content,
    /// sources).get(id)`), else `None`.
    #[must_use]
    pub fn data(&self, unit: Unit) -> Option<&'static crate::generated::UnitData> {
        self.scope().unit(unit)
    }

    /// `UnitType::is_ship` (false for a unit out of scope).
    #[must_use]
    pub fn is_ship(&self, unit: Unit) -> bool {
        self.data(unit).is_some_and(|d| d.ship)
    }

    /// `UnitType::is_fighter`.
    #[must_use]
    pub fn is_fighter(&self, unit: Unit) -> bool {
        self.data(unit).is_some_and(|d| d.base_type == UnitBaseType::Fighter)
    }
}

// -- who fights a space combat from where (shared by the combat rules and the cards that name its combatants) --

/// This player's ships in the system, in board order, then the planet-standing combatants.
#[must_use]
pub fn ships_of(rd: Rd<'_>, player: Seat, system: System) -> Vec<Piece> {
    let mut ships: Vec<Piece> = rd
        .s
        .board()
        .sys(system)
        .units
        .iter()
        .filter(|p| p.owner() == player && rd.is_ship(p.kind()))
        .copied()
        .collect();
    ships.extend(planet_combatants(rd, player, system).into_iter().map(|(_, _, p)| p));
    ships
}

/// The planets whose unit lists live in `system`, in planet (text) order.
#[must_use]
pub fn planets_of_sorted(s: &CoreState, system: System) -> SmallList<Planet, 8> {
    let mut v: SmallList<Planet, 8> = crate::query::board::planets_of(s, system).collect();
    v.as_mut_slice().sort_unstable();
    v
}

/// Units standing on a planet that fight this system's space combat as ships: a Naaz Eidolon Maximum
/// (while its owner has a ship in space) and the ground forces a Nekro Alastor chose.
#[must_use]
pub fn planet_combatants(rd: Rd<'_>, player: Seat, system: System) -> Vec<(Planet, u16, Piece)> {
    let maximum = planetary_maximum_participates(rd, player, system);
    let chosen = rd.k.hooks.seam.alastor_participants.collect(rd.q(), &SeatSystemSite { who: player, system });
    let mut found = Vec::new();
    for planet in planets_of_sorted(rd.s, system).iter() {
        for (index, unit) in rd.s.board().planet(*planet).units.iter().enumerate() {
            if unit.owner() != player {
                continue;
            }
            let voltron = maximum && unit.kind() == Unit::EidolonMaximum;
            if voltron || chosen.iter().any(|(p, i)| p == planet && usize::from(*i) == index) {
                found.push((*planet, index as u16, *unit));
            }
        }
    }
    found
}

/// The corpus clarification for Eidolon Maximum: a planet-standing unit joins its owner's fleet in space
/// combat; cargo alone is not a fleet.
#[must_use]
pub fn planetary_maximum_participates(rd: Rd<'_>, player: Seat, system: System) -> bool {
    rd.s.board().sys(system).units.iter().any(|p| p.owner() == player && rd.is_ship(p.kind()))
}
