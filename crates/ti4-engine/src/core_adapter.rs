//! The string <-> typed boundary for the movement slice (docs/ENGINE_CORE.md, "Adapter").
//!
//! Prototype, behind the `typed-core` feature (off by default): with it, `tactical::movable_into`
//! answers its reach questions with the typed core (`ti4-engine-core`).
//!
//! Everything here converts: id text to an index (binary search in the generated, sorted name
//! tables) and back, a string `Galaxy` to a [`CoreGalaxy`], a `GameState` board to a [`CoreBoard`],
//! a built `MovementRules` to [`CoreRules`]. Nothing here decides a rule.
//!
//! An id the tables do not know (a store loaded with `from_dir` that adds systems, a generated
//! tile, an owner beyond 16 seats) is reported as [`Unmapped`], and the caller answers that query
//! with the string engine: the strangler pattern's escape hatch. A set entry that is only ever
//! tested for membership is dropped instead, which is exact as long as every system a search can
//! visit is mapped (an unmapped id can then never be asked about).

use std::collections::{BTreeMap, BTreeSet};

use strum::IntoEnumIterator;
use ti4_content::galaxy::Galaxy;
use crate::movement::MovementRules;
use ti4_engine_core::board::CoreBoard;
use ti4_engine_core::galaxy::{Adjacency, CoreGalaxy};
use ti4_engine_core::hex::Hex;
use ti4_engine_core::movement::CoreRules;
use ti4_engine_core::names::{SYSTEM_NAMES, UNIT_NAMES, WORMHOLE_KIND_NAMES};
use ti4_engine_core::tables::{Scope, UNITS};
use ti4_engine_core::{Seat, SysSet, SystemIx, UnitIx, UnitSet};
use ti4_model::content_types::{Source, SourceSet};
use ti4_model::state::GameState;

/// Something the typed core cannot express; the caller falls back to the string engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unmapped {
    System(String),
    Unit(String),
    WormholeKind(String),
    /// More than 16 owners at one table.
    Seats(usize),
    /// A hex outside the dense grid.
    OffGrid(String),
    Duplicate(String),
}

#[must_use]
pub fn system_ix(id: &str) -> Option<SystemIx> {
    SYSTEM_NAMES
        .binary_search(&id)
        .ok()
        .map(|i| SystemIx(i as u16))
}

#[must_use]
pub fn system_id(ix: SystemIx) -> &'static str {
    SYSTEM_NAMES[ix.index()]
}

#[must_use]
pub fn unit_ix(id: &str) -> Option<UnitIx> {
    UNIT_NAMES.binary_search(&id).ok().map(|i| UnitIx(i as u16))
}

#[must_use]
pub fn unit_id(ix: UnitIx) -> &'static str {
    UNIT_NAMES[ix.index()]
}

fn sys(id: &str) -> Result<SystemIx, Unmapped> {
    system_ix(id).ok_or_else(|| Unmapped::System(id.to_owned()))
}

/// The scope mask of a source set: bit `i` for the `i`-th `Source` variant, as `build.rs` tags
/// records.
#[must_use]
pub fn scope(sources: SourceSet) -> Scope {
    Scope(
        Source::iter()
            .enumerate()
            .filter(|(_, s)| sources.contains(*s))
            .fold(0, |mask, (i, _)| mask | (1 << i)),
    )
}

/// Wormhole kind text to bits.
///
/// # Errors
/// A kind the corpus does not print.
pub fn wormhole_mask<'k>(kinds: impl IntoIterator<Item = &'k str>) -> Result<u8, Unmapped> {
    let mut mask = 0;
    for kind in kinds {
        let bit = WORMHOLE_KIND_NAMES
            .binary_search(&kind)
            .map_err(|_| Unmapped::WormholeKind(kind.to_owned()))?;
        mask |= 1 << bit;
    }
    Ok(mask)
}

/// Systems as a bitset; an unmapped id is an error.
///
/// # Errors
/// An id the tables do not know.
pub fn sys_set<'s>(ids: impl IntoIterator<Item = &'s str>) -> Result<SysSet, Unmapped> {
    let mut set = SysSet::EMPTY;
    for id in ids {
        set.insert(sys(id)?.index());
    }
    Ok(set)
}

/// Systems as a bitset, dropping ids the tables do not know (for membership-only sets).
pub fn sys_set_lossy<'s>(ids: impl IntoIterator<Item = &'s str>) -> SysSet {
    let mut set = SysSet::EMPTY;
    for id in ids {
        if let Some(ix) = system_ix(id) {
            set.insert(ix.index());
        }
    }
    set
}

/// Unit types as a bitset, dropping unknown types (membership only; a query with an unknown type
/// is refused by [`ship_type`]).
pub fn unit_set_lossy<'s>(ids: impl IntoIterator<Item = &'s str>) -> UnitSet {
    let mut set = UnitSet::EMPTY;
    for id in ids {
        if let Some(ix) = unit_ix(id) {
            set.insert(ix.index());
        }
    }
    set
}

/// A query's ship type.
///
/// # Errors
/// A type the tables do not know.
pub fn ship_type(id: Option<&str>) -> Result<Option<UnitIx>, Unmapped> {
    id.map(|t| unit_ix(t).ok_or_else(|| Unmapped::Unit(t.to_owned())))
        .transpose()
}

/// The string galaxy as a typed one: placement, effective wormhole kinds (printed, token and
/// copy edits merged, Nexus Sovereignty applied), hyperlanes, the switches and ability links.
///
/// # Errors
/// An id or hex the core cannot hold.
pub fn galaxy_from_legacy(galaxy: &Galaxy) -> Result<CoreGalaxy, Unmapped> {
    let mut raw = CoreGalaxy::new();
    for id in galaxy.system_ids() {
        let ix = sys(id)?;
        let hex = galaxy
            .coord_of(id)
            .ok_or_else(|| Unmapped::System(id.to_owned()))?;
        raw.place(ix, Hex::new(hex.q, hex.r)).map_err(|e| match e {
            ti4_engine_core::galaxy::PlaceError::Duplicate(_) => Unmapped::Duplicate(id.to_owned()),
            ti4_engine_core::galaxy::PlaceError::OffGrid(_) => Unmapped::OffGrid(id.to_owned()),
        })?;
    }
    for id in galaxy.off_map_systems() {
        raw.place_off_map(sys(id)?)
            .map_err(|_| Unmapped::Duplicate(id.to_owned()))?;
    }
    // Kinds as the string galaxy reports them: that already merges what the raw galaxy would
    // otherwise split (printed, token, a copy's retained or suppressed kinds, the Nexus switch).
    let in_play: Vec<SystemIx> = raw.in_play().iter().map(|i| SystemIx(i as u16)).collect();
    for ix in in_play {
        raw.set_wormholes(ix, wormhole_mask(galaxy.wormhole_kinds(system_id(ix)))?);
    }
    for id in galaxy.token_wormholes.keys() {
        let ix = sys(id)?;
        if !raw.in_play().contains(ix.index()) {
            raw.set_token_wormholes(ix, wormhole_mask(galaxy.wormhole_kinds(id))?);
        }
    }
    let hyperlanes: BTreeSet<&str> = galaxy.hyperlanes().into_iter().collect();
    for id in SYSTEM_NAMES {
        let ix = sys(id)?;
        if raw.is_hyperlane(ix) != hyperlanes.contains(id) {
            return Err(Unmapped::System(id.to_owned()));
        }
    }
    raw.hyperlane_lanes = galaxy.hyperlane_lanes;
    raw.wormholes_off = galaxy.wormholes_off;
    raw.wormholes_all_linked = galaxy.wormholes_all_linked;
    raw.wormhole_star_links = galaxy.wormhole_star_links;
    raw.nexus_wormholes_off = false; // applied above
    for (id, linked) in &galaxy.extra_links {
        raw.extra_links
            .push((sys(id)?, sys_set(linked.iter().map(String::as_str))?));
    }
    raw.refresh_lane_links();
    Ok(raw)
}

/// A typed galaxy built natively from tiles, the way `Galaxy::placed` builds the string one (for
/// the test that both builders agree; the typed core owns its galaxy at the end of the migration).
///
/// # Errors
/// An id or hex the core cannot hold.
pub fn galaxy_native(tiles: &[(&str, Hex)], off_map: &[&str]) -> Result<CoreGalaxy, Unmapped> {
    let mut raw = CoreGalaxy::new();
    for (id, hex) in tiles {
        raw.place(sys(id)?, *hex)
            .map_err(|_| Unmapped::Duplicate((*id).to_owned()))?;
    }
    for id in off_map {
        raw.place_off_map(sys(id)?)
            .map_err(|_| Unmapped::Duplicate((*id).to_owned()))?;
    }
    raw.refresh_lane_links();
    Ok(raw)
}

/// The owners of one table, in text order: seat `i` is the `i`-th.
#[derive(Debug, Clone)]
pub struct Seats {
    ids: Vec<String>,
}

impl Seats {
    /// Every player and every owner of a unit or a command token on the board.
    ///
    /// # Errors
    /// More than 16 owners.
    pub fn of(state: &GameState) -> Result<Self, Unmapped> {
        let mut ids: BTreeSet<&str> = state.players.iter().map(|p| p.id.as_str()).collect();
        for system in state.board.values() {
            ids.extend(system.units.iter().map(|u| u.owner.as_str()));
            ids.extend(system.planet_units.values().flatten().map(|u| u.owner.as_str()));
            ids.extend(system.command_tokens.iter().map(|p| p.as_str()));
        }
        if ids.len() > 16 {
            return Err(Unmapped::Seats(ids.len()));
        }
        Ok(Self {
            ids: ids.into_iter().map(str::to_owned).collect(),
        })
    }

    #[must_use]
    pub fn seat(&self, id: &str) -> Option<Seat> {
        self.ids
            .binary_search_by(|s| s.as_str().cmp(id))
            .ok()
            .map(|i| Seat(i as u8))
    }

    #[must_use]
    pub fn id(&self, seat: Seat) -> &str {
        &self.ids[usize::from(seat.0)]
    }
}

/// The board's occupancy as owner masks (`movement::Board::for_player`, for every seat at once).
///
/// # Errors
/// A board key the tables do not know, or an owner not in `seats`.
pub fn board_from_state(
    state: &GameState,
    seats: &Seats,
    sources: SourceSet,
) -> Result<CoreBoard, Unmapped> {
    let scope = scope(sources);
    let mut board = CoreBoard::new();
    let owner = |id: &str| seats.seat(id).ok_or(Unmapped::Seats(17));
    for (id, system) in &state.board {
        let ix = sys(id.as_str())?.index();
        board.systems.insert(ix);
        for unit in &system.units {
            let bit = owner(unit.owner.as_str())?.bit();
            board.unit_owners[ix] |= bit;
            // A type the corpus (in scope) does not carry is not a ship, as in the string board.
            if unit_ix(unit.type_id.as_str())
                .is_some_and(|t| UNITS[t.index()].is_ship && UNITS[t.index()].source & scope.0 != 0)
            {
                board.ship_owners[ix] |= bit;
            }
        }
        for unit in system.planet_units.values().flatten() {
            board.unit_owners[ix] |= owner(unit.owner.as_str())?.bit();
        }
        for player in &system.command_tokens {
            board.command_tokens[ix] |= owner(player.as_str())?.bit();
        }
    }
    Ok(board)
}

fn sparse<'s>(
    map: &'s BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<(SystemIx, SysSet)>, Unmapped> {
    map.iter()
        .map(|(k, v)| Ok((sys(k)?, sys_set(v.iter().map(String::as_str))?)))
        .collect()
}

/// Whether `rules` search the given galaxy, or a copy a faction module made (then its adjacency
/// must be converted from `rules.parts().galaxy`).
#[must_use]
pub fn uses_galaxy(rules: &MovementRules<'_>, galaxy: &Galaxy) -> bool {
    std::ptr::eq(rules.parts().galaxy, galaxy)
}

fn names(set: &BTreeSet<String>) -> impl Iterator<Item = &str> {
    set.iter().map(String::as_str)
}

/// A built `MovementRules` (laws, technologies, action cards and faction hooks already applied)
/// as typed rules over `adjacency`, which must be the adjacency of `rules.parts().galaxy`.
///
/// # Errors
/// An id the tables do not know where the search could visit it.
pub fn rules_from_legacy(
    rules: &MovementRules<'_>,
    sources: SourceSet,
    adjacency: &std::rc::Rc<Adjacency>,
) -> Result<CoreRules, Unmapped> {
    let p = rules.parts();
    let mut blocked: Vec<(SystemIx, SysSet)> = Vec::new();
    for (a, b) in p.blocked_edges {
        let (a, b) = (sys(a)?, sys(b)?);
        for (from, to) in [(a, b), (b, a)] {
            match blocked.iter_mut().find(|(k, _)| *k == from) {
                Some((_, set)) => {
                    set.insert(to.index());
                }
                None => {
                    let mut set = SysSet::EMPTY;
                    set.insert(to.index());
                    blocked.push((from, set));
                }
            }
        }
    }
    let ship_adjacent = p
        .ship_adjacent
        .iter()
        .filter_map(|(k, v)| unit_ix(k).map(|k| (k, v)))
        .map(|(k, v)| Ok((k, sys_set(v.iter().map(String::as_str))?)))
        .collect::<Result<Vec<_>, Unmapped>>()?;
    Ok(CoreRules {
        adjacency: adjacency.clone(),
        scope: scope(sources),
        active: sys(p.active_system)?,
        enemy_ships: sys_set_lossy(names(&p.board.enemy_ships)),
        own_command_tokens: sys_set_lossy(names(&p.board.own_command_tokens)),
        nebulae_open: rules.nebulae_open,
        asteroid_fields_open: rules.asteroid_fields_open,
        supernovae_open: rules.supernovae_open,
        supernovae_pass_through: rules.supernovae_pass_through,
        command_tokens_ignored: rules.command_tokens_ignored,
        anomalies_ignored: rules.anomalies_ignored,
        // Compared with the origin only; an origin is always mapped, so an unmapped id never
        // matches, exactly like `None`.
        ignore_enemy_ships_from: rules.ignore_enemy_ships_from.as_deref().and_then(system_ix),
        ignore_enemy_ships: rules.ignore_enemy_ships,
        also_adjacent: sys_set(names(&rules.also_adjacent))?,
        extra_adjacency: sparse(p.extra_adjacency)?,
        token_wormhole_systems: sys_set(names(&rules.token_wormhole_systems))?,
        barred_transit: sys_set_lossy(names(&rules.barred_transit)),
        gravity_rift_systems: sys_set_lossy(names(&rules.gravity_rift_systems)),
        rift_extra_steps: rules.rift_extra_steps,
        rifts_ignored: rules.rifts_ignored,
        rift_roll_exempt: sys_set_lossy(names(&rules.rift_roll_exempt)),
        passing_ship_types: unit_set_lossy(names(p.passing_ship_types)),
        own_passage_types: unit_set_lossy(names(p.own_passage_types)),
        own_unit_systems: sys_set_lossy(names(p.own_unit_systems)),
        ship_adjacent,
        token_free_types: unit_set_lossy(names(p.token_free_types)),
        nebulae_ignored: p.nebulae_ignored,
        blocked_edges: blocked,
        unladen_pass: p.unladen_pass,
    })
}

/// Whether a built `MovementRules` carries nothing beyond what [`CoreRules::plain`] plus the
/// per-mover flags of [`MoverFlags`] express: no faction-hook state, no extra adjacency.
#[must_use]
pub fn is_plain(rules: &MovementRules<'_>, galaxy: &Galaxy) -> bool {
    let p = rules.parts();
    uses_galaxy(rules, galaxy)
        && p.extra_adjacency.is_empty()
        && p.passing_ship_types.is_empty()
        && p.own_passage_types.is_empty()
        && p.ship_adjacent.is_empty()
        && p.token_free_types.is_empty()
        && !p.nebulae_ignored
        && p.blocked_edges.is_empty()
        && !p.unladen_pass
        && rules.also_adjacent.is_empty()
        && rules.token_wormhole_systems.is_empty()
        && rules.barred_transit.is_empty()
        && rules.gravity_rift_systems.is_empty()
        && rules.rift_extra_steps == 0
        && !rules.supernovae_open
        && !rules.supernovae_pass_through
}

/// The flags one mover carries into every destination (laws, technologies, cards in play): the
/// part of `with_laws` + `apply_movement_effects` that does not depend on the destination.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MoverFlags {
    pub nebulae_open: bool,
    pub asteroid_fields_open: bool,
    pub command_tokens_ignored: bool,
    pub anomalies_ignored: bool,
    pub ignore_enemy_ships: bool,
    pub ignore_enemy_ships_from: Option<SystemIx>,
}

impl MoverFlags {
    #[must_use]
    pub fn of(rules: &MovementRules<'_>) -> Self {
        Self {
            nebulae_open: rules.nebulae_open,
            asteroid_fields_open: rules.asteroid_fields_open,
            command_tokens_ignored: rules.command_tokens_ignored,
            anomalies_ignored: rules.anomalies_ignored,
            ignore_enemy_ships: rules.ignore_enemy_ships,
            ignore_enemy_ships_from: rules.ignore_enemy_ships_from.as_deref().and_then(system_ix),
        }
    }

    pub fn apply(&self, rules: &mut CoreRules) {
        rules.nebulae_open = self.nebulae_open;
        rules.asteroid_fields_open = self.asteroid_fields_open;
        rules.command_tokens_ignored = self.command_tokens_ignored;
        rules.anomalies_ignored = self.anomalies_ignored;
        rules.ignore_enemy_ships = self.ignore_enemy_ships;
        rules.ignore_enemy_ships_from = self.ignore_enemy_ships_from;
    }
}

/// A typed route back to id text.
#[must_use]
pub fn route_text(route: &[SystemIx]) -> Vec<String> {
    route.iter().map(|s| system_id(*s).to_owned()).collect()
}

thread_local! {
    /// The adjacency of the last few galaxies asked about on this thread. A galaxy is a plain value
    /// the game edits between steps, so the cache compares the whole value, as
    /// `ti4_host::projection::placed_galaxy` does.
    static ADJACENCY: std::cell::RefCell<Vec<(Galaxy, std::rc::Rc<Adjacency>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The typed adjacency of `galaxy`, cached per thread; `None` when the galaxy cannot be expressed.
#[must_use]
pub fn adjacency_for(galaxy: &Galaxy) -> Option<std::rc::Rc<Adjacency>> {
    ADJACENCY.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((_, found)) = cache.iter().find(|(g, _)| g == galaxy) {
            return Some(found.clone());
        }
        let adjacency = std::rc::Rc::new(galaxy_from_legacy(galaxy).ok()?.adjacency());
        if cache.len() >= 4 {
            cache.remove(0);
        }
        cache.push((galaxy.clone(), adjacency.clone()));
        Some(adjacency)
    })
}

/// `rules.path_from_ship(origin, move_value, ship_type)`, searched by the typed core; the string
/// search answers when anything is unmapped. One translation of the rules per call: for callers
/// that ask once per set of rules.
#[must_use]
pub fn path_from_ship(
    rules: &MovementRules<'_>,
    sources: SourceSet,
    origin: &str,
    move_value: i32,
    ship_type: Option<&str>,
) -> Option<Vec<String>> {
    let typed = (|| {
        let adjacency = adjacency_for(rules.parts().galaxy)?;
        let raw = rules_from_legacy(rules, sources, &adjacency).ok()?;
        let from = system_ix(origin)?;
        let kind = self::ship_type(ship_type).ok()?;
        Some(raw.path_from_ship(from, move_value, kind).map(|r| route_text(&r)))
    })();
    typed.unwrap_or_else(|| rules.path_from_ship(origin, move_value, ship_type))
}

/// `rules.all_routes_from_ship(origin, move_value, ship_type)`, searched by the typed core (same
/// routes, same order, same bounds); the string search answers when anything is unmapped.
#[must_use]
pub fn all_routes_from_ship(
    rules: &MovementRules<'_>,
    sources: SourceSet,
    origin: &str,
    move_value: i32,
    ship_type: Option<&str>,
) -> Vec<Vec<String>> {
    let typed = (|| {
        let adjacency = adjacency_for(rules.parts().galaxy)?;
        let raw = rules_from_legacy(rules, sources, &adjacency).ok()?;
        let from = system_ix(origin)?;
        let kind = self::ship_type(ship_type).ok()?;
        Some(
            raw.all_routes_from_ship(from, move_value, kind)
                .iter()
                .map(|r| route_text(r))
                .collect(),
        )
    })();
    typed.unwrap_or_else(|| rules.all_routes_from_ship(origin, move_value, ship_type))
}

/// One seat's movement rules for every destination of one position (docs/ENGINE_CORE.md, stage 2).
///
/// `MovementRules::with_laws` depends on the destination in one place only: the hooks that take a
/// `PassSite` (which ship types may pass other players' ships, and which earn steps through the
/// mover's own systems; Mentak's Corsair reads the active system). Everything else (the board facts,
/// laws, technologies, cards, the other hooks, the map copies) is the same for every destination.
/// So the string rules are built **once** per seat and translated, and each destination gets a
/// copy with its active system and those two sets asked again. The dual run checks, for every
/// seat and destination of the checked positions, that this equals translating `with_laws` built
/// for that destination, field by field.
#[derive(Debug, Clone)]
pub struct SeatReach {
    template: CoreRules,
    player: ti4_model::id::PlayerId,
    /// The mover's unit types on the board, as `with_laws` collects them for the `PassSite` hooks.
    types: Vec<String>,
    /// Systems holding any of the mover's units (`own_unit_systems` when a type earns steps).
    own_unit_systems: SysSet,
    site_hooks: bool,
}

impl SeatReach {
    /// `effects`: also `action_cards::apply_movement_effects` (what a real move and `movable_into`
    /// use); without it, the rules `Observed::can_reach` builds (laws only, no cards in hand).
    #[must_use]
    pub fn new(
        state: &GameState,
        content: &ti4_content::ContentStore,
        sources: SourceSet,
        galaxy: &Galaxy,
        player: &ti4_model::id::PlayerId,
        effects: bool,
    ) -> Option<Self> {
        use crate::factions::hooks_movement as hooks;
        let any_destination = galaxy.system_ids_holding_things().into_iter().next()?;
        let board = crate::movement::Board::for_player(state, content, sources, player);
        let mut rules =
            MovementRules::with_laws(galaxy, content, sources, any_destination, board, Some(state));
        if effects {
            crate::action_cards::apply_movement_effects(&mut rules, state, player);
        }
        let adjacency = adjacency_for(rules.parts().galaxy)?;
        let template = rules_from_legacy(&rules, sources, &adjacency).ok()?;
        let site_hooks = hooks::any(|table| {
            table.may_move_through_ships.is_some()
                || table.own_unit_passage.is_some()
                || table.unit_adjacent_systems.is_some()
                || table.ignores_command_tokens.is_some()
        });
        let types: Vec<String> = state
            .board
            .values()
            .flat_map(|system| system.units.iter())
            .filter(|unit| &unit.owner == player)
            .map(|unit| unit.type_id.as_str())
            .collect::<BTreeSet<&str>>()
            .into_iter()
            .map(str::to_owned)
            .collect();
        let own_unit_systems = sys_set_lossy(
            state
                .board
                .iter()
                .filter(|(_, b)| {
                    b.units
                        .iter()
                        .chain(b.planet_units.values().flatten())
                        .any(|unit| &unit.owner == player)
                })
                .map(|(system, _)| system.as_str()),
        );
        Some(Self {
            template,
            player: player.clone(),
            types,
            own_unit_systems,
            site_hooks,
        })
    }

    /// The rules for moving into `destination`; `None` when it is not a corpus system.
    #[must_use]
    pub fn rules_for(
        &self,
        state: &GameState,
        content: &ti4_content::ContentStore,
        sources: SourceSet,
        destination: &ti4_model::id::SystemId,
    ) -> Option<CoreRules> {
        use crate::factions::hooks_movement as hooks;
        let mut rules = self.template.clone();
        rules.active = system_ix(destination.as_str())?;
        rules.passing_ship_types = UnitSet::EMPTY;
        rules.own_passage_types = UnitSet::EMPTY;
        rules.own_unit_systems = SysSet::EMPTY;
        if self.site_hooks {
            for ship_type in &self.types {
                let site = hooks::PassSite {
                    player: &self.player,
                    active: destination,
                    ship_type,
                };
                if let Some(kind) = unit_ix(ship_type) {
                    if hooks::may_move_through_ships(state, content, sources, &site) {
                        rules.passing_ship_types.insert(kind.index());
                    }
                    if hooks::own_unit_passage(state, content, sources, &site) {
                        rules.own_passage_types.insert(kind.index());
                    }
                }
            }
            if !rules.own_passage_types.is_empty() {
                rules.own_unit_systems = self.own_unit_systems;
            }
        }
        Some(rules)
    }
}

/// What one position's `Observed` keeps for the typed queries: per seat the rules template (with
/// and without the cards in play) and the fleet.
#[derive(Debug, Default)]
pub struct SeatCache {
    pub reach: BTreeMap<(ti4_model::id::PlayerId, bool), Option<std::rc::Rc<SeatReach>>>,
    #[cfg(all(feature = "typed-core", not(feature = "string-movement")))]
    pub fleets: BTreeMap<ti4_model::id::PlayerId, std::rc::Rc<crate::tactical::SeatFleet>>,
}

impl SeatCache {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            reach: BTreeMap::new(),
            #[cfg(all(feature = "typed-core", not(feature = "string-movement")))]
            fleets: BTreeMap::new(),
        }
    }
}
