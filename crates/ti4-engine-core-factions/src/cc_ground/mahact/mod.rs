//! mahact (package CC, ground-and-ship line): one file per tier, so a window owner and the package never edit one file.
//!
//! * `query.rs` tier Q, `observe.rs` tier O, `offer.rs` tier W (see `docs/ENGINE_CORE_FACTIONS_PLAN.md`).

use ti4_engine_core::hooks::HookTable;

mod observe;
mod offer;
pub mod query;
pub mod timing;

/// Every tier, in the order movement, query, observe, offer.
pub fn register(t: &mut HookTable) {
    query::register(t);
    observe::register(t);
    offer::register(t);
}
