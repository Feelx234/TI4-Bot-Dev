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

/// The timing abilities of the package (the reaction windows), in the string registration order of the factions.
pub fn register_abilities(t: &mut ti4_engine_core::kernel::ability::AbilityTable) {
    mahact::timing::register_abilities(t);
    mentak::timing::register_abilities(t);
}

/// The abilities the string registers ahead of the per-seat reaction-card slots (`reactions::arm`): the round agents.
pub fn register_first(t: &mut ti4_engine_core::kernel::ability::AbilityTable) {
    crate::cd_registry::yssaril::ssruu::register_first(t);
    borrowed::timing::register_abilities(t);
}
