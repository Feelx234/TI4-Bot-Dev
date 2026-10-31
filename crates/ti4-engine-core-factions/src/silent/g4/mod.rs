//! Lane G4 (`docs/G4_RESUME.md`): the last loud gap rows of `gaps.rs` turned into native rules (Propagation, Titans commander, Spy Net, Stymie, Foresight, the Magen Defense
//! hit, Yin Spinner, the Naalu TE mech). Slots `44_000..` are this module's; the ids are named in `reactions_core::NAMED`. One file per ability; each registers where the string
//! registers it (`register_abilities`, after the rest of `silent`'s).

pub mod propagation;

use ti4_engine_core::kernel::ability::AbilityTable;

/// The rules of this module.
pub fn register_abilities(t: &mut AbilityTable) {
    propagation::register_abilities(t);
}
