//! Movement facts for the product (copy and typed rewrite of `tactical.rs` of the legacy crate): the ships that can reach a system, the routes a
//! ship may take, the rifts it leaves, and the result types. Every answer comes from the typed core (`query::reach`, `rules::tactical::routes`);
//! a position or id the core cannot hold answers as the legacy answers when it has no typed path and no string fallback applies: nothing.
//!
//! `ship_routes(.., rules: &MovementRules, ..)` and `rifts_exited(rules, ..)` of the legacy crate take the string `MovementRules`; they are not
//! copied (the host builds the rules only to call them). [`routes_into`] and [`rift_exits`] are their position-level replacements and answer
//! the same (`tests/surface_parity.rs`).

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::generated::System;
use ti4_engine_core::query::reach as core_reach;
use ti4_engine_core::rules::tactical::routes as core_routes;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;
use ti4_model::units::Unit;

use std::rc::Rc;

use super::adapter::{Position, SeatCache};
use super::adapter::unit_ix;
use crate::ties::core_adapter::{system_id, system_ix};
use crate::wire::choice::Choice;

pub use crate::kinds::tactical::{ACTIVATE_KIND, MAX_ROUTES, MOVE_KIND, ROUTES_DETAIL};

/// One ship that could legally move into the active system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Movable {
    pub origin: SystemId,
    /// Index into `state.ships_of(player, origin)`, so identical ships stay distinguishable.
    pub index: usize,
    pub unit: Unit,
    /// Printed capacity, carried because it is part of the oracle's learned option payload.
    pub capacity: i64,
    /// This otherwise-unreachable move spends Gravity Drive's +1 for the activation.
    pub gravity_drive: bool,
    /// This otherwise-unreachable move exhausts the Ionian Fuel Refinery for its +1.
    pub ionian: bool,
}

impl Movable {
    /// The string-side form of a typed answer: the hull is the one at `index` of the player's ships in `origin`. `None` when the state does
    /// not have it.
    #[must_use]
    pub fn from_typed(state: &GameState, player: &PlayerId, found: &core_reach::Movable) -> Option<Self> {
        let origin = SystemId::new(system_id(found.origin));
        let hull = (*state.ships_of(player, &origin).get(usize::from(found.index))?).clone();
        Some(Self { origin, index: usize::from(found.index), unit: hull, capacity: found.capacity, gravity_drive: found.gravity_drive, ionian: found.ionian })
    }
}

/// One way for a ship to get to the active system, with what it does on the way.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MoveRoute {
    /// The systems the ship enters, from its origin to the active system, both included.
    pub path: Vec<String>,
    /// The id that answers with this route: the option's id for the default route, else the id with `|r<k>` (see [`route_suffix`]).
    pub answer: String,
    /// The route the engine takes when the answer names none (route 0, always).
    pub default: bool,
    /// Taking this route spends Gravity Drive.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gravity_drive: bool,
    /// Taking this route exhausts the Ionian Fuel Refinery.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ionian: bool,
    /// The gravity rifts the ship leaves on this route; it rolls one die for each.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rifts: Vec<String>,
    /// Hops through a wormhole instead of across a border.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub wormholes: u32,
    /// Systems between origin and the active system that are anomalies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anomalies: Vec<String>,
    /// Systems between origin and the active system where a transport could pick up units.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pickups: Vec<String>,
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde's signature")]
const fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

/// Split a route answer into the id of the option and the number of the route: `move|206|0|r2` is route 2 of `move|206|0`.
#[must_use]
pub fn route_suffix(id: &str) -> Option<(&str, usize)> {
    let (base, tail) = id.rsplit_once("|r")?;
    let number: usize = tail.parse().ok()?;
    (number > 0 && tail == number.to_string()).then_some((base, number))
}

/// The routes that a choice lists for one of its options (empty: one route only, the default).
#[must_use]
pub fn routes_of(choice: &Choice, option_id: &str) -> Vec<MoveRoute> {
    choice
        .details
        .get(ROUTES_DETAIL)
        .and_then(|routes| routes.get(option_id))
        .and_then(|list| serde_json::from_value(list.clone()).ok())
        .unwrap_or_default()
}

/// How many routes the choice lists for `option_id`.
#[must_use]
pub fn route_count(choice: &Choice, option_id: &str) -> usize {
    choice.details.get(ROUTES_DETAIL).and_then(|routes| routes.get(option_id)).and_then(serde_json::Value::as_array).map_or(0, Vec::len)
}

/// Every ship of `player` that could reach `active` if it were activated (`tactical::movable_into`, on the typed core). Nothing when the
/// position cannot be held by the core.
#[must_use]
pub fn movable_into(state: &GameState, _content: &ContentStore, sources: SourceSet, galaxy: &Galaxy, player: &PlayerId, active: &SystemId) -> Vec<Movable> {
    let Some(pos) = Position::new(state, sources, galaxy) else { return Vec::new() };
    typed_movable_into(&mut SeatCache::new(), &pos, state, player, active).unwrap_or_default()
}

/// [`movable_into`] over a position and caches the caller keeps (`Observed::typed_movable_into`); `None` when something is not mapped.
#[must_use]
pub fn typed_movable_into(cache: &mut SeatCache, pos: &Rc<Position>, state: &GameState, player: &PlayerId, destination: &SystemId) -> Option<Vec<Movable>> {
    let to = system_ix(destination.as_str())?;
    let seat = pos.seat(player)?;
    let rules = cache.reach(pos, player, true)?.rules_for(to);
    let fleet = cache.fleet(pos, seat);
    fleet.movable(&rules).into_iter().map(|m| Movable::from_typed(state, player, &m)).collect::<Option<Vec<_>>>()
}

/// Gravity rifts a ship of `unit_type` sailing from `origin` to `destination` would exit (`Observed::typed_rift_exits`); `None` when something
/// is not mapped. A type outside the scope has no kind row: no move, no rifts.
#[must_use]
pub fn typed_rift_exits(
    cache: &mut SeatCache,
    pos: &Rc<Position>,
    player: &PlayerId,
    origin: &SystemId,
    unit_type: &str,
    boosts: (bool, bool),
    destination: &SystemId,
) -> Option<usize> {
    let kind = unit_ix(unit_type)?;
    let (from, to) = (system_ix(origin.as_str())?, system_ix(destination.as_str())?);
    let seat = pos.seat(player)?;
    let Some(value) = core_reach::effective_move_value(pos.core(), pos.kernel(), seat, from, kind, boosts) else {
        return Some(0);
    };
    let rules = cache.reach(pos, player, true)?.rules_for(to);
    Some(if rules.anomalies_ignored || rules.rifts_ignored { 0 } else { rules.path_from_ship(from, value, Some(kind)).map_or(0, |path| rules.rifts_exited(&path)) })
}

/// `Observed::can_reach` on the typed core: laws only, no cards in hand; `None` when something is not mapped.
#[must_use]
pub fn typed_can_reach(cache: &mut SeatCache, pos: &Rc<Position>, player: &PlayerId, origin: &SystemId, destination: &SystemId, move_value: i32) -> Option<bool> {
    let to = system_ix(destination.as_str())?;
    let rules = cache.reach(pos, player, false)?.rules_for(to);
    let from = system_ix(origin.as_str())?;
    Some(rules.can_reach_ship(from, move_value, None))
}

/// The routes the ship at `index` of `player`'s ships in `origin` may take to `active` (`tactical::routes_into`): the default first, at most
/// [`MAX_ROUTES`]; the option id of the ship's move is derived from the boosts. Empty without a route.
#[must_use]
#[expect(clippy::too_many_arguments, reason = "the legacy signature, kept for the call sites")]
pub fn routes_into(
    state: &GameState,
    _content: &ContentStore,
    sources: SourceSet,
    galaxy: &Galaxy,
    player: &PlayerId,
    active: &SystemId,
    origin: &SystemId,
    index: usize,
    boosts: (bool, bool),
) -> Vec<MoveRoute> {
    let Some(hull) = state.ships_of(player, origin).get(index).copied() else { return Vec::new() };
    let id = format!(
        "{}|{origin}|{index}",
        match boosts {
            (false, false) => "move",
            (true, false) => "move_gd",
            (false, true) => "move_ion",
            (true, true) => "move_gd_ion",
        }
    );
    routes_for(state, sources, galaxy, player, active, origin, index, hull.type_id.as_str(), boosts, &id)
}

/// The routes of one ship under an option id (`tactical::ship_routes` with the rules of the typed core).
#[must_use]
#[expect(clippy::too_many_arguments, reason = "the movement search's own inputs")]
pub fn routes_for(
    state: &GameState,
    sources: SourceSet,
    galaxy: &Galaxy,
    player: &PlayerId,
    active: &SystemId,
    origin: &SystemId,
    index: usize,
    hull_type: &str,
    base: (bool, bool),
    option_id: &str,
) -> Vec<MoveRoute> {
    let Some(mut pos) = Position::new_owned(state, sources, galaxy) else { return Vec::new() };
    let (Some(seat), Some(to), Some(from), Some(kind)) = (pos.seat(player), system_ix(active.as_str()), system_ix(origin.as_str()), unit_ix(hull_type)) else {
        return Vec::new();
    };
    pos.set_active_system(to);
    let Ok(index16) = u16::try_from(index) else { return Vec::new() };
    let found = core_routes::ship_routes(pos.core(), pos.kernel(), seat, from, index16, kind, base);
    let text = |systems: &[System]| systems.iter().map(|s| system_id(*s).to_owned()).collect::<Vec<_>>();
    found
        .iter()
        .enumerate()
        .map(|(number, route)| MoveRoute {
            path: text(&route.path),
            answer: if number == 0 { option_id.to_owned() } else { format!("{option_id}|r{number}") },
            default: route.default,
            gravity_drive: route.gd,
            ionian: route.ionian,
            rifts: text(&route.rifts),
            wormholes: route.wormholes,
            anomalies: text(&route.anomalies),
            pickups: text(&route.pickups),
        })
        .collect()
}

/// What a ship of this type moves from `origin` (the hull at `index` of the seat's units there), with the cards in play, the modules' move
/// bonuses (Creuss Slipstream), Gravleash's anchor and the two boosts: the core's `query::reach::effective_move_value_for_ship` over the
/// kernel with every faction module. Without an `index`, or when the core cannot hold the position or the ship, the answer is the printed move
/// with the boosts only (the cards and the modules are the core's to add).
#[must_use]
pub fn effective_move_value_for_ship(
    state: &GameState,
    kind: &ti4_content::units::UnitType<'_>,
    player: &PlayerId,
    origin: &SystemId,
    index: Option<usize>,
    gravity_drive: bool,
    ionian: bool,
) -> i32 {
    use ti4_engine_core::generated::{System, Unit};
    use ti4_engine_core::state::{Slice, SliceSet};
    let printed = i32::try_from(kind.move_value()).unwrap_or(0) + i32::from(gravity_drive) + i32::from(ionian);
    let typed = || {
        let view = super::view::Loaded::of(state, None, ti4_model::content_types::FULL, SliceSet::of_without(SliceSet::ALL, Slice::Map))?;
        let seat = view.seat(player)?;
        let hull = crate::ties::core_adapter::typed::<Unit>(kind.id())?;
        let system = crate::ties::core_adapter::typed::<System>(origin.as_str())?;
        let index = u16::try_from(index?).ok()?;
        ti4_engine_core::query::reach::effective_move_value_for_ship(view.core(), view.kernel(), seat, system, hull, index, (gravity_drive, ionian))
    };
    typed().unwrap_or(printed)
}
