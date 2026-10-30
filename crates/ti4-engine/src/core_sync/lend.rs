//! The lend: load the declared slices, run the ported call on the core, store the written slices.

use std::ops::Deref;
use std::time::Instant;

use ti4_engine_core::state::{CoreState, Slice, SliceSet};
use ti4_model::state::GameState;

use super::codec::{Codec, Sources};
use super::codec::rng::TextSeeds;
use super::ctx::Ctx;
use super::error::R;
use super::footprint::Footprint;

/// What a ported call gets besides the state: the seeding of random streams (the adapter's domain
/// texts). The rule tables of the core are static; knobs join here as areas need them.
pub struct Kernel<'a> {
    pub seeds: &'a TextSeeds,
}

/// Count and time of the loads and stores of one slice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SliceCost {
    pub loads: u64,
    pub load_ns: u64,
    pub stores: u64,
    pub store_ns: u64,
}

/// The lend-region report (docs 3.3 item 6): per slice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LendStats {
    pub slices: [SliceCost; Slice::COUNT],
    pub lends: u64,
}

impl LendStats {
    pub fn cost(&self, s: Slice) -> &SliceCost {
        &self.slices[s as usize]
    }
}

fn load(cx: &mut Ctx, gs: &GameState, src: Sources<'_>, slices: SliceSet, stats: &mut LendStats) -> R<CoreState> {
    let mut core = CoreState::with_loaded(SliceSet::EMPTY);
    cx.attach(&mut core);
    for slice in Slice::ALL {
        if slices.has(slice) {
            let t = Instant::now();
            Codec::load_slice(cx, gs, src, slice, &mut core)?;
            let c = &mut stats.slices[slice as usize];
            c.loads += 1;
            c.load_ns += t.elapsed().as_nanos() as u64;
        }
    }
    cx.adopt(&core);
    Ok(core)
}

fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState, slices: SliceSet, stats: &mut LendStats) -> R<()> {
    for slice in Slice::ALL {
        if slices.has(slice) {
            let t = Instant::now();
            Codec::store_slice(cx, core, gs, slice)?;
            let c = &mut stats.slices[slice as usize];
            c.stores += 1;
            c.store_ns += t.elapsed().as_nanos() as u64;
        }
    }
    Ok(())
}

/// The lender of one game: the string state, its tables, the outside sources, the cost report.
pub struct Lend<'g> {
    pub gs: &'g mut GameState,
    pub cx: &'g mut Ctx,
    pub src: Sources<'g>,
    pub stats: LendStats,
}

impl<'g> Lend<'g> {
    pub fn new(gs: &'g mut GameState, cx: &'g mut Ctx, src: Sources<'g>) -> Self {
        Self { gs, cx, src, stats: LendStats::default() }
    }

    /// One ported call: `lend.with_core(FOOTPRINT, |core, k| ...)`.
    ///
    /// # Errors
    /// A value the typed state cannot hold (nothing was changed), or a typed value that has no text
    /// (the written slices are partly stored: the caller must discard the state).
    pub fn with_core<T>(&mut self, fp: Footprint, f: impl FnOnce(&mut CoreState, &Kernel<'_>) -> T) -> R<T> {
        self.cx.refresh(self.gs)?;
        let mut core = load(self.cx, self.gs, self.src, fp.loads(), &mut self.stats)?;
        let seeds = TextSeeds(core.texts.clone());
        let out = f(&mut core, &Kernel { seeds: &seeds });
        store(self.cx, &core, self.gs, fp.writes, &mut self.stats)?;
        self.cx.adopt(&core);
        self.stats.lends += 1;
        Ok(out)
    }

    /// Several ported calls over one load: the closure gets a [`Region`] whose `call`s each declare a
    /// footprint inside the region's. Calls inside a region must be ported areas only.
    ///
    /// # Errors
    /// As [`Self::with_core`].
    pub fn region<T>(&mut self, fp: Footprint, f: impl FnOnce(&mut Region) -> T) -> R<T> {
        self.cx.refresh(self.gs)?;
        let core = load(self.cx, self.gs, self.src, fp.loads(), &mut self.stats)?;
        let seeds = TextSeeds(core.texts.clone());
        let mut region = Region { core, fp, seeds };
        let out = f(&mut region);
        let core = region.core;
        store(self.cx, &core, self.gs, fp.writes, &mut self.stats)?;
        self.cx.adopt(&core);
        self.stats.lends += 1;
        Ok(out)
    }

    /// A read-only typed view of `reads` (one per position; cache it).
    ///
    /// # Errors
    /// A value the typed state cannot hold.
    pub fn view(&mut self, reads: SliceSet) -> R<CoreView> {
        self.cx.refresh(self.gs)?;
        Ok(CoreView { core: load(self.cx, self.gs, self.src, reads, &mut self.stats)? })
    }
}

/// An open region: one load, several calls, one store.
pub struct Region {
    core: CoreState,
    fp: Footprint,
    seeds: TextSeeds,
}

impl Region {
    /// One call inside the region; its footprint must lie inside the region's.
    pub fn call<T>(&mut self, fp: Footprint, f: impl FnOnce(&mut CoreState, &Kernel<'_>) -> T) -> T {
        debug_assert!(self.fp.loads().contains_all(fp.loads()), "call reads a slice the region did not load");
        debug_assert!(self.fp.writes.contains_all(fp.writes), "call writes a slice the region does not store");
        f(&mut self.core, &Kernel { seeds: &self.seeds })
    }

    #[must_use]
    pub fn state(&self) -> &CoreState {
        &self.core
    }
}

/// The typed view of a position: `CoreView::of(&game_state, ..)`. Read-only; rebuilt per position.
#[derive(Debug, Clone)]
pub struct CoreView {
    core: CoreState,
}

impl CoreView {
    /// # Errors
    /// A value the typed state cannot hold.
    pub fn of(gs: &GameState, cx: &mut Ctx, src: Sources<'_>, reads: SliceSet) -> R<Self> {
        cx.refresh(gs)?;
        Ok(Self { core: Codec::load(cx, gs, src, reads)? })
    }

    /// A view over a state loaded by hand (a caller that loads fewer parts of a slice than its codec).
    #[must_use]
    pub const fn from_state(core: CoreState) -> Self {
        Self { core }
    }

    #[must_use]
    pub fn into_state(self) -> CoreState {
        self.core
    }
}

impl Deref for CoreView {
    type Target = CoreState;
    fn deref(&self) -> &CoreState {
        &self.core
    }
}

/// `with_core` over a bare state and tables (no cost report).
///
/// # Errors
/// As [`Lend::with_core`].
pub fn with_core<T>(
    gs: &mut GameState,
    cx: &mut Ctx,
    src: Sources<'_>,
    fp: Footprint,
    f: impl FnOnce(&mut CoreState, &Kernel<'_>) -> T,
) -> R<T> {
    Lend::new(gs, cx, src).with_core(fp, f)
}

/// `region` over a bare state and tables (no cost report).
///
/// # Errors
/// As [`Lend::region`].
pub fn region<T>(
    gs: &mut GameState,
    cx: &mut Ctx,
    src: Sources<'_>,
    fp: Footprint,
    f: impl FnOnce(&mut Region) -> T,
) -> R<T> {
    Lend::new(gs, cx, src).region(fp, f)
}
