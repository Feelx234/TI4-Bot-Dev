//! Lane G3 (`docs/G3_RESUME.md`): the loud gap rows of `gaps.rs` turned into native rules, part 2 (Sardakk, Nekro, Yin, Naalu, Saar, the promissory notes and
//! technologies). Slots `42_000..` are this module's; the ids are named in `reactions_core::NAMED`. One file per ability family; each registers where the
//! string registers it (`register_abilities`, after the packages, unless a file says otherwise).

pub mod ragh;
pub mod tekklar;
pub mod tro_marks;
pub mod zeta;

use ti4_engine_core::kernel::ability::AbilityTable;

/// The rules of this module, registered after the faction packages.
pub fn register_abilities(t: &mut AbilityTable) {
    zeta::register_abilities(t);
    ragh::register_abilities(t);
    tekklar::register_cleanups(t);
    tro_marks::register_abilities(t);
}

/// Rules registered between the Nekro and the Winnu modules of the string (`ca_combat` and `cb_economy`'s late rules in the core's order): Sardakk's.
pub fn register_after_ca_combat(t: &mut AbilityTable) {
    tekklar::register_play(t);
}
