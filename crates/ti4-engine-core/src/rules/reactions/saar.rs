//! Self-Assembly Routines (`sar`, `technology::timing_abilities`), second ability: "After 1 of your mechs is
//! destroyed, gain 1 trade good." Each destroyed mech is its own event; a sustained hit destroys nothing. Rules
//! version 13 (`RULES_SELF_ASSEMBLY`) and later; any seat that holds the card reacts, whatever its faction. The
//! gain is announced as `TRADE_GOODS_GAINED` (`source`: `self_assembly_routines`) through the resolver.
//!
//! The first ability of the card (the mech after PRODUCTION) is an offer of the production window, not a reaction.

use crate::choice::reaction::{AbilityKind, Relation};
use crate::generated::{Technology, UnitBaseType};
use crate::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use crate::kernel::fx;
use crate::kernel::resolver::TimingError;
use crate::query::tech;
use crate::state::{EventKind, Key};
use crate::tables::Scope;

/// Self-Assembly Routines (`tech:*:sar:GROUND_FORCE_DESTROYED:after`).
pub const SAAR_TRADE_GOOD: Slot = Slot(10);

/// `ti4_model::state::RULES_SELF_ASSEMBLY`.
const RULES_SELF_ASSEMBLY: u32 = 13;

fn condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    let p = &c.event.payload;
    c.state.meta().rules_at_least(RULES_SELF_ASSEMBLY)
        && p.seat(Key::Player) == Some(me)
        && tech::owns(c.state, me, Technology::SelfAssemblyRoutines)
        && p.unit(Key::Unit).is_some_and(|u| Scope(u32::MAX).unit(u).is_some_and(|d| d.base_type == UnitBaseType::Mech))
}

fn gain(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let source = eff.codes().self_assembly;
    let (_, announce) = fx::gain_trade_goods_announced(eff.state(), me, 1, source);
    if let Some(rec) = announce {
        eff.emit(rec)?;
    }
    Ok(())
}

pub fn register(t: &mut AbilityTable) {
    t.add(Spec::new(SAAR_TRADE_GOOD, AbilityKind::Technology, EventKind::GroundForceDestroyed, Relation::After, gain).when(condition));
}
