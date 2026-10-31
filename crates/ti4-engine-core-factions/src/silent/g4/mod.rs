//! Lane G4 (`docs/G4_RESUME.md`): the last loud gap rows of `gaps.rs` turned into native rules (Propagation, Titans commander, Spy Net, Stymie, Foresight, the Magen Defense
//! hit, Yin Spinner, the Naalu TE mech). Slots `44_000..` are this module's; the ids are named in `reactions_core::NAMED`. One file per ability; each registers where the string
//! registers it (`register_abilities`, after the rest of `silent`'s).

pub mod propagation;
pub mod spynet;
pub mod titans_commander;

use ti4_engine_core::kernel::ability::AbilityTable;

/// The places of the texts the rules of this module name (`SilentCodes::g4`; the adapter interns the texts in this order: `reactions_core::G4_CODE_TEXTS`).
pub mod code {
    /// The source `titanscommander` of the trade good the Titans commander gains.
    pub const TITANS_COMMANDER: usize = 0;
}

/// The rules of this module.
pub fn register_abilities(t: &mut AbilityTable) {
    propagation::register_abilities(t);
    titans_commander::register_abilities(t);
}

/// Rules registered after the Winnu's: the Yssaril module of the string comes last.
pub fn register_last(t: &mut AbilityTable) {
    spynet::register_abilities(t);
}
