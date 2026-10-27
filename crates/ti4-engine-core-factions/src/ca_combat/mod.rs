//! Package CA, the combat line (docs/ENGINE_CORE_FACTIONS_PLAN.md). Owns every file under this directory;
//! nobody else edits it. One sub-directory per faction, one file per tier.

use crate::{Id, Modules};

pub mod bastion;
pub mod crimson;
pub mod nekro;
pub mod ralnel;

/// The modules this package provides, with all their tiers.
pub const MODULES: Modules = &[
    (Id::Bastion, bastion::register),
    (Id::Crimson, crimson::register),
    (Id::Nekro, nekro::register),
    (Id::Ralnel, ralnel::register),
];

/// The movement slots only (package A2's hooks), for `movement_table`.
pub const MOVEMENT: Modules = &[
    (Id::Crimson, crimson::register_movement),
    (Id::Ralnel, ralnel::register_movement),
];
