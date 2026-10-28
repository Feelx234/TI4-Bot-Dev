//! The Mentak Coalition: Corsair (`mentak::may_move_through_ships`).

use ti4_engine_core::generated::{Unit, UnitBaseType};
use ti4_engine_core::hooks::{HookTable, PassSite};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::CoreState;

use crate::common::scope;

pub fn register(t: &mut HookTable) {
    t.movement.may_move_through_ships.push(may_move_through_ships);
}

/// Corsair: "If the active system contains another player's non-fighter ships, this unit can move
/// through systems that contain other players' ships." The one hook that reads the active system.
fn may_move_through_ships(s: &CoreState, k: &Kernel, site: PassSite) -> bool {
    if site.ship != Unit::Corsair {
        return false;
    }
    let scope = scope(k);
    s.board().sys(site.active).units.as_slice().iter().any(|p| {
        p.owner() != site.player
            && scope.unit(p.kind()).is_some_and(|d| d.ship && d.base_type != UnitBaseType::Fighter)
    })
}
