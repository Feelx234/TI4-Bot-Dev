//! Who fires in the space cannon offense of a system (the first half of the string `space_cannon_guns`):
//! the units standing in the system itself, space area first, then planet by planet. The guns that reach in
//! from adjacent systems (PDS II, the Indomitus mech: the string galaxy), the Ral Nel Linkship borrowing and
//! the attachments' cannons are appended by the caller and not ported here.

use super::{planets_of_sorted, Rd};
use crate::galaxy::Adjacency;
use crate::generated::{Planet, System, Unit};
use crate::ids::Seat;
use crate::kernel::seam::CannonSite;
use crate::state::{Piece, RoundKind};

/// `combat::opponent_with_ships`: the owner of the first other player's ship in the space area.
#[must_use]
pub fn opponent_with_ships(rd: Rd<'_>, active: Seat, system: System) -> Option<Seat> {
    rd.s.board().sys(system).units.iter().find(|u| u.owner() != active && rd.is_ship(u.kind())).map(|p| p.owner())
}

/// Who may fire when `active` activates `system`: the per-owner gates of the string `space_cannon_guns`.
struct Gate<'a> {
    rd: Rd<'a>,
    system: System,
    active: Seat,
    opponent: Option<Seat>,
    solar_flare: bool,
}

impl<'a> Gate<'a> {
    fn new(rd: Rd<'a>, system: System, active: Seat) -> Self {
        let (s, k) = (rd.s, rd.k);
        let activation = s.meta().activation_seq;
        let solar_flare = s.players().by_seat(active).is_some_and(|p| p.round_marks.contains(RoundKind::SolarFlare, activation))
            || k.hooks.direct.firmament_space_cannon_silenced(s, k, active, system);
        Self { rd, system, active, opponent: opponent_with_ships(rd, active, system), solar_flare }
    }

    fn barred(&self, owner: Seat) -> bool {
        let target = if owner == self.active { self.opponent } else { Some(self.active) };
        target.is_some_and(|target| {
            self.rd.k.hooks.seam.cannon_barred.decide(self.rd.q(), &CannonSite { shooter: owner, target, system: self.system }, false)
        })
    }

    /// The gate of the string `may_fire`.
    fn may_fire(&self, owner: Seat) -> bool {
        if owner == self.active { self.opponent.is_some() && !self.barred(owner) } else { !self.solar_flare && !self.barred(owner) }
    }

    /// `may_fire` and not Quietus-blanked in the system itself.
    fn may_fire_here(&self, owner: Seat) -> bool {
        self.may_fire(owner) && !self.rd.k.hooks.direct.crimson_abilities_lost(self.rd.s, owner, self.system)
    }
}

/// The guns standing in `system` that may fire when `active` activates it, in firing order.
#[must_use]
pub fn local_guns(rd: Rd<'_>, system: System, active: Seat) -> Vec<Piece> {
    let (s, k) = (rd.s, rd.k);
    let gate = Gate::new(rd, system, active);
    let mut guns: Vec<Piece> = s
        .board()
        .sys(system)
        .units
        .iter()
        .filter(|p| gate.may_fire_here(p.owner()) && !k.hooks.direct.ralnel_silenced_in_space(s, k, **p))
        .copied()
        .collect();
    for planet in planets_of_sorted(s, system).iter() {
        guns.extend(s.board().planet(*planet).units.iter().filter(|p| gate.may_fire_here(p.owner())).copied());
    }
    guns
}

/// Every gun of the string `space_cannon_guns` that is a unit, in firing order: the local ones, the ones in
/// adjacent systems whose card lets them fire into this one (`reaching`: PDS II, the Indomitus mech and
/// flagship, Hel-Titan II; the adapter lists them from the card text), and the Linkship borrowing. The
/// attachments' cannons are not units and stay with the caller.
#[must_use]
pub fn unit_guns(rd: Rd<'_>, system: System, active: Seat, adjacency: Option<&Adjacency>, reaching: &[Unit]) -> Vec<Piece> {
    let (s, k) = (rd.s, rd.k);
    let mut guns = local_guns(rd, system, active);
    let gate = Gate::new(rd, system, active);
    if let Some(adjacency) = adjacency {
        for neighbour in adjacency.of(system).iter() {
            let here = crate::ids::system_at(neighbour);
            let mut standing: Vec<Piece> = Vec::new();
            for planet in planets_of_sorted(s, here).iter() {
                standing.extend(s.board().planet(*planet).units.iter().copied());
            }
            standing.extend(s.board().sys(here).units.iter().filter(|p| !k.hooks.direct.ralnel_silenced_in_space(s, k, **p)).copied());
            guns.extend(standing.into_iter().filter(|p| {
                gate.may_fire(p.owner()) && !k.hooks.direct.crimson_abilities_lost(s, p.owner(), here) && reaching.contains(&p.kind())
            }));
        }
    }
    guns.extend(k.hooks.direct.ralnel_linkship_guns(s, k, system).iter().filter(|p| gate.may_fire_here(p.owner())).copied());
    guns
}

/// The SPACE CANNON an attachment gives its planet "as if it were a unit" (and Custodian's Favour on
/// Mecatol Rex): `(owner, planet, hit number, dice)` for each controlled planet of `system` in planet
/// order, for the owners that may fire.
#[must_use]
pub fn attachment_guns(rd: Rd<'_>, system: System, active: Seat) -> Vec<(Seat, Planet, u32, usize)> {
    let (s, k) = (rd.s, rd.k);
    let gate = Gate::new(rd, system, active);
    let mut out = Vec::new();
    for planet in planets_of_sorted(s, system).iter() {
        let cell = s.board().planet(*planet);
        let Some(owner) = cell.control else { continue };
        let mut found: Vec<(Seat, u32, usize)> = cell
            .attach
            .iter()
            .filter_map(|a| a.data().space_cannon)
            .filter(|r| r.dice > 0)
            .map(|r| (owner, u32::from(r.hits_on), usize::from(r.dice)))
            .collect();
        if let Some((who, count, hit_on)) = k.hooks.direct.keleres_custodian_cannon(s, *planet, owner) {
            found.push((who, u32::from(hit_on), count as usize));
        }
        out.extend(found.into_iter().filter(|(who, ..)| gate.may_fire(*who)).map(|(who, value, count)| (who, *planet, value, count)));
    }
    out
}
