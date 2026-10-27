//! C2 production, economy and card-draw shard (needs A5 + B3 + B6).
//!
//! Factions (docs/ENGINE_CORE_FACTIONS_PLAN.md): arborec argent saar titans yssaril.
//! This shard owns every file in this directory. Add `<faction>.rs` here, `mod` it below and list
//! it in [`MODULES`] with its [`crate::Id`]; nothing outside the directory changes.

use crate::Modules;

/// The modules this shard provides (none yet).
pub const MODULES: Modules = &[];
