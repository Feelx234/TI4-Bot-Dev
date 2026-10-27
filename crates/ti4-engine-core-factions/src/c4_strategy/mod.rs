//! C4 strategy, agenda and trade shard (needs B8 + B9 + B4).
//!
//! Factions (docs/ENGINE_CORE_FACTIONS_PLAN.md): mahact naalu obsidian winnu yin.
//! This shard owns every file in this directory. Add `<faction>.rs` here, `mod` it below and list
//! it in [`MODULES`] with its [`crate::Id`]; nothing outside the directory changes.

use crate::Modules;

/// The modules this shard provides (none yet).
pub const MODULES: Modules = &[];
