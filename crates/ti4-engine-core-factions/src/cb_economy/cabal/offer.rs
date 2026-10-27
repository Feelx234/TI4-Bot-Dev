//! cabal: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CB). Owned by CB only.
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | `economy.production_unit_exchange_performed` | `amalgamation_perform` | W (asks nothing) | `economy.production_unit_exchange_performed`: returns one captured model |
//! | `strategy.research_waiver_offer` | `riftmeld_offer` | W, BLOCKED | needs the unit-upgrade table (`unit_upgrade_base`) and the research window (B-research) |
//! | `strategy.research_waiver_paid` | `riftmeld_paid` | W, BLOCKED | the same |
//! | `timing_abilities`, `mapped_component_actions`, `perform_component`, leaders | | W, BLOCKED | A5 / B7 |

use ti4_engine_core::generated::{Faction, System, Unit};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::{Cx, Kernel};

use crate::common::{is_faction, scope};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.economy.production_unit_exchange_performed.push(amalgamation_perform);
}

/// The base type the string engine names a unit by (the catalogue's, else the id itself).
fn base_of(k: &Kernel, unit: Unit) -> Result<ti4_engine_core::generated::UnitBaseType, Unit> {
    scope(k).unit(unit).map(|d| d.base_type).ok_or(unit)
}

/// Perform the exchange: return one captured model of this unit's base type to its owner. Removing the row is the
/// return (a captured unit is off the board and on the captor's sheet). `false`, nothing changed, when none is held.
pub fn amalgamation_perform(cx: &mut Cx<'_>, who: Seat, _: System, unit: Unit) -> bool {
    if !is_faction(cx.state, who, Faction::TheVuilraithCabal) {
        return false;
    }
    let base = base_of(cx.kernel, unit);
    let kernel = cx.kernel;
    let Some(index) = cx
        .state
        .players()
        .by_seat(who)
        .and_then(|p| p.misc.captured_units.as_slice().iter().position(|(_, captured)| base_of(kernel, *captured) == base))
    else {
        return false;
    };
    let seat = cx.state.players().iter().position(|p| p.seat == who).unwrap_or(0);
    cx.state.players_mut().edit(seat).misc.captured_units.remove(index);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cb_economy::testkit::table;

    #[test]
    fn the_exchange_returns_the_first_captured_model_of_that_base_type() {
        let (mut s, k) = table(&[Faction::TheVuilraithCabal, Faction::TheFederationOfSol]);
        let sys = ti4_engine_core::ids::system_at(30);
        let mut ask = ti4_engine_core::kernel::FirstOption;
        assert!(!amalgamation_perform(&mut Cx::new(&mut s, &k, &mut ask), Seat(0), sys, Unit::CruiserI), "nothing captured");
        let held = &mut s.players_mut().edit(0).misc.captured_units;
        held.push((Seat(1), Unit::CarrierI));
        held.push((Seat(1), Unit::CruiserII));
        held.push((Seat(1), Unit::CruiserI));
        assert!(amalgamation_perform(&mut Cx::new(&mut s, &k, &mut ask), Seat(0), sys, Unit::CruiserI));
        let left: Vec<Unit> = s.players().by_seat(Seat(0)).unwrap().misc.captured_units.as_slice().iter().map(|(_, u)| *u).collect();
        assert_eq!(left, vec![Unit::CarrierI, Unit::CruiserI]);
        assert!(!amalgamation_perform(&mut Cx::new(&mut s, &k, &mut ask), Seat(1), sys, Unit::CarrierI), "only the Cabal exchanges");
    }
}
