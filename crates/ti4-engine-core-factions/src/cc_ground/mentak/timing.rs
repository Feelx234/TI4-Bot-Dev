//! mentak: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported: Salvage Operations, the
//! mandatory half ("After you win or lose a space combat, gain 1 trade good"; a draw is neither). Not ported: the
//! produce half, Pillage, Mentak's agent, hero and commander, Table's Grace.
//!
//! The gain is staged (`TRADE_GOODS_GAINED`, source `mentak`), as the string's `gain_trade_goods_staged`: the window's
//! adapter announces it.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::Technology;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::tech;
use ti4_engine_core::state::{EventKind, Key};

/// Salvage Operations (`technology:*:so:SPACE_COMBAT_ENDED:after`).
pub const SALVAGE_TRADE_GOOD: Slot = Slot(8001);

fn condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    let p = &c.event.payload;
    tech::owns(c.state, me, Technology::SalvageOperations)
        && (p.seat(Key::Attacker) == Some(me) || p.seat(Key::Defender) == Some(me))
        && p.seat(Key::Winner).is_some()
}

fn gain(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let source = eff.codes().mentak;
    fx::gain_trade_goods_staged(eff.state(), me, 1, source);
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(SALVAGE_TRADE_GOOD, AbilityKind::Technology, EventKind::SpaceCombatEnded, Relation::After, gain).when(condition));
}
