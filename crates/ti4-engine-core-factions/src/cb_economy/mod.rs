//! Package CB, the economy line (docs/ENGINE_CORE_FACTIONS_PLAN.md). Owns every file under this directory;
//! nobody else edits it. One sub-directory per faction, one file per tier.

use crate::{Id, Modules};

pub mod argent;
pub mod cabal;
pub mod deepwrought;
pub mod keleres;
pub mod winnu;

/// The modules this package provides, with all their tiers.
pub const MODULES: Modules = &[
    (Id::Argent, argent::register),
    (Id::Cabal, cabal::register),
    (Id::Deepwrought, deepwrought::register),
    (Id::KeleresMentak, keleres::register_m),
    (Id::KeleresXxcha, keleres::register_x),
    (Id::KeleresArgent, keleres::register_a),
    (Id::Winnu, winnu::register),
];

/// The movement slots only (package A2's hooks), for `movement_table`.
pub const MOVEMENT: Modules = &[
    (Id::Argent, argent::register_movement),
    (Id::Cabal, cabal::register_movement),
    (Id::Deepwrought, deepwrought::register_movement),
    (Id::Winnu, winnu::register_movement),
];
