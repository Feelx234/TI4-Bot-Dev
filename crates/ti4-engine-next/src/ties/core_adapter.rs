//! From `core_adapter.rs` of the legacy crate: the typed id lookups, the scope mask, the kernel of a scope and the typed galaxy of a string
//! `Galaxy` (copies; the position/seat caches of the string `Observed` are 8b3's business).

use std::collections::BTreeSet;

use ti4_content::galaxy::Galaxy;
use ti4_engine_core::containers::Cow;
use ti4_engine_core::galaxy::{Adjacency, CoreGalaxy};
use ti4_engine_core::generated::{Source as CoreSource, Wormhole};
use ti4_engine_core::hex::Hex;
use ti4_engine_core::kernel::{Kernel, Sources as CoreSources};
use ti4_engine_core::tables::Scope;
use ti4_engine_core::{SysSet, SystemIx};
use ti4_model::content_types::SourceSet;

use crate::generated::IdStr;


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

/// The typed enum of a string id (`None` for an id the generated tables do not know). The one copy
/// every view of the string engine's typed areas uses.
#[must_use]
pub fn typed<T: IdStr>(id: &str) -> Option<T> {
    T::from_id(id)
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

/// The printed lanes of every hyperlane tile, by system index, for the core's galaxy
/// (`ti4_content::hyperlane::lanes_of` of each hyperlane record; built once).
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

pub fn sys(id: &str) -> Result<SystemIx, Unmapped> {
    system_ix(id).ok_or_else(|| Unmapped::System(id.to_owned()).noted())
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

#[must_use]
pub fn system_id(ix: SystemIx) -> &'static str {
    ix.as_str()
}

#[must_use]
pub fn system_ix(id: &str) -> Option<SystemIx> {
    <SystemIx as IdStr>::from_id(id)
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

/// A string galaxy as the typed map slice holds it.
#[derive(Clone)]
pub struct TypedMap {
    pub galaxy: Cow<CoreGalaxy>,
    pub adjacency: std::sync::Arc<Adjacency>,
}

thread_local! {
    /// The typed galaxy and adjacency of the last few string galaxies asked about on this thread. A
    /// galaxy is a plain value the game edits between steps, so the cache compares the whole value.
    static GALAXIES: std::cell::RefCell<GalaxyCache> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The galaxies a thread has asked about, with their typed maps.
type GalaxyCache = Vec<(Galaxy, TypedMap)>;

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
