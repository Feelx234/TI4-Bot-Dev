//! Layer L6 (docs/ENGINE_CORE_STATE.md 4.1): the faction modules, as functions that fill the kernel's
//! [`HookTable`]. Nothing here imports a rule area; a module reads the typed state (`&CoreState`) and the
//! kernel's scope, and returns the neutral value unless its own condition holds.
//!
//! Package A2 (movement) registers the movement slots of 14 factions; the other slots follow with the
//! C shards. Each file mirrors the string module of the same name (`ti4-engine/src/factions/<name>.rs`).
//! The dual run compares the composed answer of this table with the string hooks on every golden
//! position (`ti4-engine-core-bridge`, `core_equivalence`) and the engine's faction tests do the same on
//! constructed states.

pub mod common;

mod argent;
mod cabal;
mod crimson;
mod deepwrought;
mod empyrean;
mod firmament;
mod ghost;
mod mentak;
mod muaat;
mod nomad;
mod ralnel;
mod saar;
mod winnu;
mod yssaril;

use ti4_engine_core::hooks::HookTable;

/// The registering function of each faction module, in the string engine's `MODULES` order.
pub const MOVEMENT_MODULES: [fn(&mut HookTable); 14] = [
    argent::register,
    cabal::register,
    crimson::register,
    deepwrought::register,
    empyrean::register,
    firmament::register,
    ghost::register,
    mentak::register,
    muaat::register,
    nomad::register,
    ralnel::register,
    saar::register,
    winnu::register,
    yssaril::register,
];

/// A table with the movement slots of every faction filled.
#[must_use]
pub fn movement_table() -> HookTable {
    HookTable::of(&MOVEMENT_MODULES)
}
