//! The string `Galaxy` (tiles on hexes, a content object) as the typed map slice. The thin crate's `ties::core_adapter::galaxy_from_legacy`
//! is the same conversion with strict-mode counters; `tests/` of the bridge compare the two on every board the setup builds.

use std::collections::BTreeSet;
use std::sync::Arc;

use ti4_content::galaxy::Galaxy;
use ti4_engine_core::containers::Cow;
use ti4_engine_core::galaxy::{Adjacency, CoreGalaxy, PlaceError};
use ti4_engine_core::generated::Wormhole;
use ti4_engine_core::hex::Hex;
use ti4_engine_core::{SysSet, SystemIx};

use crate::error::SetupError;
use crate::ids::IdStr;

/// The printed lanes of every hyperlane tile, by system index, for the core's galaxy (built once, shared by every game of the process).
#[must_use]
pub fn lane_table() -> &'static ti4_engine_core::tables::LaneTable {
    /// The lanes of each system, by index.
    type Lanes = Vec<Option<&'static [(u8, u8)]>>;
    static TABLE: std::sync::OnceLock<Lanes> = std::sync::OnceLock::new();
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

fn sys(id: &str) -> Result<SystemIx, SetupError> {
    <SystemIx as IdStr>::from_id(id).ok_or(SetupError::Galaxy)
}

fn sys_set<'s>(ids: impl IntoIterator<Item = &'s str>) -> Result<SysSet, SetupError> {
    let mut set = SysSet::EMPTY;
    for id in ids {
        set.insert(sys(id)?.index());
    }
    Ok(set)
}

fn wormhole_mask<'k>(kinds: impl IntoIterator<Item = &'k str>) -> Result<u8, SetupError> {
    let mut mask = 0;
    for kind in kinds {
        let k = <Wormhole as IdStr>::from_id(kind).ok_or(SetupError::Galaxy)?;
        mask |= ti4_engine_core::tables::wormhole_bit(k);
    }
    Ok(mask)
}

/// The string galaxy as a typed one: placement, effective wormhole kinds (printed, token and copy edits merged), hyperlanes, the switches
/// and ability links.
///
/// # Errors
/// [`SetupError::Galaxy`]: an id or hex the core cannot hold.
pub fn galaxy_to_core(galaxy: &Galaxy) -> Result<CoreGalaxy, SetupError> {
    let mut raw = CoreGalaxy::new(lane_table());
    for id in galaxy.system_ids() {
        let ix = sys(id)?;
        let hex = galaxy.coord_of(id).ok_or(SetupError::Galaxy)?;
        raw.place(ix, Hex::new(hex.q, hex.r)).map_err(|e| match e {
            PlaceError::Duplicate(_) | PlaceError::OffGrid(_) => SetupError::Galaxy,
        })?;
    }
    for id in galaxy.off_map_systems() {
        raw.place_off_map(sys(id)?).map_err(|_| SetupError::Galaxy)?;
    }
    let in_play: Vec<SystemIx> = raw.in_play().iter().map(ti4_engine_core::ids::system_at).collect();
    for ix in in_play {
        raw.set_wormholes(ix, wormhole_mask(galaxy.map_wormhole_kinds(ix.as_str()))?);
    }
    for (id, kinds) in &galaxy.token_wormholes {
        raw.set_token_wormholes(sys(id)?, wormhole_mask(kinds.iter().map(String::as_str))?);
    }
    let hyperlanes: BTreeSet<&str> = galaxy.hyperlanes().into_iter().collect();
    for ix in SystemIx::ALL {
        if raw.is_hyperlane(ix) != hyperlanes.contains(ix.as_str()) {
            return Err(SetupError::Galaxy);
        }
    }
    raw.hyperlane_lanes = galaxy.hyperlane_lanes;
    raw.wormholes_off = galaxy.wormholes_off;
    raw.wormholes_all_linked = galaxy.wormholes_all_linked;
    raw.wormhole_star_links = galaxy.wormhole_star_links;
    raw.nexus_wormholes_off = galaxy.nexus_wormholes_off;
    for (id, linked) in &galaxy.extra_links {
        raw.extra_links.push((sys(id)?, sys_set(linked.iter().map(String::as_str))?));
    }
    raw.refresh_lane_links();
    Ok(raw)
}

/// The typed galaxy and its adjacency, as the map slice holds them (`edits_applied` carried over from the string galaxy).
///
/// # Errors
/// [`SetupError::Galaxy`].
pub fn typed_map(galaxy: &Galaxy) -> Result<(Cow<CoreGalaxy>, Arc<Adjacency>), SetupError> {
    let raw = galaxy_to_core(galaxy)?;
    let adjacency = Arc::new(raw.adjacency());
    let mut typed = Cow::new(raw);
    if typed.edits_applied() != galaxy.edits_applied() {
        typed.edit().set_edits_applied(galaxy.edits_applied());
    }
    Ok((typed, adjacency))
}
