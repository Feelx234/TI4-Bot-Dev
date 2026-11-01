//! titans: Tungstantus, the commander of the Titans of Ul (`factions::borrowed_commanders::timing_abilities`): "After you use PRODUCTION in a tactical action: gain 1 trade
//! good" (the string registers it `PRODUCTION_USED:when` for every seat; an Alliance or a grant carries the commander). Optional; the source of the good is the card.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::Leader;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::leaders;
use ti4_engine_core::state::{EventKind, Key};

use super::code;

/// `leader:*:titanscommander:PRODUCTION_USED:when`.
pub const TUNGSTANTUS: Slot = Slot(44_001);

fn when(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    c.event.payload.seat(Key::Player) == Some(me) && leaders::has_commander_ability_in(c.state, me, Leader::Tungstantus)
}

fn gain(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let source = eff.codes().silent.g4[code::TITANS_COMMANDER];
    let (_, announce) = fx::gain_trade_goods_announced(eff.state(), me, 1, source);
    if let Some(rec) = announce {
        eff.emit(rec)?;
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(TUNGSTANTUS, AbilityKind::Leader, EventKind::ProductionUsed, Relation::When, gain).optional().when(when));
}
