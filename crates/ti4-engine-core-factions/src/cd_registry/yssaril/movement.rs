//! The Yssaril Tribes: the flagship moves through other players' ships.

use ti4_engine_core::generated::Unit;
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::CoreState;

use crate::common::flagship_has_text;

pub fn register(t: &mut HookTable) {
    t.movement.may_move_through_ships_anywhere.push(may_move_through_ships);
}

fn may_move_through_ships(s: &CoreState, _: &Kernel, who: Seat, ship: Unit) -> bool {
    flagship_has_text(s, who, ship, Unit::YsiaYssrila)
}
