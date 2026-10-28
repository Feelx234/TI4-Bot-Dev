//! The sides of the space combat in a system (`combat::combatants`), shared by the package's tier Q hooks.

use ti4_engine_core::containers::SeatSet;
use ti4_engine_core::generated::System;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::board;
use ti4_engine_core::state::CoreState;

use crate::common::scope;

/// Every owner that has a ship in the space area of `system` (a seat that is not a player is the neutral owner), and
/// every seated player whose Alastor sends ground forces into the fight (`combat::ships_of` counts those as ships).
#[must_use]
pub fn combatants(s: &CoreState, k: &Kernel, system: System) -> SeatSet {
    let scope = scope(k);
    let mut sides = SeatSet::EMPTY;
    for u in board::space_units(s, system) {
        if scope.unit(u.kind()).is_some_and(|d| d.ship) {
            sides.insert(u.owner());
        }
    }
    for p in s.players().iter() {
        if !k.hooks.direct.nekro_units_alastor_participants(s, p.seat, system).is_empty() {
            sides.insert(p.seat);
        }
    }
    sides
}
