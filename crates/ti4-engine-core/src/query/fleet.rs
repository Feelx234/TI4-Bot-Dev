//! Component limits and fleet supply (`supply.rs`, `fleet.rs`): the plastic a seat has on the
//! board, and what its ships in one system charge against the fleet pool and the capacity.

use crate::generated::{Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::state::CoreState;
use crate::tables::{Scope, source_bit};

use super::board;

/// `supply::UNCAPPED`: anything without plastic.
pub const UNCAPPED: i64 = 99;

/// `fleet::UNLIMITED_FLEET_BILL`.
pub const UNLIMITED_FLEET_BILL: i64 = 10_000;

/// `supply::plastic`: plastic per player, by base type.
#[must_use]
pub const fn plastic(base: UnitBaseType) -> Option<i64> {
    Some(match base {
        UnitBaseType::Flagship => 1,
        UnitBaseType::Warsun => 2,
        UnitBaseType::Dreadnought => 5,
        UnitBaseType::Cruiser | UnitBaseType::Destroyer => 8,
        UnitBaseType::Carrier | UnitBaseType::Mech => 4,
        UnitBaseType::Pds => 6,
        UnitBaseType::Spacedock => 3,
        UnitBaseType::Fighter | UnitBaseType::Infantry => return None,
    })
}

fn in_scope(scope: Scope, unit: Unit) -> bool {
    scope.0 & source_bit(unit.data().source) != 0
}

/// `supply::held` without the Crimson hero card: the seat's pieces of this base type in every
/// space area and on every planet, plus the units other players hold captured. Add the card's
/// ships (`factions::crimson_cards::held_on_card`) on the string side.
#[must_use]
pub fn held_on_board(core: &CoreState, scope: Scope, seat: Seat, base: UnitBaseType) -> i64 {
    let matches = |u: Unit| in_scope(scope, u) && u.data().base_type == base;
    let mut total = 0;
    for (_, cell) in core.board().systems() {
        total += cell.units.as_slice().iter().filter(|p| p.owner() == seat && matches(p.kind())).count() as i64;
    }
    for (_, cell) in core.board().planets() {
        total += cell.units.as_slice().iter().filter(|p| p.owner() == seat && matches(p.kind())).count() as i64;
    }
    for captor in core.players().iter() {
        total +=
            captor.misc.captured_units.as_slice().iter().filter(|(owner, unit)| *owner == seat && matches(*unit)).count() as i64;
    }
    total
}

/// `supply::remaining` with `on_card` ships on the Crimson hero card: how many more of this unit
/// may be placed. [`UNCAPPED`] for a unit out of the scope or without plastic.
#[must_use]
pub fn remaining(core: &CoreState, scope: Scope, seat: Seat, unit: Unit, on_card: i64) -> i64 {
    if !in_scope(scope, unit) {
        return UNCAPPED;
    }
    let base = unit.data().base_type;
    let Some(limit) = plastic(base) else {
        return UNCAPPED;
    };
    (limit - held_on_board(core, scope, seat, base) - on_card).max(0)
}

/// `fleet::counts_against_supply`: non-fighter ships that are not carried (37.1).
#[must_use]
pub fn counts_against_supply(unit: Unit) -> bool {
    let d = unit.data();
    d.ship && d.base_type != UnitBaseType::Fighter && !consumes_capacity(unit)
}

/// `UnitType::is_ground_force`: infantry and mechs, plus the Titans' PDS.
#[must_use]
pub fn is_ground_force(unit: Unit) -> bool {
    matches!(unit.data().base_type, UnitBaseType::Infantry | UnitBaseType::Mech)
        || matches!(unit, Unit::HelTitanI | Unit::HelTitanII)
}

/// `UnitType::consumes_capacity`: fighters and ground forces that are not structures.
#[must_use]
pub fn consumes_capacity(unit: Unit) -> bool {
    let d = unit.data();
    d.base_type == UnitBaseType::Fighter || (is_ground_force(unit) && !d.structure)
}

/// `UnitType::capacity`.
#[must_use]
pub fn capacity(unit: Unit) -> i64 {
    i64::from(unit.data().capacity.unwrap_or(0))
}

/// `UnitType::capacity_cost` (`capacityUsed`).
#[must_use]
pub fn capacity_cost(unit: Unit) -> i64 {
    i64::from(unit.data().capacity_used.unwrap_or(0))
}

/// `UnitType::fighter_support`: the fighters a structure supports without capacity ("up to N
/// fighters" in the unit's ability text; the generator carries no field for it yet).
#[must_use]
pub const fn fighter_support(unit: Unit) -> i64 {
    match unit {
        Unit::BastionSpacedock | Unit::BastionSpacedock2 | Unit::SpaceDockI | Unit::SpaceDockII => 3,
        Unit::DimensionalTearI => 6,
        Unit::DimensionalTearII => 12,
        _ => 0,
    }
}

/// `fleet::Arrival`: one unit type not yet placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrival {
    pub kind: Unit,
    pub count: i64,
    /// Placed in the space area rather than on a planet.
    pub in_space: bool,
}

/// `fleet::Standing`: fleet supply and transport for one seat in one system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Standing {
    pub fleet_limit: i64,
    pub fleet_charged: i64,
    pub transport: i64,
    pub consumed: i64,
    pub fighters_charged: i64,
    pub capacity_excess: i64,
    pub ships: i64,
    pub carried: i64,
    pub fighters: i64,
    pub fighter_support: i64,
    pub upgraded: bool,
    pub half_weight: bool,
}

impl Standing {
    #[must_use]
    pub const fn fleet_headroom(&self) -> i64 {
        self.fleet_limit - self.fleet_charged
    }

    #[must_use]
    pub const fn fleet_excess(&self) -> i64 {
        let over = self.fleet_charged - self.fleet_limit;
        if over < 0 { 0 } else { over }
    }

    #[must_use]
    pub const fn capacity_free(&self) -> i64 {
        let free = self.transport - self.consumed;
        if free < 0 { 0 } else { free }
    }
}

const fn fighters_charged_to_fleet_pool(upgraded: bool, fighters: i64, excused: i64, overflow: i64) -> i64 {
    if !upgraded {
        return 0;
    }
    let unexcused = fighters - excused;
    let cap = if unexcused < 0 { 0 } else { unexcused };
    if overflow < cap { overflow } else { cap }
}

/// Planets whose units can sit under this system's structures: every planet of the system.
fn planets_in_system(core: &CoreState, system: System) -> impl Iterator<Item = Planet> + '_ {
    board::planets_of(core, system)
}

/// `fleet::standing_with`: the one arithmetic behind fleet supply and capacity. `fleet_limit` is
/// the limit already worked out (pool, laws, abilities); `half_weight` says whether a fighter type
/// counts as half a ship for this seat (Naalu Hybrid Crystal Fighter II).
#[must_use]
pub fn standing(
    core: &CoreState,
    scope: Scope,
    seat: Seat,
    system: System,
    fleet_limit: i64,
    arriving: Option<Arrival>,
    half_weight: &dyn Fn(Unit) -> bool,
) -> Standing {
    let space = || {
        board::space_units(core, system).iter().filter(|p| p.owner() == seat).map(|p| p.kind()).filter(|u| in_scope(scope, *u))
    };
    let landing = arriving.filter(|a| !a.in_space);
    let arrival = arriving.filter(|a| a.in_space);
    let (mut new_ships, mut new_transport, mut new_carried, mut new_fighters) = (0, 0, 0, 0);
    if let Some(a) = arrival {
        new_transport = capacity(a.kind) * a.count;
        if counts_against_supply(a.kind) {
            new_ships = a.count;
        }
        if a.kind.data().base_type == UnitBaseType::Fighter {
            new_fighters = capacity_cost(a.kind) * a.count;
        } else if consumes_capacity(a.kind) {
            new_carried = capacity_cost(a.kind) * a.count;
        }
    }
    let present = space().filter(|u| counts_against_supply(*u)).count() as i64 + new_ships;
    let transport: i64 = space().map(capacity).sum::<i64>() + new_transport;
    let is_fighter = |u: Unit| u.data().base_type == UnitBaseType::Fighter;
    let carried: i64 = space().filter(|u| consumes_capacity(*u) && !is_fighter(*u)).map(capacity_cost).sum::<i64>() + new_carried;
    let fighters: i64 = space().filter(|u| is_fighter(*u)).map(capacity_cost).sum::<i64>() + new_fighters;
    let mut support: i64 = 0;
    for planet in planets_in_system(core, system) {
        support += board::planet_units(core, planet)
            .iter()
            .filter(|p| p.owner() == seat && in_scope(scope, p.kind()))
            .map(|p| fighter_support(p.kind()))
            .sum::<i64>();
    }
    support += landing.map_or(0, |a| fighter_support(a.kind) * a.count);

    let excused = fighters.min(support);
    let consumed = carried + fighters - excused;
    let overflow = (consumed - transport).max(0);
    let upgraded_fighter = |u: Unit| is_fighter(u) && !matches!(u.data().required_technology, crate::generated::Requirement::None);
    let upgraded = space().any(upgraded_fighter) || arrival.is_some_and(|a| upgraded_fighter(a.kind));
    let fighters_charged = fighters_charged_to_fleet_pool(upgraded, fighters, excused, overflow);
    let half = space().chain(arrival.map(|a| a.kind)).any(|u| upgraded_fighter(u) && half_weight(u));
    let pool_charge = if half { (fighters_charged + 1) / 2 } else { fighters_charged };
    Standing {
        fleet_limit,
        fleet_charged: present + pool_charge,
        transport,
        consumed,
        fighters_charged,
        capacity_excess: (overflow - fighters_charged).max(0),
        ships: present,
        carried,
        fighters,
        fighter_support: support,
        upgraded,
        half_weight: half,
    }
}

/// The pool before laws and abilities: `Player.fleet_tokens` plus the tokens other players hold in
/// this seat's pool (Mahact Edict, `foreign`).
#[must_use]
pub fn pool_base(core: &CoreState, seat: Seat, foreign: i32) -> i32 {
    core.players().by_seat(seat).map_or(0, |p| i32::from(p.econ.fleet_tokens)) + foreign
}

/// `fleet::is_unlimited`: Darktalon Treilla's hero, for the round it was used.
#[must_use]
pub fn is_unlimited(core: &CoreState, seat: Seat) -> bool {
    let round = core.meta().round;
    core.players().by_seat(seat).is_some_and(|p| p.production_timers.fleet_supply_unlimited_until.get() == Some(round))
}
