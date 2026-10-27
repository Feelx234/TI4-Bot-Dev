//! Production: who can produce, how much, where units may go and what a unit costs (package B3;
//! LRR 67, 68, 79). The reads of `production.rs`: `price_of_under`, `producers`, `mobile_docks`,
//! `capacity`, `module_production`, `barred_capacity`, `structures_on`, `structure_allowed`,
//! `placements`, `war_machine_bonus`.
//!
//! Everything the core cannot see yet is asked of an [`Env`]: faction hooks and the few rules whose
//! owners have not landed (Ral Nel's silenced docks, Quietus, entropic scars, space stations). The
//! string engine implements the trait with its own modules, so the sums stay exact; the hook table
//! replaces it when the factions are ported.

use crate::containers::SmallList;
use crate::generated::{Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::query::economy::{self, Spend};
use crate::query::{board, laws};
use crate::state::{CoreState, Piece, RoundKind};
use crate::tables::Scope;

/// What the production rules ask of the factions and of rules not yet in the core.
pub trait Env {
    /// `hooks_economy::planet_spend_value` (a fold): the planet's value as `who`'s units read it.
    fn planet_spend_value(&self, who: Seat, planet: Planet, spend: Spend, value: i64) -> i64;
    /// `hooks_economy::extra_production`: PRODUCTION a module grants the space area of `system`.
    fn extra_production(&self, who: Seat, system: System) -> i64;
    /// `hooks_economy::extra_production_planet`.
    fn extra_production_planet(&self, who: Seat, system: System, planet: Planet) -> i64;
    /// `hooks_economy::has_cannot_produce`: some module registered the hook.
    fn has_cannot_produce(&self) -> bool;
    /// `hooks_economy::cannot_produce`.
    fn cannot_produce(&self, who: Seat, unit: UnitBaseType, producer: UnitBaseType) -> bool;
    /// `factions::ralnel::silenced_in_space`: a structure in the space area that cannot use PRODUCTION.
    fn silenced_in_space(&self, who: Seat, unit: Unit) -> bool;
    /// `factions::crimson::abilities_lost`.
    fn abilities_lost(&self, who: Seat, system: System) -> bool;
    /// `entropic_scars::abilities_usable` for the system.
    fn abilities_usable(&self, system: System) -> bool;
    /// `galaxy::is_space_station`.
    fn is_space_station(&self, planet: Planet) -> bool;
    /// `bastion_units::resource_bonus`: the Helios dock term of `planet_value_now`.
    fn resource_bonus(&self, planet: Planet) -> i64;
}

/// What the rules read besides the state: the source scope, the blanked-laws flag and the [`Env`].
#[derive(Clone, Copy)]
pub struct Cx<'a> {
    pub core: &'a CoreState,
    pub scope: Scope,
    pub blanked: bool,
    pub env: &'a dyn Env,
}

/// A producing unit and the planet it sits on (`None`: the space area).
pub type Producers = SmallList<(Piece, Option<Planet>), 12>;

/// `production::structure_limit`.
#[must_use]
pub const fn structure_limit(base: UnitBaseType) -> Option<usize> {
    match base {
        UnitBaseType::Spacedock => Some(1),
        UnitBaseType::Pds => Some(2),
        _ => None,
    }
}

/// `production::price_of_under`: `(cost, units made per purchase)`. `cost_halves` is the printed
/// cost doubled (`None`: no cost printed, which the string reads as 0); `single` is Regulated
/// Conscription (`laws::single_unit_production`).
#[must_use]
pub fn price(cost_halves: Option<u8>, single: bool) -> (i64, usize) {
    let halves = i64::from(cost_halves.unwrap_or(0));
    if halves == 1 {
        return (1, if single { 1 } else { 2 });
    }
    // ceil(halves / 2), capped like the string loop.
    (((halves + 1) / 2).clamp(0, 64), 1)
}

/// `price_of_under` for a unit type, with the law read from the state.
#[must_use]
pub fn price_of(cx: &Cx<'_>, unit: Unit) -> (i64, usize) {
    price(unit.data().cost_halves, laws::single_unit_production(cx.core, cx.blanked))
}

/// `production::producers`: the seat's units with PRODUCTION in `system`: the space area first
/// (list order), then each planet in planet order.
#[must_use]
pub fn producers(cx: &Cx<'_>, who: Seat, system: System) -> Producers {
    let mut found = Producers::new();
    let produces = |u: Unit| u.data().production.is_some();
    for p in board::space_units(cx.core, system) {
        let k = p.kind();
        if p.owner() == who && produces(k) && !cx.env.silenced_in_space(who, k) {
            found.push((*p, None));
        }
    }
    for planet in sorted_planets(cx.core, system).as_slice().iter().copied() {
        for p in board::planet_units(cx.core, planet) {
            if p.owner() == who && produces(p.kind()) {
                found.push((*p, Some(planet)));
            }
        }
    }
    found
}

/// The planets whose entries live in `system`, in id order (the order of the string `BTreeMap`s).
fn sorted_planets(core: &CoreState, system: System) -> SmallList<Planet, 8> {
    let mut v = SmallList::new();
    for p in board::planets_of(core, system) {
        v.push(p);
    }
    v.as_mut_slice().sort_unstable();
    v
}

/// `production::mobile_docks`: space-only structures with PRODUCTION in the space area.
#[must_use]
pub fn mobile_docks(cx: &Cx<'_>, who: Seat, system: System) -> SmallList<Piece, 4> {
    let mut out = SmallList::new();
    for p in board::space_units(cx.core, system) {
        let d = p.kind().data();
        if p.owner() == who && d.space_only && d.structure && d.production.is_some() {
            out.push(*p);
        }
    }
    out
}

/// `production::war_machine_bonus`.
#[must_use]
pub fn war_machine_bonus(core: &CoreState, who: Seat) -> i64 {
    let seq = core.meta().activation_seq;
    core.players().by_seat(who).map_or(0, |p| 5 * p.round_marks.of(RoundKind::WarMachine).filter(|n| *n == seq).count() as i64)
}

/// `production::planet_value_now`.
#[must_use]
pub fn planet_value_now(cx: &Cx<'_>, planet: Planet, spend: Spend) -> i64 {
    let extra = if spend == Spend::Resources { cx.env.resource_bonus(planet) } else { 0 };
    economy::value_now(cx.core, cx.scope, cx.blanked, planet, spend, extra)
}

fn unit_production(unit: Unit, resources: i64) -> i64 {
    unit.data().production.map_or(0, |p| i64::from(p.value) + if p.plus_planet_resources { resources } else { 0 })
}

fn dock_resources(cx: &Cx<'_>, who: Seat, planet: Option<Planet>) -> i64 {
    planet.map_or(0, |pl| cx.env.planet_spend_value(who, pl, Spend::Resources, planet_value_now(cx, pl, Spend::Resources)))
}

/// `production::capacity` (68.1a).
#[must_use]
pub fn capacity(cx: &Cx<'_>, who: Seat, system: System) -> i64 {
    producers(cx, who, system)
        .as_slice()
        .iter()
        .map(|(u, pl)| unit_production(u.kind(), dock_resources(cx, who, *pl)))
        .sum::<i64>()
        + war_machine_bonus(cx.core, who)
        + module_production(cx, who, system)
}

/// `production::module_production`.
#[must_use]
pub fn module_production(cx: &Cx<'_>, who: Seat, system: System) -> i64 {
    cx.env.extra_production(who, system).max(0)
        + module_planet_producers(cx, who, system).as_slice().iter().map(|(_, v)| *v).sum::<i64>()
}

/// `production::module_planet_producers`: planets holding units or controlled, with their PRODUCTION.
#[must_use]
pub fn module_planet_producers(cx: &Cx<'_>, who: Seat, system: System) -> SmallList<(Planet, i64), 4> {
    let mut out = SmallList::new();
    for planet in sorted_planets(cx.core, system).as_slice() {
        let cell = cx.core.board().planet(*planet);
        if cell.units.is_empty() && cell.control.is_none() {
            continue;
        }
        let v = cx.env.extra_production_planet(who, system, *planet);
        if v > 0 {
            out.push((*planet, v));
        }
    }
    out
}

/// `production::barred_capacity`.
#[must_use]
pub fn barred_capacity(cx: &Cx<'_>, who: Seat, system: System, unit: UnitBaseType) -> i64 {
    if !cx.env.has_cannot_produce() {
        return 0;
    }
    producers(cx, who, system)
        .as_slice()
        .iter()
        .filter(|(u, _)| cx.env.cannot_produce(who, unit, u.kind().data().base_type))
        .map(|(u, pl)| unit_production(u.kind(), dock_resources(cx, who, *pl)))
        .sum()
}

/// `production::structures_on`: the seat's units of this base type on the planet.
#[must_use]
pub fn structures_on(cx: &Cx<'_>, who: Seat, planet: Planet, base: UnitBaseType) -> usize {
    board::planet_units(cx.core, planet).iter().filter(|p| p.owner() == who && p.kind().data().base_type == base).count()
}

/// `production::structure_allowed` (79.2).
#[must_use]
pub fn structure_allowed(cx: &Cx<'_>, who: Seat, planet: Planet, base: UnitBaseType) -> bool {
    if laws::structure_cap_lifted(cx.core, cx.blanked, base) {
        return true;
    }
    if laws::planet_is_demilitarized(cx.core, cx.blanked, planet) {
        return false;
    }
    structure_limit(base).is_none_or(|cap| structures_on(cx, who, planet, base) < cap)
}

/// Where a produced unit may go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Spot {
    Space,
    Planet(Planet),
}

/// `production::placements` (68.2 to 68.4).
#[must_use]
pub fn placements(cx: &Cx<'_>, who: Seat, system: System, unit: Unit) -> SmallList<Spot, 8> {
    let mut spots = SmallList::new();
    if cx.env.abilities_lost(who, system) {
        return spots;
    }
    let d = unit.data();
    if d.ship {
        let blockaded = board::space_units(cx.core, system).iter().any(|p| p.owner() != who && p.kind().data().ship);
        if !blockaded {
            spots.push(Spot::Space);
        }
        return spots;
    }
    if !cx.env.abilities_usable(system) {
        return spots;
    }
    let made: Producers = {
        let mut m = Producers::new();
        for (u, pl) in producers(cx, who, system).as_slice() {
            if !cx.env.cannot_produce(who, d.base_type, u.kind().data().base_type) {
                m.push((*u, *pl));
            }
        }
        m
    };
    let mut candidates: SmallList<Planet, 12> = SmallList::new();
    for (_, pl) in made.as_slice() {
        if let Some(p) = pl {
            candidates.push(*p);
        }
    }
    for (p, _) in module_planet_producers(cx, who, system).as_slice() {
        candidates.push(*p);
    }
    for planet in candidates.as_slice() {
        let planet = *planet;
        if laws::production_forbidden_on(cx.core, cx.blanked, planet)
            || laws::planet_is_demilitarized(cx.core, cx.blanked, planet)
            || cx.env.is_space_station(planet)
            || !structure_allowed(cx, who, planet, d.base_type)
        {
            continue;
        }
        let s = Spot::Planet(planet);
        if !spots.as_slice().contains(&s) {
            spots.push(s);
        }
    }
    if made.as_slice().iter().any(|(_, pl)| pl.is_none()) || cx.env.extra_production(who, system) > 0 {
        if !spots.as_slice().contains(&Spot::Space) {
            spots.push(Spot::Space);
        }
    }
    spots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_rounds_up_and_halves_the_yield() {
        assert_eq!(price(Some(1), false), (1, 2));
        assert_eq!(price(Some(1), true), (1, 1));
        assert_eq!(price(Some(2), false), (1, 1));
        assert_eq!(price(Some(5), false), (3, 1));
        assert_eq!(price(None, false), (0, 1));
    }

    #[test]
    fn structure_limits() {
        assert_eq!(structure_limit(UnitBaseType::Pds), Some(2));
        assert_eq!(structure_limit(UnitBaseType::Spacedock), Some(1));
        assert_eq!(structure_limit(UnitBaseType::Mech), None);
    }
}
