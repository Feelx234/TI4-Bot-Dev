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
    let player = m.seat;
    let system = sx.state.meta().active_system.unwrap_or(System::ALL[0]);
    let ships_moved = fx_move::ships_moved(sx.state);
    let payload = Payload::new()
        .with(Key::Player, Val::Seat(player))
        .with(Key::System, Val::System(system))
        .with(Key::ShipsMoved, Val::Int(i64::from(ships_moved)));
    let _ = sx.emit(fx::event(EventKind::MovementFinished, payload));
    m.plain.push(super::Plain::Typed(EventKind::MovementFinished));
    fx_move::clear_arrivals(sx.state);
    Closed { player, system, skip_to_commit: false, ships_moved }
}
