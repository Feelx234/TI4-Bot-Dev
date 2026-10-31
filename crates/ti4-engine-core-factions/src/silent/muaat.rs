//! muaat: Magmus and the unlock of Magmus (`factions::muaat::{magmus, commander_unlock}`). The Nova Seed of the hero is lane M6d's.
//!
//! * Magmus (STRATEGY_TOKEN_SPENT, optional, registered for every seat): "After you spend a token from your strategy pool: you may gain 1 trade good."
//! * The unlock (UNITS_PRODUCED, only the Muaat seat): the commander is unlocked by a use of PRODUCTION that made a war sun.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, Leader, UnitBaseType};
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::state::{EventKind, Key, LeaderStatus, List};

/// `leader:*:muaatcommander:STRATEGY_TOKEN_SPENT:after`.
pub const MAGMUS: Slot = Slot(42_020);
/// `leader:*:muaatcommander_unlock:UNITS_PRODUCED:after`.
pub const UNLOCK: Slot = Slot(42_021);

fn magmus_when(c: &CondCx<'_>) -> bool {
    c.event.payload.seat(Key::Player) == Some(c.def.owner) && ti4_engine_core::query::leaders::has_commander_ability_in(c.state, c.def.owner, Leader::Magmus)
}

fn magmus(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    // (The condition holds who spent the token and that the commander is in force.)
    let me = eff.owner();
    let source = eff.codes().g2.muaat;
    fx::gain_trade_goods_staged(eff.state(), me, 1, source);
    Ok(())
}

/// `produced_a_war_sun`: a war sun is among the units the use made.
fn produced_a_war_sun(c: &CondCx<'_>, who: ti4_engine_core::ids::Seat) -> bool {
    c.event.payload.seat(Key::Player) == Some(who)
        && matches!(c.event.payload.list(Key::Other(c.codes.production.units)), Some(List::Produced(units)) if units.as_slice().iter().any(|(unit, _)| unit.data().base_type == UnitBaseType::Warsun))
}

fn unlock_when(c: &CondCx<'_>) -> bool {
    ti4_engine_core::query::leaders::status(c.state, c.def.owner, Leader::Magmus) == Some(LeaderStatus::Locked) && produced_a_war_sun(c, c.def.owner)
}

fn unlock(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    // (The condition holds that the commander is locked and that this seat's use made a war sun.)
    let me = eff.owner();
    ti4_engine_core::rules::leaders::unlock(eff.state(), me, &[Leader::Magmus]);
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(UNLOCK, AbilityKind::Leader, EventKind::UnitsProduced, Relation::After, unlock).applies(Applies::Faction(Faction::TheEmbersOfMuaat)).when(unlock_when));
    t.add(Spec::new(MAGMUS, AbilityKind::Leader, EventKind::StrategyTokenSpent, Relation::After, magmus).optional().when(magmus_when));
}
