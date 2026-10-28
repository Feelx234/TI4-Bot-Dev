//! The ability rules behind the reaction windows (package `core-reactions`; docs/ENGINE_REACTIONS.md): the
//! `Spec`s of the generic abilities that are registered for every seat (technologies, leaders, the rules of
//! the table), one file each. The faction abilities live in the faction packages.
//!
//! [`register`] adds them to an `AbilityTable` in the per-owner registration order of the string
//! `reactions::arm`: the reaction-card slots, Instinct Training, I48S, Infantry II, Magen Defense, then the
//! factions.

use crate::kernel::ability::{AbilityTable, Slot};

pub mod infantry_ii;
pub mod round_agent;

/// Infantry II (`inf2`): after the owner's Infantry II is destroyed, roll a die.
pub const INFANTRY_II_ROLLS: Slot = Slot(1);

/// The abilities the string registers ahead of the per-seat reaction-card slots (`reactions::arm`): the round
/// agents.
pub fn register_first(t: &mut AbilityTable) {
    round_agent::register(t);
}

/// Add the ported generic abilities that follow the slots, in the string registration order.
pub fn register(t: &mut AbilityTable) {
    infantry_ii::register(t);
}
