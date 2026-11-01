//! saar: Scavenger Zeta DEPLOY (`factions::saar::scavenger_zeta`): "After you gain control of a planet, you may spend 1 trade good to place 1 mech on that planet."
//! Optional (the window's own use / decline is the "may"; nothing is asked inside). Registered for every seat
//! (`unit:*:saar_mech:PLANET_CONTROL_GAINED:after`); the condition names the Saar player who gained the planet, a trade good to spend and a mech in the box.
//! The Keleres agent that lets a commodity pay is dropped content: the price is a plain trade good.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Faction, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::{fx, Kernel};
use ti4_engine_core::state::{CoreState, EventKind, Key, Payload};

use crate::cd_registry::yin::timing::placed_form_overridden;

/// Scavenger Zeta (`unit:*:saar_mech:PLANET_CONTROL_GAINED:after`).
pub const ZETA: Slot = Slot(42_000);

/// `gained_by`: the event names `who` as the gainer of a planet in a system.
fn gained_by(p: &Payload, who: Seat) -> Option<(ti4_engine_core::generated::System, ti4_engine_core::generated::Planet)> {
    (p.seat(Key::Player) == Some(who)).then_some(())?;
    Some((p.system(Key::System)?, p.planet(Key::Planet)?))
}

/// `deploy_ready`: a Saar seat with a trade good and a mech it may place (31.4).
fn ready(s: &CoreState, k: &Kernel, who: Seat) -> bool {
    let plays = s.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheClanOfSaar && p.econ.trade_goods >= 1);
    let scope = crate::common::scope(k);
    plays && placed_form_overridden(s, k, scope, who, UnitBaseType::Mech).is_some_and(|mech| fx::box_has(s, k, scope, who, mech))
}

fn condition(c: &CondCx<'_>) -> bool {
    gained_by(&c.event.payload, c.def.owner).is_some() && ready(c.state, c.kernel, c.def.owner)
}

fn deploy(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some((system, planet)) = gained_by(&eff.event.payload, me) else { return Ok(()) };
    let (s, k) = eff.read();
    if !ready(s, k, me) {
        return Ok(());
    }
    if !fx::spend_trade_goods(eff.state(), me, 1) {
        return Ok(());
    }
    let scope = crate::common::scope(eff.kernel());
    let (s, k) = eff.state_and_kernel();
    let mech = placed_form_overridden(s, k, scope, me, UnitBaseType::Mech);
    let _ = fx::place_counted(s, k, scope, me, system, Some(planet), mech, 1);
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(ZETA, AbilityKind::Unit, EventKind::PlanetControlGained, Relation::After, deploy).optional().when(condition));
}
