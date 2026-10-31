//! The reads the unlock checks and the offers of the faction leaders share (package F2: the hook slots `commander_unlocked` and
//! `leader_action`). Each answers a loop the string faction modules write over `GameState::board`.
//!
//! The string board holds an entry for every system anything happened in (`SysFlags::TOUCHED`, see [`has_entry`]); its pieces are the
//! space area (`units`) and the planets (`planet_units`).

use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::state::{CoreState, Piece, SysFlags};

/// Every piece of `who` in a space area, with the system.
pub fn space_pieces(core: &CoreState, who: Seat) -> impl Iterator<Item = (System, Piece)> + '_ {
    core.board().systems().flat_map(move |(system, cell)| cell.units.as_slice().iter().filter(move |p| p.owner() == who).map(move |p| (system, *p)))
}

/// Every piece of `who` on a planet, with the planet.
pub fn planet_pieces(core: &CoreState, who: Seat) -> impl Iterator<Item = (Planet, Piece)> + '_ {
    core.board().planets().flat_map(move |(planet, cell)| cell.units.as_slice().iter().filter(move |p| p.owner() == who).map(move |p| (planet, *p)))
}

/// How many pieces of `who` satisfy `pred`, in the space areas and on the planets.
pub fn count_pieces(core: &CoreState, who: Seat, mut pred: impl FnMut(Piece) -> bool) -> usize {
    space_pieces(core, who).filter(|(_, p)| pred(*p)).count() + planet_pieces(core, who).filter(|(_, p)| pred(*p)).count()
}

/// The string board has an entry for the system (`GameState::board.contains_key`).
#[must_use]
pub fn has_entry(core: &CoreState, system: System) -> bool {
    core.board().sys(system).flags.has(SysFlags::TOUCHED)
}
