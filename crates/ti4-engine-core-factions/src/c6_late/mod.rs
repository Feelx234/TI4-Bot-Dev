//! C6 late shard (needs A4 + B5 + B6 + B8 + D1 pieces).
//!
//! Factions (docs/ENGINE_CORE_FACTIONS_PLAN.md): firmament keleres borrowed.
//! This shard owns every file in this directory. Add `<faction>.rs` here, `mod` it below and list
//! it in [`MODULES`] with its [`crate::Id`]; nothing outside the directory changes.

use crate::Modules;

/// The modules this shard provides (none yet).
pub const MODULES: Modules = &[];
