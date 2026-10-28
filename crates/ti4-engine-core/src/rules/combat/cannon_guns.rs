//! Who fires in the space cannon offense of a system (the first half of the string `space_cannon_guns`):
//! the units standing in the system itself, space area first, then planet by planet. The guns that reach in
//! from adjacent systems (PDS II, the Indomitus mech: the string galaxy), the Ral Nel Linkship borrowing and
//! the attachments' cannons are appended by the caller and not ported here.

use super::{planets_of_sorted, Rd};
use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::seam::CannonSite;
use crate::state::{Piece, RoundKind};

/// `combat::opponent_with_ships`: the owner of the first other player's ship in the space area.
#[must_use]
pub fn opponent_with_ships(rd: Rd<'_>, active: Seat, system: System) -> Option<Seat> {
    rd.s.board().sys(system).units.iter().find(|u| u.owner() != active && rd.is_ship(u.kind())).map(|p| p.owner())
}

/// The guns standing in `system` that may fire when `active` activates it, in firing order.
#[must_use]
pub fn local_guns(rd: Rd<'_>, system: System, active: Seat) -> Vec<Piece> {
    let (s, k) = (rd.s, rd.k);
    let activation = s.meta().activation_seq;
    let solar_flare = s.players().by_seat(active).is_some_and(|p| p.round_marks.contains(RoundKind::SolarFlare, activation))
        || k.hooks.direct.firmament_space_cannon_silenced(s, k, active, system);
    let opponent = opponent_with_ships(rd, active, system);
    let barred = |owner: Seat| {
        let target = if owner == active { opponent } else { Some(active) };
        target.is_some_and(|target| k.hooks.seam.cannon_barred.decide(rd.q(), &CannonSite { shooter: owner, target, system }, false))
    };
    let may_fire = |owner: Seat| if owner == active { opponent.is_some() && !barred(owner) } else { !solar_flare && !barred(owner) };
    let may_fire_here = |owner: Seat| may_fire(owner) && !k.hooks.direct.crimson_abilities_lost(s, owner, system);
    let mut guns: Vec<Piece> = s
        .board()
        .sys(system)
        .units
        .iter()
        .filter(|p| may_fire_here(p.owner()) && !k.hooks.direct.ralnel_silenced_in_space(s, k, **p))
        .copied()
        .collect();
    for planet in planets_of_sorted(s, system).iter() {
        guns.extend(s.board().planet(*planet).units.iter().filter(|p| may_fire_here(p.owner())).copied());
    }
    guns
}
