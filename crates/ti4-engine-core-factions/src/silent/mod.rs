//! Formerly silent abilities (lane G, `docs/G_SILENT_RESUME.md`): rules the string engine registers (`Ability::new` / `stateful`) that the core table had no
//! twin for, so a native game would have lacked them without any bail. Slots `40_000..` are this module's; the ids are named in `reactions_core::NAMED`.

pub mod gaps;
pub mod g3;
pub mod g4;
pub mod cabal;
pub mod empyrean;
pub mod ghost;
pub mod gift;
pub mod mitosis;
pub mod muaat;
pub mod naaz;
pub mod pillage;
pub mod returns;
pub mod scavenge;
pub mod sol;
pub mod winnu;

use ti4_engine_core::kernel::ability::AbilityTable;

/// The rules of this module, registered after the faction packages (only abilities of one window share a bucket).
pub fn register_abilities(t: &mut AbilityTable) {
    g3::register_first(t);
    gift::register_abilities(t);
    mitosis::register_abilities(t);
    returns::register_abilities(t);
    // The faction modules of the string in their order (arborec above, then cabal, empyrean, ghost, muaat, naaz, saar, winnu): only abilities of one
    // window share a bucket, and Naaz's Dart and Tai comes before Saar's Scavenge in PLANET_CONTROL_GAINED.
    cabal::register_abilities(t);
    empyrean::register_abilities(t);
    ghost::register_abilities(t);
    muaat::register_abilities(t);
    naaz::register_abilities(t);
    scavenge::register_abilities(t);
    g3::register_saar(t);
    g3::register_abilities(t);
    g4::register_abilities(t);
    winnu::register_abilities(t);
    sol::register_last(t);
    empyrean::register_last(t);
    g4::register_last(t);
}

/// Pillage, before the movement package: the Nomad's Artuno shares `TRADE_GOODS_GAINED` with it and the Mentak module comes first in the string.
pub fn register_before_movement(t: &mut AbilityTable) {
    pillage::register_abilities(t);
}
