//! yin: Yin Spinner Omega (`factions::yin::yin_spinner`): "After you produce units: place up to 2 infantry from your reinforcements on any planet you control or in any space area that contains
//! 1 or more of your ships." Optional (the window's own use / decline is the "may", so the spot is the only question). Registered for every seat (`technology:*:yso:UNITS_PRODUCED:after`); the
//! condition names the producer holding the technology, a spot, and an infantry in the box.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Technology, UnitBaseType};
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::state::{EventKind, Key};

use super::code;
use super::place::{choose, form_and_room, spots, Placement, Target};

/// Yin Spinner Omega (`technology:*:yso:UNITS_PRODUCED:after`).
pub const YIN_SPINNER: Slot = Slot(44_005);

const COUNT: usize = 2;

fn condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    c.event.payload.seat(Key::Player) == Some(me)
        && c.state.players().by_seat(me).is_some_and(|p| p.tech.owned.contains(Technology::YinSpinnerOmega))
        && !spots(c.state, me, Target::ControlledPlanetOrShipSpace).is_empty()
        && form_and_room(c.state, c.kernel, me, UnitBaseType::Infantry, 1).is_some_and(|(_, room)| room > 0)
}

fn spin(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    choose(eff, Placement { who: me, base: UnitBaseType::Infantry, count: COUNT, target: Target::ControlledPlanetOrShipSpace, optional: false, ability: code::YIN_SPINNER });
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(YIN_SPINNER, AbilityKind::Technology, EventKind::UnitsProduced, Relation::After, spin).optional().when(condition));
}
