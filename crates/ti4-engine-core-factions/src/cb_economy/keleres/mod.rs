//! keleres (package CB, economy line): one file per tier, so a window owner and the package never edit one file.
//!
//! * `query.rs` tier Q, `observe.rs` tier O, `offer.rs` tier W (see `docs/ENGINE_CORE_FACTIONS_PLAN.md`).

use ti4_engine_core::hooks::HookTable;

mod observe;
mod offer;
mod query;

/// Keleres is three string modules (`MODULE_M`, `MODULE_X`, `MODULE_A`); each registers its own part.
pub fn register_m(t: &mut HookTable) {
    query::register(t);
    observe::register(t);
    offer::register(t);
}

pub fn register_x(t: &mut HookTable) {
    query::register(t);
    observe::register(t);
    offer::register(t);
}

pub fn register_a(t: &mut HookTable) {
    query::register(t);
    observe::register(t);
    offer::register(t);
}

