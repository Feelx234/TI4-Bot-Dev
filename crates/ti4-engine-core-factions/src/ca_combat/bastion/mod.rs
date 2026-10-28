//! bastion (package CA, combat line): one file per tier, so a window owner and the package never edit one file.
//!
//! * `query.rs` tier Q, `observe.rs` tier O, `offer.rs` tier W (see `docs/ENGINE_CORE_FACTIONS_PLAN.md`).

use ti4_engine_core::hooks::HookTable;

mod observe;
mod offer;
mod query;

/// Every tier, in the order movement, query, observe, offer.
pub fn register(t: &mut HookTable) {
    query::register(t);
    observe::register(t);
    offer::register(t);
}

/// Only the observers (ship-lost note and take, retreat clearing): for a window that still takes the
/// unit-level queries from the string engine (package B1's bridge).
pub fn register_observers(t: &mut HookTable) {
    observe::register(t);
}
