//! C1 movement and board shard (needs A2 + A5 + B7).
//!
//! Factions (docs/ENGINE_CORE_FACTIONS_PLAN.md): cabal empyrean ghost muaat.
//! This shard owns every file in this directory. Add `<faction>.rs` here, `mod` it below and list
//! it in [`MODULES`] with its [`crate::Id`]; nothing outside the directory changes.

use crate::Modules;

mod muaat;

/// The modules this shard provides. Muaat is the compile-checked example of the pattern.
pub const MODULES: Modules = &[(crate::Id::Muaat, muaat::register)];
