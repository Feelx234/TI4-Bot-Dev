//! The routes a ship may take to the active system (`tactical::ship_routes`, rules version 29): the default first, at most [`MAX_ROUTES`].
//!
//! Route 0 is the route `MovementRules::path_from_ship` finds for the option's own boosts, which is the route the engine took before version
//! 29; so an answer that names no route gets what it always got. The others are chosen from the legal routes
//! (`MovementRules::all_routes_from_ship`) for what they do differently:
//!
//! 1. the default;
//! 2. a route that costs a boost (Gravity Drive, the Ionian Fuel Refinery, not yet spent) and leaves fewer gravity rifts than every route
//!    without one -- the safe way round;
//! 3. the shortest route of each other combination of rift exits, wormhole hops, anomalies passed and systems to pick up from, plain routes
//!    before boosted ones;
//! 4. the remaining plain routes, fewest systems first, each with a set of systems not yet shown.
//!
//! Ties are broken by the ids of the systems, so the list is the same wherever it is asked.

use crate::generated::{System, Unit};
use crate::ids::Seat;
use crate::kernel::{fx_move, Kernel};
use crate::query::reach::{self, scope_of, SeatReach};
use crate::query::transit;
use crate::state::CoreState;

/// How many routes one ship is offered at most (the default included).
pub const MAX_ROUTES: usize = 5;

/// One way for a ship to get to the active system, with what it does on the way (`tactical::MoveRoute`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// The systems the ship enters, from its origin to the active system, both included.
    pub path: Vec<System>,
    /// The route the engine takes when the answer names none (route 0, always).
    pub default: bool,
    /// Taking this route spends Gravity Drive / exhausts the Ionian Fuel Refinery.
    pub gd: bool,
    pub ionian: bool,
    /// The gravity rifts the ship leaves on this route; it rolls one die for each.
    pub rifts: Vec<System>,
    /// Hops through a wormhole instead of across a border.
    pub wormholes: u32,
    /// Systems between origin and the active system that are anomalies.
    pub anomalies: Vec<System>,
    /// Systems between origin and the active system where a transport could pick up units.
    pub pickups: Vec<System>,
}

fn profile(route: &Route) -> (usize, u32, Vec<System>, Vec<System>) {
    let sorted = |list: &[System]| {
        let mut ids = list.to_vec();
        // The string sorts the ids as text; the generated enums are in text order.
        ids.sort_unstable();
        ids
    };
    (route.rifts.len(), route.wormholes, sorted(&route.anomalies), sorted(&route.pickups))
}

fn system_set(route: &Route) -> Vec<System> {
    let mut set = route.path.clone();
    set.sort_unstable();
    set.dedup();
    set
}

/// The routes of the ship at `index` among the seat's hulls in `origin` towards the active system, for the option's boosts `base`.
/// Empty when the map slice holds no galaxy, nothing is active, or the ship has no route at all.
#[must_use]
pub fn ship_routes(core: &CoreState, k: &Kernel, seat: Seat, origin: System, index: u16, kind: Unit, base: (bool, bool)) -> Vec<Route> {
    let Some(active) = core.meta().active_system else { return Vec::new() };
    let Some(reach) = SeatReach::new(core, k, seat, true) else { return Vec::new() };
    let rules = reach.rules_for(core, k, active);
    let value = |boosts: (bool, bool)| reach::effective_move_value_for_ship(core, k, seat, origin, kind, index, boosts);
    let Some(default_path) = value(base).and_then(|v| rules.path_from_ship(origin, v, Some(kind))) else { return Vec::new() };

    let gravity = fx_move::gravity_drive_available(core, seat);
    let ionian = fx_move::ionian_available(core, seat);
    let mut tiers = vec![base];
    if !base.0 && gravity {
        tiers.push((true, base.1));
    }
    if !base.1 && ionian {
        tiers.push((base.0, true));
    }

    let rolls = !rules.anomalies_ignored && !rules.rifts_ignored;
    let carries = scope_of(k).unit(kind).is_some_and(|d| d.capacity.unwrap_or(0) > 0);
    let mut pickup_cache: Vec<(System, bool)> = Vec::new();
    let mut describe = |path: Vec<System>, boosts: (bool, bool)| -> Route {
        let between: Vec<System> = if path.len() > 2 { path[1..path.len() - 1].to_vec() } else { Vec::new() };
        let pickups = if carries {
            between
                .iter()
                .copied()
                .filter(|system| match pickup_cache.iter().find(|(s, _)| s == system) {
                    Some((_, found)) => *found,
                    None => {
                        let found = !transit::loadable(core, k, seat, *system, false).is_empty();
                        pickup_cache.push((*system, found));
                        found
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        let rifts = if rolls { path[..path.len().saturating_sub(1)].iter().copied().filter(|s| rules.is_gravity_rift(*s) && !rules.rift_roll_exempt.contains(s.index())).collect() } else { Vec::new() };
        Route {
            default: false,
            gd: boosts.0,
            ionian: boosts.1,
            rifts,
            wormholes: transit::wormhole_hops(core, seat, origin, &path).unwrap_or(0),
            anomalies: between.iter().copied().filter(|s| rules.is_anomalous(*s)).collect(),
            pickups,
            path,
        }
    };

    let default = {
        let mut route = describe(default_path.clone(), base);
        route.default = true;
        route
    };
    let mut plain: Vec<Route> = Vec::new();
    let mut boosted: Vec<Route> = Vec::new();
    let mut seen: Vec<Vec<System>> = vec![default_path];
    for (tier, boosts) in tiers.iter().enumerate() {
        let Some(v) = value(*boosts) else { continue };
        for path in rules.all_routes_from_ship(origin, v, Some(kind)) {
            if seen.contains(&path) {
                continue;
            }
            seen.push(path.clone());
            let route = describe(path, *boosts);
            if tier == 0 {
                plain.push(route);
            } else {
                boosted.push(route);
            }
        }
    }

    let mut chosen = vec![default];
    let new_system_set = |chosen: &[Route], route: &Route| !chosen.iter().any(|c| system_set(c) == system_set(route));
    let new_profile = |chosen: &[Route], route: &Route| !chosen.iter().any(|c| profile(c) == profile(route));
    // 2. The safe way round, for a boost.
    let fewest_plain = plain.iter().map(|r| r.rifts.len()).chain(std::iter::once(chosen[0].rifts.len())).min().unwrap_or(0);
    if let Some(route) = boosted.iter().find(|r| r.rifts.len() < fewest_plain) {
        chosen.push(route.clone());
    }
    // 3. One route of each kind of consequence.
    for route in plain.iter().chain(boosted.iter()) {
        if chosen.len() >= MAX_ROUTES {
            break;
        }
        if new_profile(&chosen, route) && new_system_set(&chosen, route) {
            chosen.push(route.clone());
        }
    }
    // 4. Fill up with other plain routes.
    for route in &plain {
        if chosen.len() >= MAX_ROUTES {
            break;
        }
        if new_system_set(&chosen, route) && !chosen.contains(route) {
            chosen.push(route.clone());
        }
    }
    chosen
}
