//! Formerly silent abilities (lane G, `docs/G_SILENT_RESUME.md`): rules the string engine registers (`Ability::new` / `stateful`) that the core table had no
//! twin for, so a native game would have lacked them without any bail. Slots `40_000..` are this module's; the ids are named in `reactions_core::NAMED`.

pub mod gaps;
pub mod ghost;
pub mod gift;
pub mod mitosis;
pub mod muaat;
pub mod pillage;
pub mod returns;
pub mod scavenge;
pub mod winnu;

use ti4_engine_core::kernel::ability::AbilityTable;

/// The rules of this module, registered after the faction packages (only abilities of one window share a bucket).
pub fn register_abilities(t: &mut AbilityTable) {
    gift::register_abilities(t);
    mitosis::register_abilities(t);
    returns::register_abilities(t);
    scavenge::register_abilities(t);
    ghost::register_abilities(t);
    muaat::register_abilities(t);
    winnu::register_abilities(t);
}

/// Pillage, before the movement package: the Nomad's Artuno shares `TRADE_GOODS_GAINED` with it and the Mentak module comes first in the string.
pub fn register_before_movement(t: &mut AbilityTable) {
    pillage::register_abilities(t);
}
