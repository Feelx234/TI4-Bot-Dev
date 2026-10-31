//! The close of the movement step (`Game::finish_tactical_inner`, up to the opening of the aftermath).

use crate::generated::System;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::kernel::fx_move;
use crate::kernel::stage::Sx;
use crate::state::{EventKind, Key, Payload, Val};

use super::Machine;

/// What the close hands to the aftermath (D1b): whose action it was, in which system, and whether the Sardakk hero skips
/// to the commit step (never in a position the machine follows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Closed {
    pub player: Seat,
    pub system: System,
    pub skip_to_commit: bool,
    pub ships_moved: u32,
}

/// `MOVEMENT_FINISHED` with the count of ships that arrived, then the count is dropped.
pub(super) fn close(m: &mut Machine, sx: &mut Sx<'_>) -> Closed {
    let closed = announce(m, sx);
    forget_arrivals(sx);
    closed
}

/// The announcement alone (`Game::finish_head`, up to the Sardakk hero, which reads the count of ships that arrived).
pub(super) fn announce(m: &mut Machine, sx: &mut Sx<'_>) -> Closed {
    let player = m.seat;
    let system = sx.state.meta().active_system.unwrap_or(System::ALL[0]);
    let ships_moved = fx_move::ships_moved(sx.state);
    let payload = Payload::new()
        .with(Key::Player, Val::Seat(player))
        .with(Key::System, Val::System(system))
        .with(Key::ShipsMoved, Val::Int(i64::from(ships_moved)));
    let _ = m.emit(sx, fx::event(EventKind::MovementFinished, payload));
    m.note(sx, super::Plain::Typed(EventKind::MovementFinished));
    Closed { player, system, skip_to_commit: false, ships_moved }
}

/// The count has served the movement step's windows.
pub(super) fn forget_arrivals(sx: &mut Sx<'_>) {
    fx_move::clear_arrivals(sx.state);
}

/// `production::destroy_blockaded_mobile_docks`: Floating Factory I, "if this unit is blockaded, it is destroyed". A seat whose space area
/// holds a PRODUCTION structure that is space-only, an enemy ship and none of its own loses those structures. Returns what was destroyed.
pub fn destroy_blockaded_mobile_docks(core: &mut crate::state::CoreState, system: System) -> Vec<crate::state::Piece> {
    use crate::query::board;
    let units: Vec<crate::state::Piece> = board::space_units(core, system).to_vec();
    let is_dock = |p: &crate::state::Piece| {
        let d = p.kind().data();
        d.structure && d.space_only && d.production.is_some()
    };
    let mut owners: Vec<Seat> = units.iter().filter(|p| is_dock(p)).map(|p| p.owner()).collect();
    owners.sort_unstable();
    owners.dedup();
    let doomed: Vec<crate::state::Piece> = owners
        .into_iter()
        .filter(|owner| {
            let enemy_ship = units.iter().any(|p| p.owner() != *owner && p.kind().data().ship);
            let own_ship = units.iter().any(|p| p.owner() == *owner && p.kind().data().ship);
            enemy_ship && !own_ship
        })
        .flat_map(|owner| units.iter().filter(move |p| p.owner() == owner && is_dock(p)).copied().collect::<Vec<_>>())
        .collect();
    for piece in &doomed {
        fx_move::destroy_ship(core, system, *piece);
    }
    doomed
}
