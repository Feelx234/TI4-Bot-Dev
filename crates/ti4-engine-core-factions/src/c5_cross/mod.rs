//! C5 cross-area shard (needs B1 + B2 + B3 + B6 + B8 direct hooks).
//!
//! Factions (docs/ENGINE_CORE_FACTIONS_PLAN.md): bastion crimson deepwrought nekro.
//! This shard owns every file in this directory. Add `<faction>.rs` here, `mod` it below and list
//! it in [`MODULES`] with its [`crate::Id`]; nothing outside the directory changes.

use crate::Modules;

/// The modules this shard provides (none yet).
pub const MODULES: Modules = &[];
