//! Lane G4 (`docs/G4_RESUME.md`): the last loud gap rows of `gaps.rs` turned into native rules (Propagation, Titans commander, Spy Net, Stymie, Foresight, the Magen Defense
//! hit, Yin Spinner, the Naalu TE mech). Slots `44_000..` are this module's; the ids are named in `reactions_core::NAMED`. One file per ability; each registers where the string
//! registers it (`register_abilities`, after the rest of `silent`'s).

pub mod magen_hit;
pub mod place;
pub mod foresight;
pub mod iconoclast;
pub mod propagation;
pub mod spynet;
pub mod stymie;
pub mod titans_commander;
pub mod yin_spinner;

use ti4_engine_core::kernel::ability::AbilityTable;

/// The places of the texts the rules of this module name (`SilentCodes::g4`; the adapter interns the texts in this order: `reactions_core::G4_CODE_TEXTS`).
pub mod code {
    /// The source `titanscommander` of the trade good the Titans commander gains.
    pub const TITANS_COMMANDER: usize = 0;
    /// The cause `md` of a ground force Magen Defense Grid's hit damages or destroys.
    pub const MD_CAUSE: usize = 1;
    /// The rule `87` (SUSTAIN DAMAGE) that sources the offer to sustain a hit an effect assigned.
    pub const SUSTAIN_RULE: usize = 2;
    /// The technology `yso` (Yin Spinner) and the unit `naalu_mech_te`: the sources of the placement questions.
    pub const YIN_SPINNER: usize = 3;
    pub const ICONOCLAST: usize = 4;
    /// The reason `naalu` of the strategy token Foresight spends and the pool `strategy` of the token it places.
    pub const NAALU: usize = 5;
    pub const STRATEGY: usize = 6;
}

/// The rules of this module.
pub fn register_abilities(t: &mut AbilityTable) {
    propagation::register_abilities(t);
    titans_commander::register_abilities(t);
    yin_spinner::register_abilities(t);
    iconoclast::register_abilities(t);
    foresight::register_abilities(t);
}

/// Rules registered after the Winnu's: the Yssaril module of the string comes last.
pub fn register_last(t: &mut AbilityTable) {
    spynet::register_abilities(t);
}

/// Rules registered with the Arborec's module of the string, ahead of the rest of `silent`'s (Stymie).
pub fn register_first(t: &mut AbilityTable) {
    stymie::register_abilities(t);
}
