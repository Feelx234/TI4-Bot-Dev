//! saar: Scavenge (`factions::saar::scavenge`): "After you gain control of a planet: gain 1 trade good." Mandatory; the string registers it for every seat
//! (`ability:*:scavenge:PLANET_CONTROL_GAINED:after`) and the condition names the holder of the printed ability and the gainer.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::Ability;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::state::{EventKind, Key};

/// Scavenge (`ability:*:scavenge:PLANET_CONTROL_GAINED:after`).
pub const SCAVENGE: Slot = Slot(40_006);

fn condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    c.event.payload.seat(Key::Player) == Some(me) && c.state.players().by_seat(me).is_some_and(|p| p.faction.data().abilities.contains(&Ability::Scavenge))
}

fn gain(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let source = eff.codes().silent.scavenge;
    let (_, announce) = fx::gain_trade_goods_announced(eff.state(), me, 1, source);
    if let Some(rec) = announce {
        eff.emit(rec)?;
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(SCAVENGE, AbilityKind::Ability, EventKind::PlanetControlGained, Relation::After, gain).when(condition));
}
