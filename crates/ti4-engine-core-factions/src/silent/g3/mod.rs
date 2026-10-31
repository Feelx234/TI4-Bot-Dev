//! Lane G3 (`docs/G3_RESUME.md`): the loud gap rows of `gaps.rs` turned into native rules, part 2 (Sardakk, Nekro, Yin, Naalu, Saar, the promissory notes and
//! technologies). Slots `42_000..` are this module's; the ids are named in `reactions_core::NAMED`. One file per ability family; each registers where the
//! string registers it (`register_abilities`, after the packages, unless a file says otherwise).

pub mod alastor;
pub mod draw;
pub mod exo2;
pub mod mentak_commander;
pub mod nekro_commander;
pub mod ragh;
pub mod suffi_an;
pub mod tekklar;
pub mod tro_marks;
pub mod zeu;
pub mod zeta;

use ti4_engine_core::kernel::ability::AbilityTable;

/// The texts the rules of this module name in the context of their questions (`SilentCodes::g3`, interned by the adapter in this order): index with the
/// constants of `code`. Unused places hold a placeholder.
pub const CODE_TEXTS: [&str; 16] = [
    "nekro_flagship",
    "technology:exo2",
    "g3_unused",
    "g3_unused",
    "g3_unused",
    "g3_unused",
    "g3_unused",
    "g3_unused",
    "g3_unused",
    "g3_unused",
    "g3_unused",
    "g3_unused",
    "g3_unused",
    "g3_unused",
    "g3_unused",
    "g3_unused",
];

/// The places of `CODE_TEXTS`.
pub mod code {
    /// The unit alias `nekro_flagship`: the source of the Alastor's question.
    pub const NEKRO_FLAGSHIP: usize = 0;
    /// The cause `technology:exo2` of the ships Exotrireme II destroys.
    pub const EXO2_CAUSE: usize = 1;
}

/// The rules of this module, registered after the faction packages.
pub fn register_abilities(t: &mut AbilityTable) {
    zeta::register_abilities(t);
    ragh::register_abilities(t);
    tekklar::register_cleanups(t);
    tro_marks::register_abilities(t);
    zeu::register_abilities(t);
    nekro_commander::register_abilities(t);
    suffi_an::register_abilities(t);
    mentak_commander::register_abilities(t);
}

/// Rules registered between the Nekro and the Winnu modules of the string (`ca_combat` and `cb_economy`'s late rules in the core's order): Sardakk's.
pub fn register_after_ca_combat(t: &mut AbilityTable) {
    tekklar::register_play(t);
    alastor::register_abilities(t);
    exo2::register_abilities(t);
}
