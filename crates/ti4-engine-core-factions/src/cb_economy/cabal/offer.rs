//! cabal: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CB). Owned by CB only.
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | `economy.production_unit_exchange_performed` | `amalgamation_perform` | W (asks nothing) | `economy.production_unit_exchange_performed`: returns one captured model |
//! | `strategy.research_waiver_offer` | `riftmeld_offer` | W (asks nothing; lane G4) | `strategy.research_waiver_offer`: the unit-upgrade table is `UnitData::{upgrades_from, required_technology}`; the window that asks is the research window's (`production::strategy_cards::Research`) |
//! | `strategy.research_waiver_paid` | `riftmeld_paid` | W (lane G4) | `strategy.research_waiver_paid`: returns the captured model the payment names |
//! | `timing_abilities`, `mapped_component_actions`, `perform_component`, leaders | | W, BLOCKED | A5 / B7 |

use ti4_engine_core::generated::{Faction, System, Technology, Unit, UnitBaseType};
use ti4_engine_core::hooks::{HookTable, ResearchWaiver, Waiver};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::{Cx, Kernel};

use crate::common::{is_faction, scope};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.economy.production_unit_exchange_performed.push(amalgamation_perform);
    t.strategy.research_waiver_offer.push(riftmeld_offer);
    t.strategy.research_waiver_paid.push(riftmeld_paid);
}

/// The waiver of Riftmeld, as `Waiver::code` (the module is the Cabal's, its only waiver).
pub const RIFTMELD: u16 = 0;

/// `unit_upgrade_base`: the base type a unit-upgrade technology replaces. "A technology that no (upgraded type, replaced type) pair of the unit table names upgrades no unit" -- the corpus keeps both halves on
/// the UNIT row (`required_technology` of the upgrade, `upgrades_from` the type it replaces).
#[must_use]
pub fn unit_upgrade_base(k: &Kernel, tech: Technology) -> Option<UnitBaseType> {
    let scope = scope(k);
    Unit::ALL.iter().filter_map(|u| scope.unit(*u)).find(|d| d.upgrades_from.is_some() && d.required_technology == ti4_engine_core::generated::Requirement::Known(tech)).map(|d| d.base_type)
}

/// The code of a payment: the captured model of `owner` (a seat) of the form `unit`.
#[must_use]
pub fn payment_code(owner: Seat, unit: Unit) -> u16 {
    (u16::from(owner.0) << 8) | u16::try_from(unit.index()).unwrap_or(0) // mutants:skip -- a unit index always fits a u16, the fallback is never taken
}

/// The captured model a payment code names: the seat it belongs to and its form (the inverse of [`payment_code`]).
#[must_use]
pub fn payment_model(code: u16) -> Option<(Seat, Unit)> {
    Unit::from_index(usize::from(code & 0xff)).map(|unit| (Seat(u8::try_from(code >> 8).unwrap_or(u8::MAX)), unit))
}

/// `riftmeld_payments`: every captured model the Cabal could return for `tech`, in capture order. (A model of one owner and form held twice is one payment: both pay the same.)
fn payments(s: &ti4_engine_core::state::CoreState, k: &Kernel, who: Seat, tech: Technology) -> Vec<(Waiver, Seat, UnitBaseType)> {
    let Some(base) = unit_upgrade_base(k, tech) else { return Vec::new() };
    let mut found: Vec<(Waiver, Seat, UnitBaseType)> = Vec::new();
    for (owner, unit) in s.players().by_seat(who).map(|p| p.misc.captured_units.as_slice()).unwrap_or(&[]) {
        if unit.data().base_type != base {
            continue;
        }
        let waiver = Waiver { module: Faction::TheVuilraithCabal, code: payment_code(*owner, *unit) };
        if !found.iter().any(|(w, _, _)| *w == waiver) {
            found.push((waiver, *owner, base));
        }
    }
    found
}

/// Riftmeld: "When you research a unit upgrade technology: you may return 1 captured unit of that type to ignore all of the technology's prerequisites." Pure: asks nothing and mutates nothing.
#[must_use]
pub fn riftmeld_offer(s: &ti4_engine_core::state::CoreState, k: &Kernel, who: Seat, tech: Technology) -> Option<ResearchWaiver> {
    if !is_faction(s, who, Faction::TheVuilraithCabal) {
        return None;
    }
    let payments: ti4_engine_core::containers::SmallList<Waiver, 4> = payments(s, k, who, tech).into_iter().map(|(w, _, _)| w).collect();
    (!payments.as_slice().is_empty()).then_some(ResearchWaiver { waiver: Waiver { module: Faction::TheVuilraithCabal, code: RIFTMELD }, payments })
}

/// `riftmeld_paid`: return the captured model the payment names. The target is recomputed and revalidated before any mutation, so a stale choice researches nothing.
pub fn riftmeld_paid(cx: &mut Cx<'_>, who: Seat, tech: Technology, payment: Waiver) -> bool {
    if payment.module != Faction::TheVuilraithCabal || !is_faction(cx.state, who, Faction::TheVuilraithCabal) {
        return false;
    }
    let Some((_, owner, base)) = payments(cx.state, cx.kernel, who, tech).into_iter().find(|(w, _, _)| *w == payment) else { return false };
    ti4_engine_core::rules::production::supply::return_captured(cx.state, cx.kernel, who, owner, base).is_some()
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
