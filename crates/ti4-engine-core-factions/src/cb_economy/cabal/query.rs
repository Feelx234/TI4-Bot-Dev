//! cabal: tier Q: pure hooks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CB). Owned by CB only.
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | movement `gravity_rifts`, `rift_roll_exempt`, `rift_crucible`, `move_bonus` | `tear_systems`, `tear_exempts_roll`, `crucible_in_effect`, `alraith_move_bonus` | Q | `movement.rs` (A2, done) |
//! | `economy.production_unit_exchange` | `amalgamation_offer` | Q | `economy.production_unit_exchange` (pure offer) |
//! | `economy.production_unit_exchange_performed` | `amalgamation_perform` | W | `offer.rs`: returns a captured model (`supply::return_captured`) |
//! | `strategy.research_waiver_offer`, `research_waiver_paid` | `riftmeld_offer`, `riftmeld_paid` | W | `offer.rs` (needs the unit-upgrade table of the corpus) |
//! | `direct.cabal_note_replenished` | `note_replenished` | O | `observe.rs` |
//! | `timing_abilities`, `mapped_component_actions`, `perform_component`, leaders | | W | A5 / B7 |

use ti4_engine_core::generated::{Faction, System, Unit};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::CoreState;

use crate::common::{is_faction, player, scope};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.economy.production_unit_exchange.push(amalgamation_offer);
}

/// The base type the string engine names a unit by (`supply::base_type_of`: the catalogue's, else the id itself).
fn base_of(k: &Kernel, unit: Unit) -> Result<ti4_engine_core::generated::UnitBaseType, Unit> {
    scope(k).unit(unit).map(|d| d.base_type).ok_or(unit)
}

/// Amalgamation: "You may return 1 captured unit of that type to produce that unit without spending
/// resources." Offered when the Cabal holds a captured model of the unit's base type.
pub fn amalgamation_offer(s: &CoreState, k: &Kernel, who: Seat, _: System, unit: Unit) -> bool {
    if !is_faction(s, who, Faction::TheVuilraithCabal) {
        return false;
    }
    let base = base_of(k, unit);
    player(s, who).is_some_and(|p| p.misc.captured_units.as_slice().iter().any(|(_, captured)| base_of(k, *captured) == base))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cb_economy::testkit::table;

    #[test]
    fn amalgamation_needs_a_captured_model_of_the_same_base_type() {
        let (mut s, k) = table(&[Faction::TheVuilraithCabal, Faction::TheFederationOfSol]);
        let sys = ti4_engine_core::ids::system_at(30);
        assert!(!amalgamation_offer(&s, &k, Seat(0), sys, Unit::CruiserI));
        s.players_mut().edit(0).misc.captured_units.push((Seat(1), Unit::CruiserII));
        assert!(amalgamation_offer(&s, &k, Seat(0), sys, Unit::CruiserI), "by base type, whichever upgrade");
        assert!(!amalgamation_offer(&s, &k, Seat(0), sys, Unit::CarrierI));
        s.players_mut().edit(1).misc.captured_units.push((Seat(0), Unit::CarrierI));
        assert!(!amalgamation_offer(&s, &k, Seat(1), sys, Unit::CarrierI), "only the Cabal exchanges");
    }
}
