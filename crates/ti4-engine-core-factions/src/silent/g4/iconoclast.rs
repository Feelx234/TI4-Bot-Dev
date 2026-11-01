//! naalu: Iconoclast, the Thunder's Edge printing of the Naalu mech (`factions::naalu::mech`, unit `naalu_mech_te`): DEPLOY "after another player gains a relic: place this mech on a planet you control".
//! The string announces `RELIC_GAINED` at the start of the step after any relic is gained (the turn flow's reconcile does the same here). Optional; registered for every seat
//! (`unit:*:naalu_mech_te:RELIC_GAINED:after`), the condition names a Naalu seat, another player as the gainer and a planet to put the mech on. Not asked for one planet; nothing placed with no mech in the box.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, UnitBaseType};
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::state::{EventKind, Key};

use super::code;
use super::place::{choose, spots, Placement, Target};

/// Iconoclast (`unit:*:naalu_mech_te:RELIC_GAINED:after`).
pub const ICONOCLAST: Slot = Slot(44_006);

fn condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    c.state.players().by_seat(me).is_some_and(|p| p.faction == Faction::TheNaaluCollective)
        && c.event.payload.seat(Key::Player).is_some_and(|gainer| gainer != me)
        && !spots(c.state, me, Target::ControlledPlanet).is_empty()
}

fn deploy(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    choose(eff, Placement { who: me, base: UnitBaseType::Mech, count: 1, target: Target::ControlledPlanet, optional: false, ability: code::ICONOCLAST });
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(ICONOCLAST, AbilityKind::Unit, EventKind::RelicGained, Relation::After, deploy).optional().when(condition));
}
