//! Package CE, the movement-and-leaders line (docs/ENGINE_CORE_FACTIONS_PLAN.md). Owns every file under this directory;
//! nobody else edits it. One sub-directory per faction, one file per tier.

use crate::{Id, Modules};

pub mod arborec;
pub mod empyrean;
pub mod muaat;
pub mod nomad;
pub mod obsidian;
pub mod saar;

/// The modules this package provides, with all their tiers.
pub const MODULES: Modules = &[
    (Id::Arborec, arborec::register),
    (Id::Empyrean, empyrean::register),
    (Id::Muaat, muaat::register),
    (Id::Nomad, nomad::register),
    (Id::Obsidian, obsidian::register),
    (Id::Saar, saar::register),
];

/// The movement slots only (package A2's hooks), for `movement_table`.
pub const MOVEMENT: Modules = &[
    (Id::Empyrean, empyrean::register_movement),
    (Id::Muaat, muaat::register_movement),
    (Id::Nomad, nomad::register_movement),
    (Id::Saar, saar::register_movement),
];

pub(crate) mod readers;

#[cfg(test)]
pub(crate) use crate::common::testkit;

/// The timing abilities of the package (the reaction windows), in the string registration order of the factions.
pub fn register_abilities(t: &mut ti4_engine_core::kernel::ability::AbilityTable) {
    arborec::timing::register_abilities(t);
    empyrean::timing::register_abilities(t);
    empyrean::timing::register_agent(t);
}

/// The abilities of the factions that come after Mentak and Nekro in the string order of the modules (Nomad).
pub fn register_late_abilities(t: &mut ti4_engine_core::kernel::ability::AbilityTable) {
    muaat::timing::register_abilities(t);
    nomad::timing::register_abilities(t);
    empyrean::timing::register_late(t);
}
