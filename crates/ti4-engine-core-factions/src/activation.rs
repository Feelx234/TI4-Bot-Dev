//! The abilities of `SYSTEM_ACTIVATED / after` that the native tactical action offers (lane M6c): the 13 in-scope abilities the first
//! port of the island lacked. The string registers every module's `timing_abilities` per seat, in the order of `factions::MODULES`;
//! within the one bucket `SYSTEM_ACTIVATED / after` that order is the order of the window's options, so the faction files register their
//! activation abilities here, in the module order (arborec, cabal, empyrean, ghost, mentak, naalu, saar, titans, yssaril), after
//! every other ability of the table (the only ones already in this bucket are I48S and the reaction slot, which the string arms first).

use ti4_engine_core::kernel::ability::AbilityTable;

/// Register the activation abilities in the string's module order.
pub fn register(t: &mut AbilityTable) {
    crate::ce_movement::empyrean::timing::register_activation(t);
    crate::cc_ground::mentak::timing::register_activation(t);
}
