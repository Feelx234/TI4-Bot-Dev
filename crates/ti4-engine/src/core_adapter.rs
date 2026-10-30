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

use ti4_content::galaxy::Galaxy;
use crate::movement::MovementRules;
use ti4_engine_core::board::CoreBoard;
use ti4_engine_core::galaxy::{Adjacency, CoreGalaxy};
use ti4_engine_core::hex::Hex;
use ti4_engine_core::movement::CoreRules;
use crate::generated::IdStr;
use ti4_engine_core::generated::{Source as CoreSource, Wormhole};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::kernel::{Kernel, Sources as CoreSources};
use ti4_engine_core::query::reach::{SeatFleet as CoreSeatFleet, SeatReach as CoreSeatReach};
use ti4_engine_core::state::{CoreState, Slice, SliceSet};
use ti4_engine_core::tables::Scope;
use crate::core_sync::{CoreView, Ctx};
use crate::core_sync::codec::Sources as CodecSources;
use ti4_engine_core::{Seat, SysSet, SystemIx, UnitIx, UnitSet};
use ti4_model::content_types::SourceSet;
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

impl Unmapped {
    /// Count this refusal (`core_sync::unmapped`); every construction site ends in it.
    #[must_use]
    fn noted(self) -> Self {
        let (what, text) = match &self {
            Self::System(t) => ("adapter: system", t.clone()),
            Self::Unit(t) => ("adapter: unit", t.clone()),
            Self::WormholeKind(t) => ("adapter: wormhole kind", t.clone()),
            Self::Seats(n) => ("adapter: more than 16 owners", n.to_string()),
            Self::OffGrid(t) => ("adapter: off the grid", t.clone()),
            Self::Duplicate(t) => ("adapter: duplicate", t.clone()),
        };
        crate::core_sync::unmapped::note_adapter(what, &text);
        self
    }
}

#[must_use]
pub fn system_ix(id: &str) -> Option<SystemIx> {
    <SystemIx as IdStr>::from_id(id)
}

#[must_use]
pub fn system_id(ix: SystemIx) -> &'static str {
    ix.as_str()
}

#[must_use]
pub fn unit_ix(id: &str) -> Option<UnitIx> {
    <UnitIx as IdStr>::from_id(id)
}

#[must_use]
pub fn unit_id(ix: UnitIx) -> &'static str {
    ix.as_str()
}

/// The printed lanes of every hyperlane tile, by system index, for the core's galaxy
/// (`ti4_content::hyperlane::lanes_of` of each hyperlane record; built once).
#[must_use]
pub fn lane_table() -> &'static ti4_engine_core::tables::LaneTable {
    static TABLE: std::sync::OnceLock<Vec<Option<&'static [(u8, u8)]>>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        SystemIx::ALL
            .iter()
            .map(|s| {
                (s.data().hyperlane)
                    .then(|| ti4_content::hyperlane::lanes_of(s.as_str()))
                    .flatten()
                    .map(|l| &*Box::leak(l.into_boxed_slice()))
            })
            .collect()
    })
}

fn sys(id: &str) -> Result<SystemIx, Unmapped> {
    system_ix(id).ok_or_else(|| Unmapped::System(id.to_owned()).noted())
}

/// The scope mask of a source set: the core's bit of each source (`tables::source_bit`), matched
/// by the source's id text.
#[must_use]
pub fn scope(sources: SourceSet) -> Scope {
    Scope(
        sources
            .iter()
            .filter_map(|s| <CoreSource as IdStr>::from_id(&s.to_string()))
            .fold(0, |mask, s| mask | ti4_engine_core::tables::source_bit(s)),
    )
}

/// Wormhole kind text to bits.
///
/// # Errors
/// A kind the corpus does not print.
pub fn wormhole_mask<'k>(kinds: impl IntoIterator<Item = &'k str>) -> Result<u8, Unmapped> {
    let mut mask = 0;
    for kind in kinds {
        let k = <Wormhole as IdStr>::from_id(kind)
            .ok_or_else(|| Unmapped::WormholeKind(kind.to_owned()).noted())?;
        mask |= ti4_engine_core::tables::wormhole_bit(k);
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
    id.map(|t| unit_ix(t).ok_or_else(|| Unmapped::Unit(t.to_owned()).noted()))
        .transpose()
}

/// The string galaxy as a typed one: placement, effective wormhole kinds (printed, token and
/// copy edits merged, Nexus Sovereignty applied), hyperlanes, the switches and ability links.
///
/// # Errors
/// An id or hex the core cannot hold.
pub fn galaxy_from_legacy(galaxy: &Galaxy) -> Result<CoreGalaxy, Unmapped> {
    let mut raw = CoreGalaxy::new(lane_table());
    for id in galaxy.system_ids() {
        let ix = sys(id)?;
        let hex = galaxy
            .coord_of(id)
            .ok_or_else(|| Unmapped::System(id.to_owned()).noted())?;
        raw.place(ix, Hex::new(hex.q, hex.r)).map_err(|e| match e {
            ti4_engine_core::galaxy::PlaceError::Duplicate(_) => Unmapped::Duplicate(id.to_owned()).noted(),
            ti4_engine_core::galaxy::PlaceError::OffGrid(_) => Unmapped::OffGrid(id.to_owned()).noted(),
        })?;
    }
    for id in galaxy.off_map_systems() {
        raw.place_off_map(sys(id)?)
            .map_err(|_| Unmapped::Duplicate(id.to_owned()).noted())?;
    }
    // Kinds as the string galaxy reports them (printed, token, a copy's retained or suppressed kinds)
    // with Nexus Sovereignty lifted; the switch itself is copied below, so `wormhole_kinds` applies it
    // like the string galaxy does and the Ghosts' "cannot be prevented" can read the lifted kinds.
    let lifted;
    let galaxy_kinds = if galaxy.nexus_wormholes_off {
        let mut copy = galaxy.clone();
        copy.nexus_wormholes_off = false;
        lifted = copy;
        &lifted
    } else {
        galaxy
    };
    let in_play: Vec<SystemIx> = raw.in_play().iter().map(ti4_engine_core::ids::system_at).collect();
    for ix in in_play {
        raw.set_wormholes(ix, wormhole_mask(galaxy_kinds.wormhole_kinds(system_id(ix)))?);
    }
    for id in galaxy.token_wormholes.keys() {
        let ix = sys(id)?;
        if !raw.in_play().contains(ix.index()) {
            raw.set_token_wormholes(ix, wormhole_mask(galaxy_kinds.wormhole_kinds(id))?);
        }
    }
    let hyperlanes: BTreeSet<&str> = galaxy.hyperlanes().into_iter().collect();
    for ix in SystemIx::ALL {
        let id = ix.as_str();
        if raw.is_hyperlane(ix) != hyperlanes.contains(id) {
            return Err(Unmapped::System(id.to_owned()).noted());
        }
    }
    raw.hyperlane_lanes = galaxy.hyperlane_lanes;
    raw.wormholes_off = galaxy.wormholes_off;
    raw.wormholes_all_linked = galaxy.wormholes_all_linked;
    raw.wormhole_star_links = galaxy.wormhole_star_links;
    raw.nexus_wormholes_off = galaxy.nexus_wormholes_off;
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
    let mut raw = CoreGalaxy::new(lane_table());
    for (id, hex) in tiles {
        raw.place(sys(id)?, *hex)
            .map_err(|_| Unmapped::Duplicate((*id).to_owned()).noted())?;
    }
    for id in off_map {
        raw.place_off_map(sys(id)?)
            .map_err(|_| Unmapped::Duplicate((*id).to_owned()).noted())?;
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
            return Err(Unmapped::Seats(ids.len()).noted());
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
    let owner = |id: &str| seats.seat(id).ok_or_else(|| Unmapped::Seats(17).noted());
    for (id, system) in &state.board {
        let ix = sys(id.as_str())?.index();
        board.systems.insert(ix);
        for unit in &system.units {
            let bit = owner(unit.owner.as_str())?.bit();
            board.unit_owners[ix] |= bit;
            // A type the corpus (in scope) does not carry is not a ship, as in the string board.
            if unit_ix(unit.type_id.as_str())
                .is_some_and(|t| scope.unit(t).is_some_and(|u| u.ship))
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
    adjacency: &std::sync::Arc<Adjacency>,
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
        scratch: ti4_engine_core::movement::ScratchCell::default(),
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
    /// The typed galaxy and adjacency of the last few string galaxies asked about on this thread. A
    /// galaxy is a plain value the game edits between steps, so the cache compares the whole value, as
    /// `ti4_host::projection::placed_galaxy` does; a position asks once (`Position`), not per query.
    static GALAXIES: std::cell::RefCell<GalaxyCache> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The galaxies a thread has asked about, with their typed maps.
type GalaxyCache = Vec<(Galaxy, TypedMap)>;

/// The memoised ships that reach each system, for one seat.
type MovableRow = Vec<Option<std::rc::Rc<[crate::tactical::Movable]>>>;

/// A string galaxy as the typed map slice holds it.
#[derive(Clone)]
pub struct TypedMap {
    pub galaxy: Cow<CoreGalaxy>,
    pub adjacency: std::sync::Arc<Adjacency>,
}

/// The typed galaxy and adjacency of `galaxy`, cached per thread; `None` when the galaxy cannot be
/// expressed.
#[must_use]
pub fn typed_map(galaxy: &Galaxy) -> Option<TypedMap> {
    GALAXIES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((_, found)) = cache.iter().find(|(g, _)| g == galaxy) {
            return Some(found.clone());
        }
        let raw = galaxy_from_legacy(galaxy).ok()?;
        let adjacency = std::sync::Arc::new(raw.adjacency());
        let map = TypedMap { galaxy: Cow::new(raw), adjacency };
        if cache.len() >= 4 {
            cache.remove(0);
        }
        cache.push((galaxy.clone(), map.clone()));
        Some(map)
    })
}

/// The typed enum of a string id (`None` for an id the generated tables do not know). The one copy
/// every view of the string engine's typed areas uses.
#[must_use]
pub(crate) fn typed<T: IdStr>(id: &str) -> Option<T> {
    T::from_id(id)
}

/// The typed adjacency of `galaxy`; `None` when the galaxy cannot be expressed.
#[must_use]
pub fn adjacency_for(galaxy: &Galaxy) -> Option<std::sync::Arc<Adjacency>> {
    typed_map(galaxy).map(|m| m.adjacency)
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
    typed_path_from_ship(rules, sources, origin, move_value, ship_type)
        .unwrap_or_else(|| rules.path_from_ship(origin, move_value, ship_type))
}

/// [`path_from_ship`] without the fallback: `None` when something is not mapped (the dual run
/// compares only what the core answered).
#[must_use]
pub fn typed_path_from_ship(
    rules: &MovementRules<'_>,
    sources: SourceSet,
    origin: &str,
    move_value: i32,
    ship_type: Option<&str>,
) -> Option<Option<Vec<String>>> {
    let adjacency = adjacency_for(rules.parts().galaxy)?;
    let raw = rules_from_legacy(rules, sources, &adjacency).ok()?;
    let from = system_ix(origin)?;
    let kind = self::ship_type(ship_type).ok()?;
    Some(raw.path_from_ship(from, move_value, kind).map(|r| route_text(&r)))
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
    typed_all_routes_from_ship(rules, sources, origin, move_value, ship_type)
        .unwrap_or_else(|| rules.all_routes_from_ship(origin, move_value, ship_type))
}

/// [`all_routes_from_ship`] without the fallback.
#[must_use]
pub fn typed_all_routes_from_ship(
    rules: &MovementRules<'_>,
    sources: SourceSet,
    origin: &str,
    move_value: i32,
    ship_type: Option<&str>,
) -> Option<Vec<Vec<String>>> {
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
}

/// The kernel of a source scope: the hook table with the movement modules registered. Built once per
/// scope and process.
#[must_use]
pub fn kernel_for(sources: SourceSet) -> &'static Kernel {
    static KERNELS: [std::sync::OnceLock<Kernel>; 128] = [const { std::sync::OnceLock::new() }; 128];
    let slot = usize::try_from(sources.as_u32()).ok().and_then(|i| KERNELS.get(i)).unwrap_or(&KERNELS[127]);
    slot.get_or_init(|| Kernel {
        hooks: ti4_engine_core_factions::movement_table(),
        sources: CoreSources(u8::try_from(scope(sources).0).unwrap_or(u8::MAX)),
    })
}

/// A position as the typed rules read it: the loaded slices movement needs, the owner table that maps
/// player ids to seats, and the kernel. Built once per `Observed`, on the first typed question.
pub struct Position {
    view: CoreView,
    cx: Ctx,
    kernel: &'static Kernel,
}

/// The key texts of the marks the movement hooks read (`ti4-engine-core-factions`): Crimson's sever
/// tokens and active breaches, the Empyrean's marks, the Firmament agent, the Ghosts' agent link, the
/// Nekro Z tokens, the Nomad hero, the Cabal's Crucible, the Saar agent, Winnu's Imperator and Law's
/// Order. Every other mark is left out of the reach view.
const REACH_MARKS: [&str; 11] = [
    "cabal:crucible:",
    "crimson:breach:active:",
    "crimson:sever:",
    "empyrean|",
    "firmament:agent",
    "ghost:agent_link",
    "keleres|laws_blank",
    "nekro:token:",
    "nomad:hero:",
    "private:",
    "saar:agent:boost:",
];

/// The board as movement reads it: the units of every space area and planet, command tokens, which
/// systems have an entry, who controls each planet (Mecatol Rex and Tempesta are asked) and where a
/// placed planet sits. Not read: exhaustion, attachments, coexistence, purges (the full codec copies
/// a chunk of the planet table for each of them).
fn load_reach_board(cx: &Ctx, gs: &GameState, core: &mut CoreState) -> Option<()> {
    use ti4_engine_core::generated::Planet;
    let seat = |id: &ti4_model::id::PlayerId| cx.owners.seat(id.as_str());
    let piece = |u: &ti4_model::units::Unit| -> Option<ti4_engine_core::state::Piece> {
        Some(
            ti4_engine_core::state::Piece::new(unit_ix(u.type_id.as_str())?, seat(&u.owner)?)
                .with_sustained(u.sustained_damage)
                .with_galvanized(u.galvanized),
        )
    };
    let board = &mut core.board;
    for (sid, st) in &gs.board {
        let cell = board.sys_mut(system_ix(sid.as_str())?);
        for u in &st.units {
            cell.units.push(piece(u)?);
        }
        for p in &st.command_tokens {
            cell.cmd.insert(seat(p)?);
        }
        cell.flags.set(ti4_engine_core::state::SysFlags::TOUCHED, true);
        for (pid, owner) in &st.planet_control {
            board.planet_mut(<Planet as IdStr>::from_id(pid.as_str())?).control = Some(seat(owner)?);
        }
        for (pid, units) in &st.planet_units {
            let cell = board.planet_mut(<Planet as IdStr>::from_id(pid.as_str())?);
            for u in units {
                cell.units.push(piece(u)?);
            }
        }
    }
    for (pid, sid) in &gs.placed_planets {
        board.planet_mut(<Planet as IdStr>::from_id(pid.as_str())?).placed_in = Some(system_ix(sid.as_str())?);
    }
    core.mark_loaded(Slice::Board);
    Some(())
}

/// The slices the movement rules read, loaded the cheap way: the board, the players and the map by
/// their codecs; of the explore, agenda, transient and marks slices only the few facts movement reads
/// (the full codecs load every exploration record, every law outcome and every mark).
fn load_reach(cx: &mut Ctx, gs: &GameState, galaxy: &Galaxy) -> Option<CoreState> {
    use ti4_engine_core::generated::{Agenda, Wormhole};
    use ti4_engine_core::keys::Outcome;
    let src = CodecSources { galaxy: Some(galaxy), rng: None };
    let mut core = CoreState::with_loaded(SliceSet::EMPTY);
    for slice in [Slice::Meta, Slice::Players, Slice::Map] {
        crate::core_sync::Codec::load_slice(cx, gs, src, slice, &mut core).ok()?;
    }
    load_reach_board(cx, gs, &mut core)?;
    let e = &mut core.explore;
    for (set, texts) in [(&mut e.ingress, &gs.ingress_tokens), (&mut e.breach, &gs.breach_tokens)] {
        for t in texts {
            set.insert(system_ix(t.as_str())?);
        }
    }
    for (kind, system) in &gs.wormhole_tokens {
        e.wormhole_tokens.insert(<Wormhole as IdStr>::from_id(kind)?, system_ix(system.as_str())?);
    }
    core.mark_loaded(Slice::Explore);
    if gs.laws.contains_key("shared_research") {
        core.agenda.laws.insert(Agenda::SharedResearch, Outcome::Empty);
    }
    core.mark_loaded(Slice::Agenda);
    for (system, value) in &gs.gravleash_move_values {
        core.transient.movement.gravleash_move_values.insert(system_ix(system.as_str())?, *value);
    }
    core.mark_loaded(Slice::Transient);
    let mut marks = ti4_engine_core::state::MarkMap::new();
    for (key, value) in &gs.faction_marks {
        if REACH_MARKS.iter().any(|p| key.starts_with(p)) {
            let (k, v) = cx.keys.parse_mark(&mut core.texts.blobs, key, value)?;
            marks.insert(k, v);
        }
    }
    core.marks.tail = Cow::new(marks);
    core.mark_loaded(Slice::Marks);
    Some(core)
}

impl Position {
    /// `None` when a value of the position cannot be held by the typed state (the caller asks the
    /// string engine).
    #[must_use]
    pub fn new(
        state: &GameState,
        sources: SourceSet,
        galaxy: &Galaxy,
    ) -> Option<std::rc::Rc<Self>> {
        let mut cx = Ctx::of(state).ok()?;
        let view = CoreView::from_state(load_reach(&mut cx, state, galaxy)?);
        Some(std::rc::Rc::new(Self { view, cx, kernel: kernel_for(sources) }))
    }

    /// The seat of a player id.
    #[must_use]
    pub fn seat(&self, player: &ti4_model::id::PlayerId) -> Option<Seat> {
        self.cx.owners.seat(player.as_str())
    }

    /// The typed state.
    #[must_use]
    pub fn core(&self) -> &CoreState {
        &self.view
    }

    #[must_use]
    pub const fn kernel(&self) -> &'static Kernel {
        self.kernel
    }
}

/// One seat's movement rules for every destination of one position, built by the typed core from the
/// hook table (no string `MovementRules`). The rules of a destination are built once and shared.
pub struct SeatReach {
    pos: std::rc::Rc<Position>,
    reach: CoreSeatReach,
    rules: std::cell::RefCell<Vec<Option<std::rc::Rc<CoreRules>>>>,
}

impl SeatReach {
    /// `effects`: also the cards in play (what a real move and `movable_into` use); without it, the
    /// rules `Observed::can_reach` builds (laws only, no cards in hand).
    #[must_use]
    pub fn new(pos: &std::rc::Rc<Position>, player: &ti4_model::id::PlayerId, effects: bool) -> Option<Self> {
        let seat = pos.seat(player)?;
        let reach = CoreSeatReach::new(pos.core(), pos.kernel, seat, effects)?;
        Some(Self { pos: pos.clone(), reach, rules: std::cell::RefCell::new(Vec::new()) })
    }

    /// The rules for moving into `destination`.
    #[must_use]
    pub fn rules_for(&self, destination: SystemIx) -> std::rc::Rc<CoreRules> {
        let mut rules = self.rules.borrow_mut();
        if rules.is_empty() {
            rules.resize(SystemIx::ALL.len(), None);
        }
        rules[destination.index()]
            .get_or_insert_with(|| std::rc::Rc::new(self.reach.rules_for(self.pos.core(), self.pos.kernel, destination)))
            .clone()
    }

    #[must_use]
    pub const fn seat(&self) -> Seat {
        self.reach.seat()
    }
}

/// What one position's `Observed` keeps for the typed queries, indexed by seat and system.
#[derive(Default)]
pub struct SeatCache {
    position: Option<Option<std::rc::Rc<Position>>>,
    reach: [[Option<Option<std::rc::Rc<SeatReach>>>; 2]; 16],
    fleets: [Option<std::rc::Rc<CoreSeatFleet>>; 16],
    movable: [Option<MovableRow>; 8],
    rift_exits: std::collections::HashMap<u32, usize>,
}

impl SeatCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The typed position, built on first use.
    pub fn position(
        &mut self,
        state: &GameState,
        sources: SourceSet,
        galaxy: &Galaxy,
    ) -> Option<std::rc::Rc<Position>> {
        self.position.get_or_insert_with(|| Position::new(state, sources, galaxy)).clone()
    }

    /// The rules of one seat.
    pub fn reach(
        &mut self,
        pos: &std::rc::Rc<Position>,
        player: &ti4_model::id::PlayerId,
        effects: bool,
    ) -> Option<std::rc::Rc<SeatReach>> {
        let seat = pos.seat(player)?;
        self.reach[usize::from(seat.0)][usize::from(effects)]
            .get_or_insert_with(|| SeatReach::new(pos, player, effects).map(std::rc::Rc::new))
            .clone()
    }

    /// The ships of one seat.
    pub fn fleet(&mut self, pos: &Position, seat: Seat) -> std::rc::Rc<CoreSeatFleet> {
        self.fleets[usize::from(seat.0)]
            .get_or_insert_with(|| std::rc::Rc::new(CoreSeatFleet::new(pos.core(), pos.kernel, seat)))
            .clone()
    }

    /// The memoised ships that reach a system, when asked before.
    #[must_use]
    pub fn known_movable(&self, row: usize, system: SystemIx) -> Option<std::rc::Rc<[crate::tactical::Movable]>> {
        self.movable.get(row)?.as_ref()?[system.index()].clone()
    }

    pub fn remember_movable(&mut self, row: usize, system: SystemIx, found: std::rc::Rc<[crate::tactical::Movable]>) {
        if let Some(cell) = self.movable.get_mut(row) {
            cell.get_or_insert_with(|| vec![None; SystemIx::ALL.len()])[system.index()] = Some(found);
        }
    }

    /// The memoised rift exits of one ship type and route, when asked before.
    #[must_use]
    pub fn known_rift_exits(&self, key: u32) -> Option<usize> {
        self.rift_exits.get(&key).copied()
    }

    pub fn remember_rift_exits(&mut self, key: u32, exits: usize) {
        self.rift_exits.insert(key, exits);
    }
}

/// The key of a rift-exit query: seat, destination, origin, ship type and boosts.
#[must_use]
pub fn rift_key(row: usize, destination: SystemIx, origin: SystemIx, kind: UnitIx, boosts: (bool, bool)) -> u32 {
    ((row as u32) << 28)
        | ((destination.index() as u32) << 19)
        | ((origin.index() as u32) << 10)
        | ((kind.index() as u32) << 2)
        | u32::from(boosts.0) << 1
        | u32::from(boosts.1)
}

/// `transactions::present_together` (LRR 60: two presences share a system or occupy adjacent ones)
/// on the core's adjacency; `None` when an id is not mapped.
#[must_use]
pub fn present_together(
    galaxy: &Galaxy,
    here: &BTreeSet<ti4_model::id::SystemId>,
    there: &BTreeSet<ti4_model::id::SystemId>,
) -> Option<bool> {
    let here_set = sys_set(here.iter().map(ti4_model::id::SystemId::as_str)).ok()?;
    let there_set = sys_set(there.iter().map(ti4_model::id::SystemId::as_str)).ok()?;
    if here_set.intersects(&there_set) {
        return Some(true);
    }
    let adjacency = adjacency_for(galaxy)?;
    Some(
        here_set
            .iter()
            .any(|s| adjacency.of(ti4_engine_core::ids::system_at(s)).intersects(&there_set)),
    )
}

/// The planet a corpus id names in a source scope (`ContentStore::resolve_id`, newer printings
/// included), from a table built once per scope by asking `resolve_id` for every corpus planet.
/// `None` when the core cannot answer: a store other than the compiled-in one (`from_dir`, test
/// stores: run-time content stays on the string engine), or an id the corpus does not carry.
/// `Some(None)`: the id is a corpus planet but nothing of it is in scope.
#[must_use]
pub fn planet_in_scope(
    content: &ti4_content::ContentStore,
    id: &str,
    sources: SourceSet,
) -> Option<Option<ti4_engine_core::PlanetIx>> {
    use ti4_engine_core::PlanetIx;
    static TABLES: [std::sync::OnceLock<Vec<Option<PlanetIx>>>; 128] =
        [const { std::sync::OnceLock::new() }; 128];
    if !content.is_embedded() {
        return None;
    }
    let planet = <PlanetIx as IdStr>::from_id(id)?;
    let slot = TABLES.get(usize::try_from(sources.as_u32()).ok()?)?;
    let table = slot.get_or_init(|| {
        PlanetIx::ALL
            .iter()
            .map(|p| {
                content
                    .resolve_id(ti4_model::content_types::ContentType::Planets, p.as_str(), sources)
                    .and_then(<PlanetIx as IdStr>::from_id)
            })
            .collect()
    });
    // A resolution to a planet outside the enum cannot happen for the compiled-in corpus (every
    // record has a variant); `table` holds `None` only for "not in scope".
    Some(table[planet.index()])
}

/// The system a corpus id names in a source scope (`galaxy::system`), as [`planet_in_scope`].
#[must_use]
pub fn system_in_scope(
    content: &ti4_content::ContentStore,
    id: &str,
    sources: SourceSet,
) -> Option<Option<SystemIx>> {
    static TABLES: [std::sync::OnceLock<Vec<Option<SystemIx>>>; 128] =
        [const { std::sync::OnceLock::new() }; 128];
    if !content.is_embedded() {
        return None;
    }
    let system = <SystemIx as IdStr>::from_id(id)?;
    let slot = TABLES.get(usize::try_from(sources.as_u32()).ok()?)?;
    let table = slot.get_or_init(|| {
        SystemIx::ALL
            .iter()
            .map(|s| {
                content
                    .resolve_id(ti4_model::content_types::ContentType::Systems, s.as_str(), sources)
                    .and_then(<SystemIx as IdStr>::from_id)
            })
            .collect()
    });
    Some(table[system.index()])
}

/// `production::planet_value` (the printed resources or influence) from the generated row.
#[must_use]
pub fn planet_value(
    content: &ti4_content::ContentStore,
    sources: SourceSet,
    planet: &str,
    resources: bool,
) -> Option<i64> {
    Some(planet_in_scope(content, planet, sources)?.map_or(0, |p| {
        let d = p.data();
        i64::from(if resources { d.resources } else { d.influence })
    }))
}

/// The printed technology specialties of a planet, lower case (`tech_specialties_now`'s printed
/// part), from the generated row: a static slice per planet, built once.
#[must_use]
pub fn printed_specialties(
    content: &ti4_content::ContentStore,
    sources: SourceSet,
    planet: &str,
) -> Option<&'static [&'static str]> {
    use ti4_engine_core::generated::TechType;
    static ROWS: std::sync::OnceLock<Vec<&'static [&'static str]>> = std::sync::OnceLock::new();
    let planet = planet_in_scope(content, planet, sources)?;
    let Some(p) = planet else { return Some(&[]) };
    let rows = ROWS.get_or_init(|| {
        ti4_engine_core::PlanetIx::ALL
            .iter()
            .map(|p| {
                let names: Vec<&'static str> = p
                    .data()
                    .specialties
                    .iter()
                    .map(|t| match t {
                        TechType::Biotic => "biotic",
                        TechType::Cybernetic => "cybernetic",
                        TechType::Propulsion => "propulsion",
                        TechType::Warfare => "warfare",
                        TechType::Unitupgrade => "unitupgrade",
                    })
                    .collect();
                &*Box::leak(names.into_boxed_slice())
            })
            .collect()
    });
    Some(rows[p.index()])
}

/// `space_stations::stations_in` from the generated rows: the space stations printed on a tile.
#[must_use]
pub fn stations_in(
    content: &ti4_content::ContentStore,
    sources: SourceSet,
    system: &str,
) -> Option<Vec<ti4_model::id::PlanetId>> {
    use ti4_engine_core::generated::PlanetTrait;
    let Some(tile) = system_in_scope(content, system, sources)? else {
        return Some(Vec::new());
    };
    let mut out = Vec::new();
    for p in tile.data().planets {
        // `galaxy::is_space_station` resolves the planet id in scope first.
        let station = planet_in_scope(content, p.as_str(), sources)?
            .is_some_and(|r| r.data().traits.contains(&PlanetTrait::Spacestation));
        if station {
            out.push(ti4_model::id::PlanetId::new(p.as_str()));
        }
    }
    Some(out)
}
