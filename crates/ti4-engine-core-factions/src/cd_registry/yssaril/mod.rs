//! yssaril (package CD, registry line): one file per tier, so a window owner and the package never edit one file.
//!
//! * `movement.rs`: the movement slots (package A2), already ported.
//! * `query.rs` tier Q, `observe.rs` tier O, `offer.rs` tier W (see `docs/ENGINE_CORE_FACTIONS_PLAN.md`).

use ti4_engine_core::hooks::HookTable;

pub(crate) mod movement;
mod observe;
mod offer;
pub mod query;

/// Every tier, in the order movement, query, observe, offer.
pub fn register(t: &mut HookTable) {
    movement::register(t);
    query::register(t);
    observe::register(t);
    offer::register(t);
}

/// Only the movement slots (`movement_table`).
pub fn register_movement(t: &mut HookTable) {
    movement::register(t);
}
