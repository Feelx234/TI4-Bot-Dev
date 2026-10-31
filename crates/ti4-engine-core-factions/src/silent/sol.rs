//! sol: Claire Gibson, the commander of the Federation of Sol (`factions::borrowed_commanders::claire_gibson`). "At the start of a ground combat on a planet you control: you
//! may place 1 infantry from your reinforcements on that planet." Registered for every seat (an Alliance or a grant carries the commander), offered only while an
//! infantry remains in the box.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Leader, Unit, UnitBaseType};
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::{board, fleet, leaders};
use ti4_engine_core::state::{EventKind, Key};

use crate::cd_registry::yin::timing::placed_form_overridden;
use crate::common::scope;

/// `leader:*:solcommander:GROUND_COMBAT_STARTED:after`.
pub const CLAIRE_GIBSON: Slot = Slot(42_060);

fn claire_when(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    let Some(planet) = c.event.payload.planet(Key::Planet) else { return false };
    c.event.payload.system(Key::System).is_some()
        && board::controls(c.state, me, planet)
        && leaders::has_commander_ability_in(c.state, me, Leader::ClaireGibson)
        && fleet::remaining_all(c.state, scope(c.kernel), me, Unit::InfantryI) > 0
}

fn claire(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let (Some(system), Some(planet)) = (eff.event.payload.system(Key::System), eff.event.payload.planet(Key::Planet)) else { return Ok(()) };
    let (s, k) = eff.state_and_kernel();
    let form = placed_form_overridden(s, k, scope(k), me, UnitBaseType::Infantry);
    let _ = fx::place_counted(s, k, scope(k), me, system, Some(planet), form, 1);
    Ok(())
}

/// Registered with the acquired commanders (`borrowed_commanders`), after every faction module and the technologies.
pub fn register_last(t: &mut AbilityTable) {
    t.add(Spec::new(CLAIRE_GIBSON, AbilityKind::Leader, EventKind::GroundCombatStarted, Relation::After, claire).optional().when(claire_when));
}
