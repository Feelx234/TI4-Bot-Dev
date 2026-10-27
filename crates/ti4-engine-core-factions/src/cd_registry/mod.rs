//! Package CD, the registry line (docs/ENGINE_CORE_FACTIONS_PLAN.md). Owns every file under this directory;
//! nobody else edits it. One sub-directory per faction, one file per tier.

use crate::{Id, Modules};

pub mod naalu;
pub mod naaz;
pub mod sardakk;
pub mod titans;
pub mod yin;
pub mod yssaril;

/// The modules this package provides, with all their tiers.
pub const MODULES: Modules = &[
    (Id::Naalu, naalu::register),
    (Id::Naaz, naaz::register),
    (Id::Sardakk, sardakk::register),
    (Id::Titans, titans::register),
    (Id::Yin, yin::register),
    (Id::Yssaril, yssaril::register),
];

/// The movement slots only (package A2's hooks), for `movement_table`.
pub const MOVEMENT: Modules = &[
    (Id::Yssaril, yssaril::register_movement),
];
