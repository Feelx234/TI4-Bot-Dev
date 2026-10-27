//! C3 combat and ground shard (needs B1 + B2).
//!
//! Factions (docs/ENGINE_CORE_FACTIONS_PLAN.md): mentak nomad naaz ralnel sardakk.
//! This shard owns every file in this directory. Add `<faction>.rs` here, `mod` it below and list
//! it in [`MODULES`] with its [`crate::Id`]; nothing outside the directory changes.

use crate::Modules;

/// The modules this shard provides (none yet).
pub const MODULES: Modules = &[];
