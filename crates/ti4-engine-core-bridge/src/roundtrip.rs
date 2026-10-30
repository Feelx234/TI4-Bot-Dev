//! The round-trip check of the `core_sync` codecs for one position (gate 4 of docs/ENGINE_CORE_STATE.md;
//! package D2-1 lifted it out of the `core_sync_gate` binary so that `alpha_roundtrip` runs the same check).
//!
//! For one string position `g`:
//!
//! 1. `load` every slice (a refusal is counted by reason; it is a fallback to the string engine, not a failure
//!    unless the caller runs strict);
//! 2. `store` every slice into a clone of the **previous position's** state (so a field no codec writes would keep the
//!    other position's value) and compare the serialised bytes with the position: `store(load(g)) == g`;
//! 3. `load` the stored state again with the same tables and compare the core states: `load(store(x)) == x`;
//! 4. time every slice's load and store, and count its string-side JSON bytes (lend cost table).

use std::collections::BTreeMap;
use std::time::Instant;

use serde_json::Value;
use ti4_content::galaxy::Galaxy;
use ti4_engine::core_sync::{Codec, Ctx, Slice, SliceSet, Sources};
use ti4_engine_core::state::CoreState;
use ti4_model::state::GameState;

/// The slice a top-level `GameState` JSON key belongs to (the lend cost table of the gate): the one table of the engine,
/// `core_sync::materialize::slice_of_field` (the reverse lend and the footprint audit read it too).
#[must_use]
pub fn slice_of_key(key: &str) -> Slice {
    ti4_engine::core_sync::materialize::slice_of_field(key)
}

/// Mean-cost accumulator of one slice.
#[derive(Default, Clone, Copy)]
pub struct Cost {
    pub load_ns: u128,
    pub store_ns: u128,
    pub bytes: u128,
    pub n: u128,
}

fn json(state: &GameState) -> Value {
    serde_json::to_value(state).expect("state serialises")
}

/// The top-level keys under which two serialised states differ.
#[must_use]
pub fn differing_keys(a: &Value, b: &Value) -> Vec<String> {
    let (Value::Object(a), Value::Object(b)) = (a, b) else { return vec!["<not objects>".into()] };
    let mut keys: Vec<String> = a.keys().chain(b.keys()).cloned().collect();
    keys.sort();
    keys.dedup();
    keys.into_iter().filter(|k| a.get(k) != b.get(k)).collect()
}

/// What a run of [`Checker::check_position`] found.
#[derive(Default)]
pub struct Stats {
    /// Positions checked.
    pub positions: usize,
    /// Positions every slice of which loaded.
    pub loaded: usize,
    /// Positions whose reloaded core state was compared.
    pub checked_core: usize,
    /// Refusals by reason text: count and an example label.
    pub unmapped: BTreeMap<String, (usize, String)>,
    /// `store(load(g)) != g` findings.
    pub mismatches: Vec<String>,
    /// `load(store(x)) != x` findings.
    pub core_diffs: Vec<String>,
    pub costs: [Cost; Slice::COUNT],
    /// High-water marks of the per-game tables: owners, interned texts, blobs (package D2-3 reads them).
    pub max_owners: usize,
    pub max_texts: usize,
    pub max_blobs: usize,
}

impl Stats {
    /// Whether the round trip held on every position (refusals do not count here).
    #[must_use]
    pub fn round_trip_holds(&self) -> bool {
        self.mismatches.is_empty() && self.core_diffs.is_empty()
    }

    /// Positions that were refused somewhere (not fully loaded).
    #[must_use]
    pub fn unmapped_positions(&self) -> usize {
        self.positions - self.loaded
    }

    /// Fold another run in (the alpha tool checks game after game).
    pub fn add(&mut self, other: Self) {
        self.positions += other.positions;
        self.loaded += other.loaded;
        self.checked_core += other.checked_core;
        for (k, (n, e)) in other.unmapped {
            let entry = self.unmapped.entry(k).or_default();
            entry.0 += n;
            entry.1 = e;
        }
        self.mismatches.extend(other.mismatches);
        self.core_diffs.extend(other.core_diffs);
        for (a, b) in self.costs.iter_mut().zip(other.costs) {
            a.load_ns += b.load_ns;
            a.store_ns += b.store_ns;
            a.bytes += b.bytes;
            a.n += b.n;
        }
        self.max_owners = self.max_owners.max(other.max_owners);
        self.max_texts = self.max_texts.max(other.max_texts);
        self.max_blobs = self.max_blobs.max(other.max_blobs);
    }
}

/// Checks positions one after another. The store target of each check is the previous position, so a field no
/// codec writes would keep that position's value and show as a mismatch.
#[derive(Default)]
pub struct Checker {
    pub stats: Stats,
    previous: Option<GameState>,
}

impl Checker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget the previous position (a new game starts: the store target is then the position itself).
    pub fn new_game(&mut self) {
        self.previous = None;
    }

    /// The round trip of one position. `galaxy` feeds the map slice (it lives outside `GameState`).
    pub fn check_position(&mut self, label: &str, state: &GameState, galaxy: &Galaxy) {
        let stats = &mut self.stats;
        stats.positions += 1;
        let mut cx = match Ctx::of(state) {
            Ok(cx) => cx,
            Err(e) => {
                let entry = stats.unmapped.entry(format!("{e}")).or_default();
                entry.0 += 1;
                entry.1 = label.to_owned();
                return;
            }
        };
        let src = Sources { galaxy: Some(galaxy), rng: None };
        let mut core = CoreState::with_loaded(SliceSet::EMPTY);
        cx.attach(&mut core);
        let mut ok = true;
        let value = json(state);
        for slice in Slice::ALL {
            let t = Instant::now();
            let r = Codec::load_slice(&mut cx, state, src, slice, &mut core);
            let ns = t.elapsed().as_nanos();
            match r {
                Ok(()) => {
                    let c = &mut stats.costs[slice as usize];
                    c.load_ns += ns;
                    c.n += 1;
                }
                Err(e) => {
                    ok = false;
                    let entry = stats
                        .unmapped
                        .entry(format!("{:?}: {} ({})", e.slice, e.what, e.text.chars().take(24).collect::<String>()))
                        .or_default();
                    entry.0 += 1;
                    entry.1 = label.to_owned();
                    break;
                }
            }
        }
        if let Value::Object(map) = &value {
            for (k, v) in map {
                stats.costs[slice_of_key(k) as usize].bytes += serde_json::to_vec(v).map_or(0, |b| b.len() as u128);
            }
        }
        if !ok {
            self.previous = Some(state.clone());
            return;
        }
        stats.loaded += 1;
        stats.max_owners = stats.max_owners.max(cx.owners.len());
        // The tables travel with the state (D2-3): the lineage adopts them, and the high-water marks are the state's.
        cx.adopt(&core);
        stats.max_texts = stats.max_texts.max(core.texts.codes.len());
        stats.max_blobs = stats.max_blobs.max(core.texts.blobs.len());
        let mut back = self.previous.clone().unwrap_or_else(|| state.clone());
        for slice in Slice::ALL {
            let t = Instant::now();
            let r = Codec::store_slice(&cx, &core, &mut back, slice);
            stats.costs[slice as usize].store_ns += t.elapsed().as_nanos();
            if let Err(e) = r {
                stats.mismatches.push(format!("{label}: store {e}"));
            }
        }
        let back_value = json(&back);
        if back_value != value {
            let keys = differing_keys(&value, &back_value);
            stats.mismatches.push(format!("{label}: bytes differ in {}", keys.join(", ")));
        } else if serde_json::to_vec(&back).ok() != serde_json::to_vec(state).ok() {
            stats.mismatches.push(format!("{label}: serialised bytes differ (same JSON value)"));
        }
        // load(store(x)) == x, with the same tables (the stored state has the same owners).
        match Codec::load(&mut cx, &back, src, SliceSet::ALL) {
            Ok(again) => {
                stats.checked_core += 1;
                if again != core {
                    stats.core_diffs.push(label.to_owned());
                }
            }
            Err(e) => stats.core_diffs.push(format!("{label}: reload {e}")),
        }
        self.previous = Some(state.clone());
    }
}

/// `size_of` the typed slice, for the lend cost table.
#[must_use]
pub fn inline_size(s: Slice) -> usize {
    use std::mem::size_of;
    use ti4_engine_core::state as st;
    match s {
        Slice::Meta => size_of::<st::Meta>(),
        Slice::Seats => size_of::<st::Seats>(),
        Slice::Players => size_of::<st::Players>(),
        Slice::Board => size_of::<st::Board>(),
        Slice::Map => size_of::<st::MapSlice>(),
        Slice::Decks => size_of::<st::Decks>(),
        Slice::Cards => size_of::<st::CardZones>(),
        Slice::Objectives => size_of::<st::Objectives>(),
        Slice::Agenda => size_of::<st::AgendaState>(),
        Slice::Strategy => size_of::<st::StrategyState>(),
        Slice::Explore => size_of::<st::ExploreState>(),
        Slice::Trade => size_of::<st::TradeState>(),
        Slice::Marks => size_of::<st::Marks>(),
        Slice::Transient => size_of::<st::Transient>(),
        Slice::Pending => size_of::<st::Pending>(),
        Slice::Rng => size_of::<st::Rng>(),
        Slice::Ledger => size_of::<st::Ledger>(),
    }
}
