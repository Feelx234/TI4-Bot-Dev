//! keleres (package CB, economy line): one file per tier, so a window owner and the package never edit one file.
//!
//! * `query.rs` tier Q, `observe.rs` tier O, `offer.rs` tier W (see `docs/ENGINE_CORE_FACTIONS_PLAN.md`).

use ti4_engine_core::hooks::HookTable;

pub mod observe;
pub mod offer;
pub mod query;

/// Keleres is three string modules (`MODULE_M`, `MODULE_X`, `MODULE_A`) in the `MODULES` order, but only
/// `MODULE_M` carries the hooks (`HOOKS`); X and A "claim only" (`hooks: Hooks::NONE`). Registering the
/// hooks in all three would triple every summed slot, so X and A register nothing: their string order position
/// is kept by the `Id`s, which is what the dispatch order needs.
pub fn register_m(t: &mut HookTable) {
    query::register(t);
    observe::register(t);
    offer::register(t);
}

/// `MODULE_X`: claims only (no hooks).
pub fn register_x(_: &mut HookTable) {}

/// `MODULE_A`: claims only (no hooks).
pub fn register_a(_: &mut HookTable) {}
