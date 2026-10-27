//! Movement reachability, typed: the route search of `ti4_engine::movement::MovementRules`.
//!
//! The same LRR rules (58.4a-f, 11.1, 86.1, 59.1-2, 41.1/41.3) and the same modifiers, one field
//! each, with the same meaning; ids are indices and sets are bitsets. The search is the same
//! breadth-first search with the same dominance rule and the same neighbour order (index order =
//! text order), so the route it returns is the route the string search returns, not just a route
//! of the same length. Tested against it in `ti4-engine-core-bridge`.
//!
//! Who decides the modifiers (laws, technologies, action cards, faction hooks) is not here: the
//! adapter copies them from a built `MovementRules`, or [`CoreRules::plain`] sets the board facts for
//! a mover that has none of them.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use crate::bits::{SysSet, UnitSet};
use crate::board::CoreBoard;
use crate::galaxy::Adjacency;
use crate::ids::{Seat, SystemIx, UnitIx};
use crate::generated::SystemData;
use crate::tables::{Flags, SYSTEM_COUNT, Scope, SystemFlags};

/// `movement::ROUTE_SEARCH_NODES`.
pub const ROUTE_SEARCH_NODES: usize = 20_000;
/// `movement::ROUTE_SEARCH_ROUTES`.
pub const ROUTE_SEARCH_ROUTES: usize = 512;

/// Reachability for one mover towards one active system. Field for field the string rules.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreRules {
    pub adjacency: Arc<Adjacency>,
    /// Systems of the corpus in the game's source scope (`systems_index`).
    pub scope: Scope,
    pub active: SystemIx,
    pub enemy_ships: SysSet,
    pub own_command_tokens: SysSet,
    pub nebulae_open: bool,
    pub asteroid_fields_open: bool,
    pub supernovae_open: bool,
    pub supernovae_pass_through: bool,
    pub command_tokens_ignored: bool,
    pub anomalies_ignored: bool,
    pub ignore_enemy_ships_from: Option<SystemIx>,
    pub ignore_enemy_ships: bool,
    pub also_adjacent: SysSet,
    pub extra_adjacency: Vec<(SystemIx, SysSet)>,
    pub token_wormhole_systems: SysSet,
    pub barred_transit: SysSet,
    pub gravity_rift_systems: SysSet,
    pub rift_extra_steps: i32,
    /// `MovementRules::rifts_ignored` (Circlet of the Void, Crucible): no roll for any rift.
    pub rifts_ignored: bool,
    /// `MovementRules::rift_roll_exempt`: created rifts this mover does not roll for.
    pub rift_roll_exempt: SysSet,
    pub passing_ship_types: UnitSet,
    pub own_passage_types: UnitSet,
    pub own_unit_systems: SysSet,
    pub ship_adjacent: Vec<(UnitIx, SysSet)>,
    pub token_free_types: UnitSet,
    pub nebulae_ignored: bool,
    /// Per system, the systems a blocked border separates it from (both directions).
    pub blocked_edges: Vec<(SystemIx, SysSet)>,
    pub unladen_pass: bool,
    /// Search buffers, shared by every copy of these rules (see [`Scratch`]).
    pub scratch: ScratchCell,
}

fn sparse_get<K: PartialEq + Copy>(list: &[(K, SysSet)], key: K) -> Option<&SysSet> {
    list.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

type Frontier = (SystemIx, i32, i32, u32, bool, SysSet);

/// The buffers of a route search, kept between searches so a query allocates nothing but its answer:
/// the dominance array, the trail arena (system, parent) of queued routes, the queue, and the sorted
/// table of the Luminous-step search.
#[derive(Debug, Default)]
pub struct Scratch {
    flat: Vec<i32>,
    nodes: Vec<(SystemIx, u32)>,
    queue: VecDeque<Frontier>,
    /// `((system, bonus used, earned systems), best remaining)`, sorted by key.
    keyed: Vec<((SystemIx, bool, SysSet), i32)>,
}

impl Scratch {
    fn route(&self, mut at: u32) -> Vec<SystemIx> {
        let mut out = Vec::new();
        loop {
            let (system, parent) = self.nodes[at as usize];
            out.push(system);
            if parent == u32::MAX {
                break;
            }
            at = parent;
        }
        out.reverse();
        out
    }
}

/// A shared [`Scratch`]. All copies of one set of rules (one per destination of a seat) use the same
/// buffers; equality ignores it, a clone shares it.
#[derive(Debug, Clone, Default)]
pub struct ScratchCell(Rc<RefCell<Scratch>>);

impl PartialEq for ScratchCell {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl CoreRules {
    /// Rules with only the board facts set: what `MovementRules::new` builds (no laws, no hooks),
    /// plus whatever the caller sets on the public flags afterwards.
    #[must_use]
    pub fn plain(
        adjacency: Arc<Adjacency>,
        scope: Scope,
        active: SystemIx,
        board: &CoreBoard,
        mover: Seat,
    ) -> Self {
        Self::with_board(
            adjacency,
            scope,
            active,
            board.enemy_ships(mover),
            board.own_command_tokens(mover),
        )
    }

    /// [`Self::plain`] from already computed board facts (one per mover, shared by destinations).
    #[must_use]
    pub fn with_board(
        adjacency: Arc<Adjacency>,
        scope: Scope,
        active: SystemIx,
        enemy_ships: SysSet,
        own_command_tokens: SysSet,
    ) -> Self {
        Self {
            adjacency,
            scope,
            active,
            enemy_ships,
            own_command_tokens,
            nebulae_open: false,
            asteroid_fields_open: false,
            supernovae_open: false,
            supernovae_pass_through: false,
            command_tokens_ignored: false,
            anomalies_ignored: false,
            ignore_enemy_ships_from: None,
            ignore_enemy_ships: false,
            also_adjacent: SysSet::EMPTY,
            extra_adjacency: Vec::new(),
            token_wormhole_systems: SysSet::EMPTY,
            barred_transit: SysSet::EMPTY,
            gravity_rift_systems: SysSet::EMPTY,
            rift_extra_steps: 0,
            rifts_ignored: false,
            rift_roll_exempt: SysSet::EMPTY,
            passing_ship_types: UnitSet::EMPTY,
            own_passage_types: UnitSet::EMPTY,
            own_unit_systems: SysSet::EMPTY,
            ship_adjacent: Vec::new(),
            token_free_types: UnitSet::EMPTY,
            nebulae_ignored: false,
            blocked_edges: Vec::new(),
            unladen_pass: false,
            scratch: ScratchCell::default(),
        }
    }

    #[inline]
    fn system(&self, system: SystemIx) -> Option<&'static SystemData> {
        self.scope.system(system)
    }

    #[inline]
    fn has(&self, system: SystemIx, flag: u8) -> bool {
        self.system(system).is_some_and(|s| s.has(flag))
    }

    #[must_use]
    pub fn is_gravity_rift(&self, system: SystemIx) -> bool {
        self.has(system, SystemFlags::GRAVITY_RIFT)
            || self.gravity_rift_systems.contains(system.index())
    }

    const fn nebulae_are_open(&self) -> bool {
        self.nebulae_open || self.anomalies_ignored || self.nebulae_ignored
    }

    /// `transit::rifts_exited(..).len()`: the rifts a route leaves (every system but the last) that
    /// this mover rolls for.
    #[must_use]
    pub fn rifts_exited(&self, path: &[SystemIx]) -> usize {
        let Some((_, before)) = path.split_last() else {
            return 0;
        };
        before
            .iter()
            .filter(|s| self.is_gravity_rift(**s) && !self.rift_roll_exempt.contains(s.index()))
            .count()
    }

    #[must_use]
    pub fn can_enter(&self, system: SystemIx) -> bool {
        self.enterable(system, false)
    }

    fn enterable(&self, system: SystemIx, passing: bool) -> bool {
        if !passing && self.adjacency.is_hyperlane(system) {
            return false;
        }
        if self.anomalies_ignored {
            return true;
        }
        let Some(data) = self.system(system) else {
            return false;
        };
        if (data.has(SystemFlags::SUPERNOVA)
            && !self.supernovae_open
            && !(passing && self.supernovae_pass_through))
            || (data.has(SystemFlags::ASTEROID_FIELD) && !self.asteroid_fields_open)
        {
            return false;
        }
        if data.has(SystemFlags::NEBULA) && system != self.active {
            return self.nebulae_are_open();
        }
        true
    }

    fn passes(
        &self,
        system: SystemIx,
        origin: Option<SystemIx>,
        ship_type: Option<UnitIx>,
        past_ships: bool,
    ) -> bool {
        if !self.enterable(system, true) {
            return false;
        }
        if self.has(system, SystemFlags::NEBULA) && !self.nebulae_are_open() {
            return false;
        }
        if self.barred_transit.contains(system.index()) {
            return false;
        }
        if self.ignore_enemy_ships || past_ships {
            return true;
        }
        if ship_type.is_some_and(|k| self.passing_ship_types.contains(k.index())) {
            return true;
        }
        if ship_type.is_some_and(|k| self.own_passage_types.contains(k.index()))
            && self.own_unit_systems.contains(system.index())
        {
            return true;
        }
        if origin.is_some() && origin == self.ignore_enemy_ships_from {
            return true;
        }
        !self.enemy_ships.contains(system.index())
    }

    #[must_use]
    pub fn can_pass_through_ship(
        &self,
        system: SystemIx,
        origin: Option<SystemIx>,
        ship_type: Option<UnitIx>,
    ) -> bool {
        self.passes(system, origin, ship_type, false)
    }

    #[must_use]
    pub fn may_depart(&self, origin: SystemIx) -> bool {
        if origin == self.active || self.command_tokens_ignored {
            return true;
        }
        !self.own_command_tokens.contains(origin.index())
    }

    fn may_depart_ship(&self, origin: SystemIx, ship_type: Option<UnitIx>) -> bool {
        self.may_depart(origin)
            || ship_type.is_some_and(|k| self.token_free_types.contains(k.index()))
    }

    #[must_use]
    pub fn nebula_caps(&self, origin: SystemIx) -> bool {
        self.has(origin, SystemFlags::NEBULA) && !self.anomalies_ignored && !self.nebulae_ignored
    }

    #[must_use]
    pub fn can_reach_ship(&self, origin: SystemIx, move_value: i32, ship_type: Option<UnitIx>) -> bool {
        self.path_from_ship(origin, move_value, ship_type).is_some()
    }

    /// `MovementRules::path_from_ship`: the route, origin first, active system last.
    #[must_use]
    pub fn path_from_ship(
        &self,
        origin: SystemIx,
        move_value: i32,
        ship_type: Option<UnitIx>,
    ) -> Option<Vec<SystemIx>> {
        self.search_path(origin, move_value, ship_type, false)
            .or_else(|| {
                self.unladen_pass
                    .then(|| self.search_path(origin, move_value, ship_type, true))
                    .flatten()
            })
    }

    /// `MovementRules::step_neighbours`.
    fn step_neighbours(&self, current: SystemIx, ship_type: Option<UnitIx>) -> SysSet {
        let mut n = *self.adjacency.of(current);
        if self.token_wormhole_systems.contains(current.index()) {
            n.union_with(&self.token_wormhole_systems);
            // The token set minus `current`; `current` itself was not a neighbour before unless the
            // map said so, which it never does (adjacency excludes the system itself).
            if !self.adjacency.of(current).contains(current.index()) {
                n.remove(current.index());
            }
        }
        if let Some(extra) = sparse_get(&self.extra_adjacency, current) {
            n.union_with(extra);
        }
        if let Some(near) = ship_type.and_then(|k| sparse_get(&self.ship_adjacent, k)) {
            let had_self = n.contains(current.index());
            n.union_with(near);
            if !had_self {
                n.remove(current.index());
            }
        }
        if self.also_adjacent.contains(current.index()) {
            n.insert(self.active.index());
        } else if current == self.active {
            n.union_with(&self.also_adjacent);
        }
        if let Some(blocked) = sparse_get(&self.blocked_edges, current) {
            for other in blocked.iter() {
                n.remove(other);
            }
        }
        n
    }

    fn search_path(
        &self,
        origin: SystemIx,
        move_value: i32,
        ship_type: Option<UnitIx>,
        past_ships: bool,
    ) -> Option<Vec<SystemIx>> {
        if !self.may_depart_ship(origin, ship_type) {
            return None;
        }
        let budget = if self.nebula_caps(origin) { 1 } else { move_value };
        if budget <= 0 {
            return None;
        }
        let earns = ship_type.is_some_and(|k| self.own_passage_types.contains(k.index()));
        // Dominance as in the string search: per (system, bonus used, earned systems) the best
        // remaining budget seen. Without the Luminous step the earned set is always empty, so the
        // key is (system, bonus) and a flat array does.
        let mut guard = self.scratch.0.borrow_mut();
        let Scratch { flat, nodes, queue, keyed } = &mut *guard;
        flat.clear();
        keyed.clear();
        nodes.clear();
        queue.clear();
        if !earns {
            flat.resize(2 * SYSTEM_COUNT, i32::MIN);
        }
        nodes.push((origin, u32::MAX));
        queue.push_back((origin, 0, budget, 0, false, SysSet::EMPTY));
        let mut found = None;
        'search: while let Some((current, entered, mut allowance, at, mut bonus_used, mut earned)) =
            queue.pop_front()
        {
            if self.is_gravity_rift(current) && !self.anomalies_ignored {
                allowance += 1;
                if !bonus_used && self.rift_extra_steps > 0 {
                    allowance += self.rift_extra_steps;
                    bonus_used = true;
                }
            }
            if entered > 0
                && earns
                && self.own_unit_systems.contains(current.index())
                && earned.insert(current.index())
            {
                allowance += 1;
            }
            let remaining = allowance - entered;
            if earns {
                let key = (current, bonus_used, earned);
                match keyed.binary_search_by(|(k, _)| k.cmp(&key)) {
                    Ok(at) => {
                        if keyed[at].1 >= remaining {
                            continue;
                        }
                        keyed[at].1 = remaining;
                    }
                    Err(at) => keyed.insert(at, (key, remaining)),
                }
            } else {
                let slot = &mut flat[current.index() * 2 + usize::from(bonus_used)];
                if *slot >= remaining {
                    continue;
                }
                *slot = remaining;
            }
            if remaining <= 0 {
                continue;
            }
            for n in self.step_neighbours(current, ship_type).iter() {
                let neighbour = crate::ids::system_at(n);
                let ends_here = neighbour == self.active;
                if ends_here && !self.can_enter(neighbour) {
                    continue;
                }
                nodes.push((neighbour, at));
                let here = (nodes.len() - 1) as u32;
                if ends_here {
                    found = Some(here);
                    break 'search;
                }
                if !self.passes(neighbour, Some(origin), ship_type, past_ships) {
                    continue;
                }
                queue.push_back((neighbour, entered + 1, allowance, here, bonus_used, earned));
            }
        }
        found.map(|here| guard.route(here))
    }

    /// `MovementRules::all_routes_from_ship`: every simple route within the budget, sorted by
    /// length then by ids, bounded the same way.
    #[must_use]
    pub fn all_routes_from_ship(
        &self,
        origin: SystemIx,
        move_value: i32,
        ship_type: Option<UnitIx>,
    ) -> Vec<Vec<SystemIx>> {
        let past_ships =
            self.unladen_pass && self.search_path(origin, move_value, ship_type, false).is_none();
        if !self.may_depart_ship(origin, ship_type) {
            return Vec::new();
        }
        let budget = if self.nebula_caps(origin) { 1 } else { move_value };
        if budget <= 0 {
            return Vec::new();
        }
        let mut walk = Walk {
            origin,
            ship_type,
            past_ships,
            found: Vec::new(),
            nodes: 0,
        };
        let mut route = vec![origin];
        self.extend_routes(&mut walk, &mut route, 0, budget, false, SysSet::EMPTY);
        walk.found
            .sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
        walk.found
    }

    fn extend_routes(
        &self,
        walk: &mut Walk,
        route: &mut Vec<SystemIx>,
        entered: i32,
        mut allowance: i32,
        mut bonus_used: bool,
        mut earned: SysSet,
    ) {
        walk.nodes += 1;
        if walk.nodes > ROUTE_SEARCH_NODES || walk.found.len() >= ROUTE_SEARCH_ROUTES {
            return;
        }
        let current = *route.last().expect("a route starts at its origin");
        if self.is_gravity_rift(current) && !self.anomalies_ignored {
            allowance += 1;
            if !bonus_used && self.rift_extra_steps > 0 {
                allowance += self.rift_extra_steps;
                bonus_used = true;
            }
        }
        if entered > 0
            && walk
                .ship_type
                .is_some_and(|k| self.own_passage_types.contains(k.index()))
            && self.own_unit_systems.contains(current.index())
            && earned.insert(current.index())
        {
            allowance += 1;
        }
        if allowance - entered <= 0 {
            return;
        }
        for n in self.step_neighbours(current, walk.ship_type).iter() {
            let neighbour = crate::ids::system_at(n);
            if route.contains(&neighbour) {
                continue;
            }
            if neighbour == self.active {
                if self.can_enter(neighbour) {
                    let mut arrived = route.clone();
                    arrived.push(neighbour);
                    walk.found.push(arrived);
                }
                continue;
            }
            if !self.passes(neighbour, Some(walk.origin), walk.ship_type, walk.past_ships) {
                continue;
            }
            route.push(neighbour);
            self.extend_routes(walk, route, entered + 1, allowance, bonus_used, earned);
            route.pop();
        }
    }
}

struct Walk {
    origin: SystemIx,
    ship_type: Option<UnitIx>,
    past_ships: bool,
    found: Vec<Vec<SystemIx>>,
    nodes: usize,
}
