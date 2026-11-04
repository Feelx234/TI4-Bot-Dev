//! Fleet composition reads of the surface (`opening::fleet_of`, content and the board only).

use ti4_content::ContentStore;
use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

/// `(ships that can carry something, ground forces)` of one player, wherever they stand. Capacity comes from the content, not from a
/// list of hull names; infantry being transported sit in the space area and still count.
#[must_use]
pub fn opening_fleet(state: &GameState, player: &PlayerId, content: &ContentStore, sources: SourceSet) -> (usize, usize) {
    let mut capacity_ships = 0;
    let mut infantry = 0;
    let ground = |unit: &ti4_model::units::Unit| -> bool {
        let kind = unit.type_id.as_str();
        kind.ends_with("infantry") || kind.ends_with("mech")
    };
    for system in state.board.values() {
        for unit in system.units.iter().filter(|unit| &unit.owner == player) {
            if ground(unit) {
                infantry += 1;
            } else if super::transit::capacity_of(content, sources, unit) > 0 {
                capacity_ships += 1;
            }
        }
        infantry += system.planet_units.values().flatten().filter(|unit| &unit.owner == player && ground(unit)).count();
    }
    (capacity_ships, infantry)
}

/// The fleet bill an unlimited seat (Letnev's hero for the round) reports: bounded so derived headroom stays within the printed-integer range
/// the policy features encode, and far above any reachable fleet.
pub const UNLIMITED_FLEET_BILL: i64 = 10_000;

/// Darktalon Treilla: during the game round her hero was used, fleet supply is limited by neither laws nor the pool.
#[must_use]
pub fn is_unlimited(state: &GameState, player: &PlayerId) -> bool {
    state.player(player).is_some_and(|seat| seat.fleet_supply_unlimited_until == Some(state.round))
}

/// One unit that has not been placed yet, for asking what a placement would leave behind.
#[derive(Debug, Clone, Copy)]
pub struct Arrival<'a> {
    pub kind: ti4_content::units::UnitType<'a>,
    pub count: i64,
    /// Placed in the space area rather than on a planet.
    pub in_space: bool,
}

/// Fleet supply and transport for one seat in one system, answered together (LRR 16, 37).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Standing {
    /// Non-fighter ships this seat may keep here (37.1).
    pub fleet_limit: i64,
    /// Ships charged against that limit, including the fighters the pool carries.
    pub fleet_charged: i64,
    /// Capacity the ships present provide (16.1).
    pub transport: i64,
    /// Capacity that fighters and ground forces in the space area consume, after a structure's fighter-only support has excused what it can.
    pub consumed: i64,
    /// Fighters the fleet pool carries instead of capacity (Fighter II).
    pub fighters_charged: i64,
    /// Capacity-consuming units that cannot legally remain here (16.3).
    pub capacity_excess: i64,
    /// Non-fighter ships here, before any fighter the pool carries (37.1).
    pub ships: i64,
    /// Capacity the ground forces in the space area consume (16.3).
    pub carried: i64,
    /// Capacity the fighters here would consume, before structure support excuses any.
    pub fighters: i64,
    /// Fighters that structures on planets here support without capacity (Space Dock II).
    pub fighter_support: i64,
    /// Whether a fighter here is upgraded, so the pool carries what capacity cannot (Fighter II).
    pub upgraded: bool,
    /// Whether the pool counts such a fighter as half a ship (Naalu Hybrid Crystal Fighter II).
    pub half_weight: bool,
}

impl Standing {
    /// Ships this seat could still add before 37.3 takes any off the board. Signed on purpose.
    #[must_use]
    pub const fn fleet_headroom(&self) -> i64 {
        self.fleet_limit - self.fleet_charged
    }

    /// Ships 37.3 would remove.
    #[must_use]
    pub const fn fleet_excess(&self) -> i64 {
        let over = self.fleet_charged - self.fleet_limit;
        if over < 0 { 0 } else { over }
    }

    /// Capacity still free. Never negative: what does not fit is [`Self::capacity_excess`].
    #[must_use]
    pub const fn capacity_free(&self) -> i64 {
        let free = self.transport - self.consumed;
        if free < 0 { 0 } else { free }
    }
}

/// Fleet supply and transport for one seat in one system, optionally with a unit not yet placed (the core's `query::fleet::standing`). All zero
/// when the core cannot hold the position.
#[must_use]
pub fn standing(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, system: &ti4_model::id::SystemId, arriving: Option<Arrival<'_>>) -> Standing {
    standing_in(state, content, sources, player, system, arriving).unwrap_or_default()
}

fn standing_in(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, system: &ti4_model::id::SystemId, arriving: Option<Arrival<'_>>) -> Option<Standing> {
    use ti4_engine_core::generated::{System, Unit};
    use ti4_engine_core::state::{Slice, SliceSet};
    let fleet_limit = if is_unlimited(state, player) { UNLIMITED_FLEET_BILL } else { i64::from(super::factions::fleet_limit(state, content, player)).max(0) };
    let view = super::view::Loaded::of(state, None, sources, SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Strategy]))?;
    let (seat, sys) = (view.seat(player)?, crate::ties::core_adapter::typed::<System>(system.as_str())?);
    let arrival = match arriving {
        None => None,
        Some(a) => Some(ti4_engine_core::query::fleet::Arrival { kind: crate::ties::core_adapter::typed::<Unit>(a.kind.id())?, count: a.count, in_space: a.in_space }),
    };
    let (core, k) = (view.core(), view.kernel());
    let half = |u: Unit| k.hooks.combat.fighter_fleet_weight_halves(core, k, seat, u);
    let s = ti4_engine_core::query::fleet::standing(core, view.scope(), seat, sys, fleet_limit, arrival, &half);
    Some(Standing {
        fleet_limit: s.fleet_limit,
        fleet_charged: s.fleet_charged,
        transport: s.transport,
        consumed: s.consumed,
        fighters_charged: s.fighters_charged,
        capacity_excess: s.capacity_excess,
        ships: s.ships,
        carried: s.carried,
        fighters: s.fighters,
        fighter_support: s.fighter_support,
        upgraded: s.upgraded,
        half_weight: s.half_weight,
    })
}

/// Ships beyond the fleet limit in this system, if any (37.3).
#[must_use]
pub fn over_supply(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, system: &ti4_model::id::SystemId) -> usize {
    usize::try_from(standing(state, content, sources, player, system, None).fleet_excess()).unwrap_or(0)
}

/// Capacity-consuming units that cannot legally remain in this space area (16.3).
#[must_use]
pub fn over_capacity(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, system: &ti4_model::id::SystemId) -> usize {
    usize::try_from(standing(state, content, sources, player, system, None).capacity_excess).unwrap_or(0)
}
