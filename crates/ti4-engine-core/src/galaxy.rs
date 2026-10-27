//! The board's systems and their adjacency, typed: `ti4_content::galaxy::Galaxy` with indices for
//! ids and bitsets for sets.
//!
//! Same rules as the string galaxy (LRR Adjacency): the six hex neighbours, wormhole pairs (with
//! the law and card switches), the printed lanes of hyperlane tiles (rules version 22), and links an
//! ability grants for the current activation. [`CoreGalaxy::adjacency`] computes every system's
//! neighbours once, so a route search reads a precomputed bitset per step instead of building a
//! `BTreeSet<&str>` per call.

use crate::bits::SysSet;
use crate::hex::{Grid, Hex};
use crate::ids::SystemIx;
use crate::tables::{
    LaneTable, SYSTEM_COUNT, WH_ALPHA, WH_ALPHA_BETA, WH_BETA, WORMHOLE_KIND_COUNT, is_nexus,
    printed_wormholes,
};

/// Why a placement was refused (the string galaxy's `GalaxyError`, minus unknown ids, which cannot
/// be expressed here: the adapter refuses them before they become indices).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceError {
    Duplicate(SystemIx),
    OffGrid(SystemIx),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreGalaxy {
    /// The printed lanes of every hyperlane tile (by system index), from the adapter.
    lanes: &'static LaneTable,
    coord: Vec<Option<Hex>>,
    grid: Grid,
    /// Placed on the grid or off it: the keys of the string galaxy's `wormholes` map.
    in_play: SysSet,
    hyperlanes: SysSet,
    /// Wormhole kinds per system (printed, or whatever the adapter copied in).
    wormholes: Vec<u8>,
    /// Kinds placed by tokens, per system.
    token_wormholes: Vec<u8>,
    /// Systems the printed lanes join, both directions; empty without hyperlanes.
    lane_links: Vec<(SystemIx, SysSet)>,
    pub hyperlane_lanes: bool,
    pub wormholes_off: bool,
    pub wormholes_all_linked: bool,
    pub wormhole_star_links: bool,
    pub nexus_wormholes_off: bool,
    /// Adjacency an ability grants for the current activation, both directions listed.
    pub extra_links: Vec<(SystemIx, SysSet)>,
}


/// Every system's neighbours on one board, computed once ([`CoreGalaxy::adjacency`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adjacency {
    adjacent: Vec<SysSet>,
    hyperlanes: SysSet,
}

impl Adjacency {
    #[inline]
    #[must_use]
    pub fn of(&self, system: SystemIx) -> &SysSet {
        &self.adjacent[system.index()]
    }

    #[inline]
    #[must_use]
    pub fn is_hyperlane(&self, system: SystemIx) -> bool {
        self.hyperlanes.contains(system.index())
    }
}

fn sparse_get(list: &[(SystemIx, SysSet)], key: SystemIx) -> Option<&SysSet> {
    list.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

fn sparse_entry(list: &mut Vec<(SystemIx, SysSet)>, key: SystemIx) -> &mut SysSet {
    let at = match list.iter().position(|(k, _)| *k == key) {
        Some(at) => at,
        None => {
            list.push((key, SysSet::EMPTY));
            list.len() - 1
        }
    };
    &mut list[at].1
}

impl CoreGalaxy {
    /// An empty board; `lanes` is the printed lane table of the hyperlane tiles.
    #[must_use]
    pub fn new(lanes: &'static LaneTable) -> Self {
        Self {
            lanes,
            coord: vec![None; SYSTEM_COUNT],
            grid: Grid::default(),
            in_play: SysSet::EMPTY,
            hyperlanes: SysSet::EMPTY,
            wormholes: vec![0; SYSTEM_COUNT],
            token_wormholes: vec![0; SYSTEM_COUNT],
            lane_links: Vec::new(),
            hyperlane_lanes: false,
            wormholes_off: false,
            wormholes_all_linked: false,
            wormhole_star_links: false,
            nexus_wormholes_off: false,
            extra_links: Vec::new(),
        }
    }

    /// Put a corpus system on a hex, with its printed wormholes (`Galaxy::placed`). Call
    /// [`Self::refresh_lane_links`] after the last placement.
    ///
    /// # Errors
    /// A system placed twice, or a hex outside the dense grid.
    pub fn place(&mut self, system: SystemIx, hex: Hex) -> Result<(), PlaceError> {
        if self.coord[system.index()].is_some() {
            return Err(PlaceError::Duplicate(system));
        }
        if !self.grid.set(hex, Some(system)) {
            return Err(PlaceError::OffGrid(system));
        }
        self.coord[system.index()] = Some(hex);
        self.join(system);
        Ok(())
    }

    /// A system in play beside the grid (`Galaxy::place_off_map`).
    ///
    /// # Errors
    /// The system is already on the grid.
    pub fn place_off_map(&mut self, system: SystemIx) -> Result<(), PlaceError> {
        if self.coord[system.index()].is_some() {
            return Err(PlaceError::Duplicate(system));
        }
        self.join(system);
        Ok(())
    }

    fn join(&mut self, system: SystemIx) {
        self.in_play.insert(system.index());
        self.wormholes[system.index()] = printed_wormholes(system);
        if system.data().hyperlane {
            self.hyperlanes.insert(system.index());
        }
    }

    /// Replace the wormhole kinds of a system in play (the adapter copies the string galaxy's
    /// effective kinds, which already carry its copies' edits).
    pub fn set_wormholes(&mut self, system: SystemIx, kinds: u8) {
        self.wormholes[system.index()] = kinds;
    }

    pub fn set_token_wormholes(&mut self, system: SystemIx, kinds: u8) {
        self.token_wormholes[system.index()] = kinds;
    }

    #[must_use]
    pub fn coord_of(&self, system: SystemIx) -> Option<Hex> {
        self.coord[system.index()]
    }

    #[must_use]
    pub fn system_at(&self, hex: Hex) -> Option<SystemIx> {
        self.grid.get(hex)
    }

    #[must_use]
    pub fn is_hyperlane(&self, system: SystemIx) -> bool {
        self.hyperlanes.contains(system.index())
    }

    #[must_use]
    pub const fn in_play(&self) -> &SysSet {
        &self.in_play
    }

    /// Wormhole kinds at a system, printed and token, Nexus Sovereignty applied
    /// (`Galaxy::wormhole_kinds`).
    #[must_use]
    pub fn wormhole_kinds(&self, system: SystemIx) -> u8 {
        let i = system.index();
        let mut kinds = self.wormholes[i] | self.token_wormholes[i];
        if self.nexus_wormholes_off && is_nexus(system) {
            kinds &= !WH_ALPHA_BETA;
        }
        kinds
    }

    /// [`Self::wormhole_kinds`] with Nexus Sovereignty lifted ("game effects cannot prevent you"):
    /// what the Ghosts' Quantum Entanglement reads.
    #[must_use]
    pub fn wormhole_kinds_unsuppressed(&self, system: SystemIx) -> u8 {
        self.wormholes[system.index()] | self.token_wormholes[system.index()]
    }

    /// Systems that hold any wormhole (`Galaxy::wormhole_systems`), in system order.
    #[must_use]
    pub fn wormhole_systems(&self) -> SysSet {
        let mut out = SysSet::EMPTY;
        for i in 0..SYSTEM_COUNT {
            if self.wormhole_kinds(crate::ids::system_at(i)) != 0 {
                out.insert(i);
            }
        }
        out
    }

    /// Add wormhole kinds to a system (the modules' pieces carry them; `apply_extra_wormholes`).
    pub fn add_token_wormholes(&mut self, system: SystemIx, kinds: u8) {
        self.token_wormholes[system.index()] |= kinds;
    }

    /// Remove every wormhole of a system, printed and token (`Galaxy::suppress_wormholes_at`).
    pub fn suppress_wormholes_at(&mut self, system: SystemIx) {
        self.wormholes[system.index()] = 0;
        self.token_wormholes[system.index()] = 0;
    }

    /// Keep only these wormhole kinds everywhere (`Galaxy::retain_wormhole_kinds`).
    pub fn retain_wormhole_kinds(&mut self, keep: u8) {
        for k in self.wormholes.iter_mut().chain(self.token_wormholes.iter_mut()) {
            *k &= keep;
        }
    }

    /// Recompute which systems the printed lanes join (`Galaxy::refresh_lane_links`).
    pub fn refresh_lane_links(&mut self) {
        self.lane_links.clear();
        if self.hyperlanes.is_empty() {
            return;
        }
        let lanes_at = |hex: Hex| -> Option<&'static [(u8, u8)]> {
            let system = self.grid.get(hex)?;
            if !self.hyperlanes.contains(system.index()) {
                return None;
            }
            self.lanes.get(system.index()).copied().flatten()
        };
        let mut links: Vec<(SystemIx, SysSet)> = Vec::new();
        let placed: Vec<(SystemIx, Hex)> = self
            .coord
            .iter()
            .enumerate()
            .filter_map(|(i, h)| h.map(|h| (crate::ids::system_at(i), h)))
            .collect();
        if !placed.iter().any(|(_, h)| lanes_at(*h).is_some()) {
            return;
        }
        for &(from, start) in &placed {
            if lanes_at(start).is_some() {
                continue;
            }
            for edge in 0..6u8 {
                let first = start.edge(edge);
                let entry = (edge + 3) % 6;
                if lanes_at(first).is_some_and(|l| l.iter().any(|&(a, b)| a == entry || b == entry))
                {
                    let mut seen: Vec<(Hex, u8)> = Vec::new();
                    let mut found: Vec<Hex> = Vec::new();
                    walk(&lanes_at, &self.grid, first, entry, &mut seen, &mut found);
                    for hex in found {
                        if hex != start
                            && let Some(to) = self.grid.get(hex)
                        {
                            sparse_entry(&mut links, from).insert(to.index());
                            sparse_entry(&mut links, to).insert(from.index());
                        }
                    }
                }
            }
        }
        self.lane_links = links;
    }

    /// Systems that hold each wormhole kind (printed or token, switches applied).
    fn holders(&self) -> [SysSet; WORMHOLE_KIND_COUNT] {
        let mut by_kind = [SysSet::EMPTY; WORMHOLE_KIND_COUNT];
        for i in 0..SYSTEM_COUNT {
            let kinds = self.wormhole_kinds(crate::ids::system_at(i));
            if kinds == 0 {
                continue;
            }
            for (k, set) in by_kind.iter_mut().enumerate() {
                if kinds & (1 << k) != 0 {
                    set.insert(i);
                }
            }
        }
        by_kind
    }

    fn partners(&self, system: SystemIx, holders: &[SysSet; WORMHOLE_KIND_COUNT]) -> SysSet {
        let kinds = self.wormhole_kinds(system);
        let mut out = SysSet::EMPTY;
        if kinds == 0 {
            return out;
        }
        let alpha = WH_ALPHA.trailing_zeros() as usize;
        let beta = WH_BETA.trailing_zeros() as usize;
        if self.wormholes_all_linked && kinds & WH_ALPHA_BETA != 0 {
            out.union_with(&holders[alpha]);
            out.union_with(&holders[beta]);
        } else if self.wormhole_star_links && kinds & WH_ALPHA_BETA == WH_ALPHA_BETA {
            out = holders[alpha];
            out.intersect_with(&holders[beta]);
        } else {
            for (k, set) in holders.iter().enumerate() {
                if kinds & (1 << k) != 0 {
                    out.union_with(set);
                }
            }
        }
        out.remove(system.index());
        out
    }

    fn adjacent_with(&self, system: SystemIx, holders: &[SysSet; WORMHOLE_KIND_COUNT]) -> SysSet {
        let lanes = self.hyperlane_lanes && !self.hyperlanes.is_empty();
        let mut out = SysSet::EMPTY;
        if let Some(here) = self.coord[system.index()] {
            for n in here.neighbours() {
                if let Some(s) = self.grid.get(n)
                    && (!lanes || !self.hyperlanes.contains(s.index()))
                {
                    out.insert(s.index());
                }
            }
        }
        if lanes {
            if self.hyperlanes.contains(system.index()) {
                return SysSet::EMPTY;
            }
            if let Some(joined) = sparse_get(&self.lane_links, system) {
                out.union_with(joined);
            }
        }
        if !self.wormholes_off {
            out.union_with(&self.partners(system, holders));
        }
        if let Some(linked) = sparse_get(&self.extra_links, system) {
            out.union_with(linked);
        }
        out.remove(system.index());
        out
    }

    /// Neighbouring systems of one system (`Galaxy::adjacent`).
    #[must_use]
    pub fn adjacent(&self, system: SystemIx) -> SysSet {
        self.adjacent_with(system, &self.holders())
    }

    /// Every system's neighbours, for a run of queries on this board.
    #[must_use]
    pub fn adjacency(&self) -> Adjacency {
        let holders = self.holders();
        Adjacency {
            adjacent: (0..SYSTEM_COUNT)
                .map(|i| self.adjacent_with(crate::ids::system_at(i), &holders))
                .collect(),
            hyperlanes: self.hyperlanes,
        }
    }
}

/// `ti4_content::hyperlane::walk`: arrived at the lane tile `at` through port `entry`, follow every
/// lane that starts there; push each ordinary hex a lane leaves into.
fn walk(
    lanes_at: &impl Fn(Hex) -> Option<&'static [(u8, u8)]>,
    grid: &Grid,
    at: Hex,
    entry: u8,
    seen: &mut Vec<(Hex, u8)>,
    found: &mut Vec<Hex>,
) {
    if seen.contains(&(at, entry)) {
        return;
    }
    seen.push((at, entry));
    let Some(here) = lanes_at(at) else {
        unreachable!("walk only enters lane tiles");
    };
    for &(a, b) in here {
        if a != entry && b != entry {
            continue;
        }
        let out = if a == entry { b } else { a };
        if seen.contains(&(at, out)) {
            continue;
        }
        seen.push((at, out));
        let next = at.edge(out);
        let back = (out + 3) % 6;
        if let Some(neighbour) = lanes_at(next) {
            if neighbour.iter().any(|&(x, y)| x == back || y == back) {
                walk(lanes_at, grid, next, back, seen, found);
            }
        } else if grid.get(next).is_some() {
            found.push(next);
        }
        if let Some(p) = seen.iter().position(|s| *s == (at, out)) {
            seen.remove(p);
        }
    }
    if let Some(p) = seen.iter().position(|s| *s == (at, entry)) {
        seen.remove(p);
    }
}
