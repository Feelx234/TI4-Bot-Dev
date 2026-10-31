//! Whether a production by an ability would ask anything, from the position alone (package F2, the hook slots `leader_action` of
//! Letani Miasmiala and Umbat).
//!
//! The string reads `ProductionWindow::for_ability(..).with_max_unit_cost(..).pending_choice(..).is_some()`. The core window
//! ([`ProductionWindow::ability`]) takes the facts of the use as data; here they are read from the position: the units the seat may
//! build, the fleet limit, the cost reduction of the hooks and the exemptions of the Cabal commander (`production_view::buildable_for`,
//! `fleet::limit`, `production_cost_reduction`; the same readings as the home use of the Warfare secondary).

use super::window::ProductionWindow;
use crate::generated::{Ability, Leader, System, Technology, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::children::ProduceFacts;
use crate::kernel::stage::Window;
use crate::kernel::Kernel;
use crate::query::tech::{self, UnitRoster};
use crate::state::CoreState;
use crate::tables::Scope;

/// `production::BUILDABLE`: the base types a seat may produce, in the order of the offer.
const BUILDABLE: [UnitBaseType; 9] = [
    UnitBaseType::Fighter,
    UnitBaseType::Infantry,
    UnitBaseType::Carrier,
    UnitBaseType::Cruiser,
    UnitBaseType::Destroyer,
    UnitBaseType::Dreadnought,
    UnitBaseType::Mech,
    UnitBaseType::Flagship,
    UnitBaseType::Warsun,
];

/// The fleet bill of a seat whose fleet supply is unlimited for the round (`fleet::UNLIMITED_FLEET_BILL`).
const UNLIMITED_FLEET_BILL: i64 = 10_000;

/// `production_view::buildable_for`: what `who` may produce, in the order of the offer. A unit upgrade owned replaces the unit it covers,
/// the faction's own unit stands in for a plain one, a mech and a flagship exist only as the faction's own, a war sun needs the
/// technology (or a faction that prints its own), and the hook table may name another form (Mentak's Corsair). `None`: no such seat.
#[must_use]
pub fn buildable_for(core: &CoreState, k: &Kernel, scope: Scope, roster: &UnitRoster, who: Seat) -> Option<[Option<Unit>; 9]> {
    let p = core.players().by_seat(who)?;
    let mut out = [None; 9];
    let mut slots = out.iter_mut();
    for base in BUILDABLE {
        let own = tech::faction_unit(roster, p.faction, base);
        if base == UnitBaseType::Warsun && !p.tech.owned.contains(Technology::WarSun) && own.is_none() {
            continue;
        }
        let chosen = tech::unlocked_upgrade(scope, base, p.faction, &p.tech.owned)
            .or(own)
            .or_else(|| if matches!(base, UnitBaseType::Mech | UnitBaseType::Flagship) { None } else { tech::generic_unit(scope, base) });
        let form = chosen.map(|unit| k.hooks.strategy.unit_form_override(core, k, who, base, unit).unwrap_or(unit));
        if let (Some(form), Some(slot)) = (form, slots.next()) {
            *slot = Some(form);
        }
    }
    Some(out)
}

/// `faction_abilities::armada_bonus`: Letnev's Armada lifts the fleet limit by two.
fn armada(core: &CoreState, who: Seat) -> i32 {
    if core.players().by_seat(who).is_some_and(|p| p.faction.data().abilities.contains(&Ability::Armada)) { 2 } else { 0 }
}

/// The fleet limit one seat judges a production by (`fleet::limit`, or the unlimited bill for the round of Darktalon Treilla's hero): the
/// pool with the tokens other players hold in it, held by Fleet Regulations, lifted by Armada and the hook table.
#[must_use]
pub fn fleet_limit(core: &CoreState, k: &Kernel, blanked: bool, who: Seat) -> i64 {
    if crate::query::fleet::is_unlimited(core, who) {
        return UNLIMITED_FLEET_BILL;
    }
    let foreign = k.hooks.direct.mahact_foreign_tokens(core, who);
    let capped = crate::query::laws::fleet_pool_cap(core, blanked, crate::query::fleet::pool_base(core, who, foreign));
    let lifted = k.hooks.general.fleet_supply(core, k, who, capped + armada(core, who));
    i64::from(lifted).max(0)
}

/// The facts of a production by an ability (`ProductionWindow::for_ability`) that `who` would open in `system` with `limit` units and
/// a ceiling on the printed cost of one unit, read from the position. `None`: a seat that does not exist or a number beyond 32 bits.
#[must_use]
pub fn ability_facts(core: &CoreState, k: &Kernel, roster: &UnitRoster, who: Seat, system: System, limit: i64, max_unit_cost: Option<i32>) -> Option<ProduceFacts> {
    let scope = Scope(u32::from(k.sources.0));
    let blanked = crate::query::reach::blanked(core);
    Some(ProduceFacts {
        who,
        system,
        limit: i32::try_from(limit).ok()?,
        max_unit_cost,
        small_exemptions: if crate::query::leaders::has_commander_ability_in(core, who, Leader::ThatWhichMoldsFlesh) { 2 } else { 0 },
        discount: i32::try_from(k.hooks.economy.production_cost_reduction(core, who, system).max(0)).ok()?,
        fleet_limit: i32::try_from(fleet_limit(core, k, blanked, who)).ok()?,
        armada: armada(core, who),
        flagship_free: crate::query::leaders::has_commander_ability_in(core, who, Leader::NavarchFeng),
        buildable: buildable_for(core, k, scope, roster, who)?,
    })
}

/// Whether a production by an ability (`limit` units, none costing more than `max_unit_cost`) would ask `who` anything in `system`:
/// the string `ProductionWindow::for_ability(..).with_max_unit_cost(..).pending_choice(..).is_some()`.
#[must_use]
pub fn would_ask(core: &CoreState, k: &Kernel, roster: &UnitRoster, who: Seat, system: System, limit: i64, max_unit_cost: Option<i32>) -> bool {
    ability_facts(core, k, roster, who, system, limit, max_unit_cost).is_some_and(|facts| ProductionWindow::ability(&facts).ask_of(core, k).is_some())
}
