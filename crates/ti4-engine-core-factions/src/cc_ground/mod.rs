//! Package CC, the ground-and-ship line (docs/ENGINE_CORE_FACTIONS_PLAN.md). Owns every file under this directory;
//! nobody else edits it. One sub-directory per faction, one file per tier.

use crate::{Id, Modules};

pub mod firmament;
pub mod ghost;
pub mod mahact;
pub mod mentak;
pub mod borrowed;
pub mod sides;

/// The modules this package provides, with all their tiers.
pub const MODULES: Modules = &[
    (Id::Firmament, firmament::register),
    (Id::Ghost, ghost::register),
    (Id::Mahact, mahact::register),
    (Id::Mentak, mentak::register),
    (Id::Borrowed, borrowed::register),
];

/// The movement slots only (package A2's hooks), for `movement_table`.
pub const MOVEMENT: Modules = &[
    (Id::Firmament, firmament::register_movement),
    (Id::Ghost, ghost::register_movement),
    (Id::Mentak, mentak::register_movement),
];

#[cfg(test)]
pub(crate) use crate::common::testkit;
