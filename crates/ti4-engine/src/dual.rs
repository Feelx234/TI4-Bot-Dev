//! The dual run (docs/ENGINE_CORE_STATE.md, 3.3): per area, the string implementation, the core
//! implementation, or both with every answer compared.
//!
//! * [`Area`] names a ported area; [`AreaMask`] says which [`Backend`] each one runs on. The
//!   default comes from the cargo features (`string-movement`, `typed-core`); tests and binaries
//!   override it for the whole process ([`set_default_mask`]) or one thread ([`with_mask`]); a
//!   native process reads `TI4_AREAS` once (`movement=dual,combat=core`, `all=string`).
//! * [`dual!`] / [`dual_query!`] are the call-site forms. In `Dual` they run both sides, compare
//!   (state, returned value, decisions and their V1/V2 hashes, events, rng positions, dice) and
//!   return the **string** result, so a bug in the port cannot change the game under test.
//! * Differences go into the process-wide [`DualReport`] with the JSON path of the first differing
//!   leaf ([`diff_json`]); with [`set_fail_fast`] the first one panics.
//!
//! Nothing here depends on core types: the harness compares the string engine's own values.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt::{self, Debug, Write as _};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use serde_json::Value;
use ti4_model::state::GameState;

use crate::choice::{Choice, DecisionRecord};
use crate::dice::Roll;
use crate::event::Event;
use crate::fingerprint::{CanonicalHashVersion, decision_hash, event_hash};
use crate::rng::{GameRng, RngPositions};

// --- areas and backends ------------------------------------------------------------------------

/// One ported area (one per package of 4.4; a package adds no variant of its own, it uses its row).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Area {
    /// A2, and the stage 1-5 prototype: reach, routes, adjacency, neighbours, planet and system
    /// content answered by the core.
    Movement,
    BoardQueries,
    Technology,
    Objectives,
    Kernel,
    Combat,
    Invasion,
    Production,
    TradeDiplomacy,
    ExplorationRelics,
    ActionCards,
    LeadersAbilities,
    Strategy,
    AgendaLaws,
    Factions,
    Driver,
}

impl Area {
    pub const ALL: [Self; 16] = [
        Self::Movement,
        Self::BoardQueries,
        Self::Technology,
        Self::Objectives,
        Self::Kernel,
        Self::Combat,
        Self::Invasion,
        Self::Production,
        Self::TradeDiplomacy,
        Self::ExplorationRelics,
        Self::ActionCards,
        Self::LeadersAbilities,
        Self::Strategy,
        Self::AgendaLaws,
        Self::Factions,
        Self::Driver,
    ];

    /// The text of `TI4_AREAS` and of the report.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Movement => "movement",
            Self::BoardQueries => "board",
            Self::Technology => "technology",
            Self::Objectives => "objectives",
            Self::Kernel => "kernel",
            Self::Combat => "combat",
            Self::Invasion => "invasion",
            Self::Production => "production",
            Self::TradeDiplomacy => "trade",
            Self::ExplorationRelics => "exploration",
            Self::ActionCards => "action_cards",
            Self::LeadersAbilities => "leaders",
            Self::Strategy => "strategy",
            Self::AgendaLaws => "agenda",
            Self::Factions => "factions",
            Self::Driver => "driver",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.name() == name)
    }

    const fn bit(self) -> u32 {
        1 << (self as u8)
    }
}

/// Which implementation answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    String,
    Core,
    /// Both, compared; the string result is returned.
    Dual,
}

/// The backend of every area: `Dual` bit, else `Core` bit, else `String`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AreaMask {
    core: u32,
    dual: u32,
}

impl AreaMask {
    /// Every area on the string implementation.
    pub const STRING: Self = Self { core: 0, dual: 0 };

    /// Every area on `backend`.
    #[must_use]
    pub fn all(backend: Backend) -> Self {
        Area::ALL.into_iter().fold(Self::STRING, |m, a| m.with(a, backend))
    }

    #[must_use]
    pub const fn with(mut self, area: Area, backend: Backend) -> Self {
        let bit = area.bit();
        self.core &= !bit;
        self.dual &= !bit;
        match backend {
            Backend::String => {}
            Backend::Core => self.core |= bit,
            Backend::Dual => self.dual |= bit,
        }
        self
    }

    #[must_use]
    pub const fn backend(self, area: Area) -> Backend {
        let bit = area.bit();
        if self.dual & bit != 0 {
            Backend::Dual
        } else if self.core & bit != 0 {
            Backend::Core
        } else {
            Backend::String
        }
    }

    /// The build's default: every merged area on `Core` unless its `string-<area>` feature forces
    /// the string path; without `typed-core` nothing is compiled for `Core`.
    #[must_use]
    pub const fn build_default() -> Self {
        let mut mask = Self::STRING;
        if cfg!(all(feature = "typed-core", not(feature = "string-movement"))) {
            mask = mask.with(Area::Movement, Backend::Core);
        }
        if cfg!(all(feature = "typed-core", not(feature = "string-exploration"))) {
            mask = mask.with(Area::ExplorationRelics, Backend::Core);
        }
        mask
    }

    /// Parse `area=backend,area=backend`; `all` names every area. Later entries win.
    ///
    /// # Errors
    /// An unknown area or backend name.
    pub fn parse(spec: &str, mut base: Self) -> Result<Self, String> {
        for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (area, backend) = part.split_once('=').ok_or_else(|| format!("`{part}`: want area=backend"))?;
            let backend = match backend.trim() {
                "string" => Backend::String,
                "core" => Backend::Core,
                "dual" => Backend::Dual,
                other => return Err(format!("unknown backend `{other}`")),
            };
            if area.trim() == "all" {
                base = Self::all(backend);
            } else {
                let area = Area::from_name(area.trim()).ok_or_else(|| format!("unknown area `{area}`"))?;
                base = base.with(area, backend);
            }
        }
        Ok(base)
    }
}

static DEFAULT_CORE: AtomicU64 = AtomicU64::new(u64::MAX);
static INIT: OnceLock<()> = OnceLock::new();

thread_local! {
    static OVERRIDE: Cell<Option<AreaMask>> = const { Cell::new(None) };
}

fn pack(mask: AreaMask) -> u64 {
    u64::from(mask.core) | (u64::from(mask.dual) << 32)
}

fn unpack(bits: u64) -> AreaMask {
    AreaMask { core: (bits & 0xffff_ffff) as u32, dual: (bits >> 32) as u32 }
}

fn init_default() {
    INIT.get_or_init(|| {
        let mut mask = AreaMask::build_default();
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(spec) = std::env::var("TI4_AREAS") {
            match AreaMask::parse(&spec, mask) {
                Ok(parsed) => mask = parsed,
                Err(error) => eprintln!("TI4_AREAS ignored: {error}"),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        if std::env::var("TI4_DUAL_FAIL_FAST").is_ok_and(|v| v != "0") {
            FAIL_FAST.store(true, Ordering::Relaxed);
        }
        // A binary that already called `set_default_mask` keeps its value.
        let _ = DEFAULT_CORE.compare_exchange(u64::MAX, pack(mask), Ordering::Relaxed, Ordering::Relaxed);
    });
}

/// Number of live thread overrides ([`with_mask`]); while zero, [`mask`] is two relaxed loads.
static OVERRIDES: AtomicU64 = AtomicU64::new(0);

/// The mask in force on this thread.
#[inline]
#[must_use]
pub fn mask() -> AreaMask {
    if OVERRIDES.load(Ordering::Relaxed) != 0
        && let Some(mask) = OVERRIDE.with(Cell::get)
    {
        return mask;
    }
    let bits = DEFAULT_CORE.load(Ordering::Relaxed);
    if bits != u64::MAX {
        return unpack(bits);
    }
    init_default();
    unpack(DEFAULT_CORE.load(Ordering::Relaxed))
}

/// The backend of `area` on this thread. Without `typed-core` always `String`.
#[inline]
#[must_use]
pub fn backend(area: Area) -> Backend {
    if cfg!(not(feature = "typed-core")) {
        return Backend::String;
    }
    mask().backend(area)
}

/// Set the process default (every thread without an override), e.g. from a binary's flags.
pub fn set_default_mask(mask: AreaMask) {
    INIT.get_or_init(|| ());
    DEFAULT_CORE.store(pack(mask), Ordering::Relaxed);
}

/// Run `body` with `mask` on this thread only; restored afterwards, also on panic.
pub fn with_mask<T>(mask: AreaMask, body: impl FnOnce() -> T) -> T {
    struct Restore(Option<AreaMask>);
    impl Drop for Restore {
        fn drop(&mut self) {
            OVERRIDES.fetch_sub(1, Ordering::Relaxed);
            OVERRIDE.with(|o| o.set(self.0));
        }
    }
    OVERRIDES.fetch_add(1, Ordering::Relaxed);
    let _restore = Restore(OVERRIDE.with(|o| o.replace(Some(mask))));
    body()
}

// --- the report --------------------------------------------------------------------------------

/// What was compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Category {
    /// `GameState` as JSON, leaf by leaf.
    State,
    /// `GameState` as serialised bytes (the byte-exact check).
    Bytes,
    /// Returned value or error.
    Value,
    /// A `Choice` (ids, kinds, labels, payload, prompt, context, details).
    Decision,
    /// The V1 and V2 `decision_hash` of a record built from each side.
    DecisionHash,
    /// The ordered events: structure and V1/V2 `event_hash`.
    Event,
    /// `GameRng::positions()`.
    Rng,
    /// The dice history.
    Dice,
}

impl Category {
    pub const ALL: [Self; 8] = [
        Self::State,
        Self::Bytes,
        Self::Value,
        Self::Decision,
        Self::DecisionHash,
        Self::Event,
        Self::Rng,
        Self::Dice,
    ];
}

/// One difference between the string and the core side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    pub area: Area,
    pub category: Category,
    /// The call site (`"tactical::movable_into"`).
    pub label: String,
    /// JSON path of the first differing leaf (`players[2].technologies[3]`), empty for a whole value.
    pub path: String,
    pub string_side: String,
    pub core_side: String,
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DUAL DIFFERENCE [{}/{:?}] {} at `{}`: string = {} | core = {}",
            self.area.name(),
            self.category,
            self.label,
            self.path,
            self.string_side,
            self.core_side
        )
    }
}

/// Counters of one area.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AreaCounts {
    /// Dual calls.
    pub calls: u64,
    /// Comparisons made, by category index.
    pub compared: [u64; 8],
    /// Comparisons that differed, by category index.
    pub differing: [u64; 8],
    /// Lend cost (docs 3.3 item 6): regions lent, slices, bytes, nanoseconds.
    pub lends: u64,
    pub lend_slices: u64,
    pub lend_bytes: u64,
    pub lend_ns: u64,
}

/// Everything the dual run saw in this process.
#[derive(Debug, Clone, Default)]
pub struct DualReport {
    pub areas: BTreeMap<Area, AreaCounts>,
    /// The first [`MAX_KEPT`] differences.
    pub differences: Vec<Difference>,
    /// Differences beyond [`MAX_KEPT`] (counted only).
    pub dropped: u64,
}

/// Differences kept with their detail.
pub const MAX_KEPT: usize = 200;

impl DualReport {
    #[must_use]
    pub fn total_calls(&self) -> u64 {
        self.areas.values().map(|c| c.calls).sum()
    }

    #[must_use]
    pub fn total_compared(&self) -> u64 {
        self.areas.values().flat_map(|c| c.compared).sum()
    }

    #[must_use]
    pub fn total_differences(&self) -> u64 {
        self.differences.len() as u64 + self.dropped
    }

    /// Differences found in `category`.
    #[must_use]
    pub fn differing(&self, category: Category) -> u64 {
        self.areas.values().map(|c| c.differing[category as usize]).sum()
    }

    /// Fold another report in (for runs split over processes or threads).
    pub fn add(&mut self, other: &Self) {
        for (area, counts) in &other.areas {
            let into = self.areas.entry(*area).or_default();
            into.calls += counts.calls;
            for i in 0..8 {
                into.compared[i] += counts.compared[i];
                into.differing[i] += counts.differing[i];
            }
            into.lends += counts.lends;
            into.lend_slices += counts.lend_slices;
            into.lend_bytes += counts.lend_bytes;
            into.lend_ns += counts.lend_ns;
        }
        for d in &other.differences {
            if self.differences.len() < MAX_KEPT {
                self.differences.push(d.clone());
            } else {
                self.dropped += 1;
            }
        }
        self.dropped += other.dropped;
    }
}

impl fmt::Display for DualReport {
    /// The summary the gate prints: calls and comparisons per area and category, then the differences.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "dual: {} calls, {} comparisons, {} differences",
            self.total_calls(),
            self.total_compared(),
            self.total_differences()
        )?;
        for (area, c) in &self.areas {
            write!(f, "  {:<12} calls {}", area.name(), c.calls)?;
            for cat in Category::ALL {
                let (n, d) = (c.compared[cat as usize], c.differing[cat as usize]);
                if n > 0 {
                    write!(f, " | {cat:?} {n}/{d}")?;
                }
            }
            if c.lends > 0 {
                write!(
                    f,
                    " | lends {} ({} slices, {} B, {} ns)",
                    c.lends, c.lend_slices, c.lend_bytes, c.lend_ns
                )?;
            }
            writeln!(f)?;
        }
        for d in &self.differences {
            writeln!(f, "{d}")?;
        }
        if self.dropped > 0 {
            writeln!(f, "... and {} more", self.dropped)?;
        }
        Ok(())
    }
}

static REPORT: Mutex<Option<DualReport>> = Mutex::new(None);
static FAIL_FAST: AtomicBool = AtomicBool::new(false);

fn with_report<T>(body: impl FnOnce(&mut DualReport) -> T) -> T {
    let mut guard = REPORT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    body(guard.get_or_insert_with(DualReport::default))
}

/// A copy of the process-wide report.
#[must_use]
pub fn report() -> DualReport {
    with_report(|r| r.clone())
}

/// Empty the process-wide report; returns what it held.
pub fn take_report() -> DualReport {
    with_report(std::mem::take)
}

/// The first difference panics (the engine's own test suite runs this way).
pub fn set_fail_fast(on: bool) {
    FAIL_FAST.store(on, Ordering::Relaxed);
}

/// Whether the first difference panics.
#[must_use]
pub fn fail_fast() -> bool {
    FAIL_FAST.load(Ordering::Relaxed)
}

/// Record the lend cost of one region (called by the sync layer; docs 3.3 item 6).
pub fn record_cost(area: Area, slices: u64, bytes: u64, ns: u64) {
    with_report(|r| {
        let c = r.areas.entry(area).or_default();
        c.lends += 1;
        c.lend_slices += slices;
        c.lend_bytes += bytes;
        c.lend_ns += ns;
    });
}

fn note_call(area: Area) {
    with_report(|r| r.areas.entry(area).or_default().calls += 1);
}

fn tally(area: Area, category: Category, label: &str, found: Vec<(String, String, String)>) -> bool {
    let fail = FAIL_FAST.load(Ordering::Relaxed);
    let mut first = None;
    with_report(|r| {
        let c = r.areas.entry(area).or_default();
        c.compared[category as usize] += 1;
        if !found.is_empty() {
            c.differing[category as usize] += 1;
        }
        for (path, string_side, core_side) in found {
            let d = Difference { area, category, label: label.to_owned(), path, string_side, core_side };
            if first.is_none() {
                first = Some(d.clone());
            }
            if r.differences.len() < MAX_KEPT {
                r.differences.push(d);
            } else {
                r.dropped += 1;
            }
        }
    });
    if let Some(d) = first {
        assert!(!fail, "{d}");
        return false;
    }
    true
}

// --- the JSON-path diff reporter ---------------------------------------------------------------

/// One differing leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonDiff {
    /// `players[2].technologies[3]`, `board.38.units[1].owner`; empty for the root.
    pub path: String,
    pub left: String,
    pub right: String,
}

const ABSENT: &str = "<absent>";

fn shown(v: &Value) -> String {
    let mut s = v.to_string();
    if s.len() > 200 {
        s.truncate(200);
        s.push_str("...");
    }
    s
}

/// Every leaf at which `left` and `right` differ (at most `limit`), with its path. An array of
/// different length reports the shared prefix's differences and each extra element; an object key
/// on one side only is reported with `<absent>` on the other.
#[must_use]
pub fn diff_json(left: &Value, right: &Value, limit: usize) -> Vec<JsonDiff> {
    fn walk(path: &mut String, l: &Value, r: &Value, out: &mut Vec<JsonDiff>, limit: usize) {
        if out.len() >= limit {
            return;
        }
        match (l, r) {
            (Value::Object(a), Value::Object(b)) => {
                for (k, av) in a {
                    let keep = path.len();
                    if !path.is_empty() {
                        path.push('.');
                    }
                    path.push_str(k);
                    match b.get(k) {
                        Some(bv) => walk(path, av, bv, out, limit),
                        None if out.len() < limit => out.push(JsonDiff {
                            path: path.clone(),
                            left: shown(av),
                            right: ABSENT.into(),
                        }),
                        None => {}
                    }
                    path.truncate(keep);
                }
                for (k, bv) in b {
                    if !a.contains_key(k) && out.len() < limit {
                        let keep = path.len();
                        if !path.is_empty() {
                            path.push('.');
                        }
                        path.push_str(k);
                        out.push(JsonDiff { path: path.clone(), left: ABSENT.into(), right: shown(bv) });
                        path.truncate(keep);
                    }
                }
            }
            (Value::Array(a), Value::Array(b)) => {
                for i in 0..a.len().max(b.len()) {
                    let keep = path.len();
                    let _ = write!(path, "[{i}]");
                    match (a.get(i), b.get(i)) {
                        (Some(av), Some(bv)) => walk(path, av, bv, out, limit),
                        (Some(av), None) if out.len() < limit => {
                            out.push(JsonDiff { path: path.clone(), left: shown(av), right: ABSENT.into() });
                        }
                        (None, Some(bv)) if out.len() < limit => {
                            out.push(JsonDiff { path: path.clone(), left: ABSENT.into(), right: shown(bv) });
                        }
                        _ => {}
                    }
                    path.truncate(keep);
                }
            }
            _ if l == r => {}
            _ => out.push(JsonDiff { path: path.clone(), left: shown(l), right: shown(r) }),
        }
    }
    let mut out = Vec::new();
    walk(&mut String::new(), left, right, &mut out, limit);
    out
}

fn found(diffs: Vec<JsonDiff>) -> Vec<(String, String, String)> {
    diffs.into_iter().map(|d| (d.path, d.left, d.right)).collect()
}

/// Leaves reported per comparison.
const LEAVES: usize = 8;

// --- the comparisons ---------------------------------------------------------------------------

/// State, field by field (`serde_json::to_value`), then the serialised bytes. Returns whether equal.
pub fn compare_state(area: Area, label: &str, string_side: &GameState, core_side: &GameState) -> bool {
    let a = serde_json::to_value(string_side).unwrap_or(Value::Null);
    let b = serde_json::to_value(core_side).unwrap_or(Value::Null);
    let mut ok = tally(area, Category::State, label, found(diff_json(&a, &b, LEAVES)));
    let bytes = serde_json::to_vec(string_side).ok() == serde_json::to_vec(core_side).ok();
    let found_bytes = if bytes {
        Vec::new()
    } else {
        vec![("<bytes>".into(), "serialised bytes differ".into(), String::new())]
    };
    // The leaf comparison already reported a visible difference; the bytes catch the rest.
    ok &= tally(area, Category::Bytes, label, if ok { found_bytes } else { Vec::new() });
    ok
}

/// Returned value or error, by `PartialEq`, reported by `Debug`.
pub fn compare_value<T: PartialEq + Debug>(area: Area, label: &str, string_side: &T, core_side: &T) -> bool {
    let found = if string_side == core_side {
        Vec::new()
    } else {
        vec![(String::new(), format!("{string_side:?}"), format!("{core_side:?}"))]
    };
    tally(area, Category::Value, label, found)
}

/// A value by its JSON form: reported by leaf path.
pub fn compare_json<T: Serialize>(
    area: Area,
    category: Category,
    label: &str,
    string_side: &T,
    core_side: &T,
) -> bool {
    let a = serde_json::to_value(string_side).unwrap_or(Value::Null);
    let b = serde_json::to_value(core_side).unwrap_or(Value::Null);
    tally(area, category, label, found(diff_json(&a, &b, LEAVES)))
}

/// The record a decision opened by `choice` would leave once `chosen` is picked.
#[must_use]
pub fn record_of(choice: &Choice, chosen: &str) -> DecisionRecord {
    DecisionRecord::from(crate::choice::DecisionRecordFields {
        player: choice.player.clone(),
        prompt: choice.prompt.clone(),
        chosen: chosen.to_owned(),
        offered: choice.options.iter().map(|o| o.id.clone()).collect(),
        context: choice.context.clone(),
    })
}

/// An opened decision (`None`: the call opened none) on both sides: the whole `Choice` leaf by leaf
/// (ids, kinds, labels, payload JSON, prompt, context incl. its canonical text, details), the
/// `canonical()` of the contexts, and the V1 and V2 `decision_hash` of a record built from each.
pub fn compare_choice(area: Area, label: &str, string_side: Option<&Choice>, core_side: Option<&Choice>) -> bool {
    let a = serde_json::to_value(string_side).unwrap_or(Value::Null);
    let b = serde_json::to_value(core_side).unwrap_or(Value::Null);
    let mut diffs = found(diff_json(&a, &b, LEAVES));
    // `ChoiceOption::eq` ignores the payload and preview; the JSON above has the payload.
    if let (Some(x), Some(y)) = (string_side, core_side) {
        if x.options != y.options {
            diffs.push((String::new(), "options differ".into(), "options differ".into()));
        }
        let (cx, cy) = (x.context.as_ref().map(|c| c.canonical()), y.context.as_ref().map(|c| c.canonical()));
        if cx != cy {
            diffs.push(("context.canonical()".into(), format!("{cx:?}"), format!("{cy:?}")));
        }
    }
    let ok = tally(area, Category::Decision, label, diffs);
    let hashes = match (string_side, core_side) {
        (Some(x), Some(y)) => compare_records(area, label, &record_of(x, ""), &record_of(y, "")),
        _ => true,
    };
    ok && hashes
}

/// The V1 and V2 `decision_hash` of two records (answered decisions).
pub fn compare_records(area: Area, label: &str, string_side: &DecisionRecord, core_side: &DecisionRecord) -> bool {
    let mut diffs = Vec::new();
    for version in [CanonicalHashVersion::V1, CanonicalHashVersion::V2] {
        let (x, y) = (decision_hash(version, string_side), decision_hash(version, core_side));
        if x != y {
            diffs.push((format!("v{}", version.get()), x.digest, y.digest));
        }
    }
    tally(area, Category::DecisionHash, label, diffs)
}

/// The ordered event lists: length, each event leaf by leaf, and the V1 and V2 `event_hash`.
pub fn compare_events(area: Area, label: &str, string_side: &[Event], core_side: &[Event]) -> bool {
    let a = serde_json::to_value(string_side).unwrap_or(Value::Null);
    let b = serde_json::to_value(core_side).unwrap_or(Value::Null);
    let mut diffs = found(diff_json(&a, &b, LEAVES));
    for (i, (x, y)) in string_side.iter().zip(core_side).enumerate() {
        for version in [CanonicalHashVersion::V1, CanonicalHashVersion::V2] {
            let (hx, hy) = (event_hash(version, x), event_hash(version, y));
            if hx != hy && diffs.len() < LEAVES * 2 {
                diffs.push((format!("[{i}].hash.v{}", version.get()), hx.digest, hy.digest));
            }
        }
    }
    tally(area, Category::Event, label, diffs)
}

/// Every stream's word position: equal positions mean the same draws in the same order.
pub fn compare_rng(area: Area, label: &str, string_side: &GameRng, core_side: &GameRng) -> bool {
    compare_positions(area, label, &string_side.positions(), &core_side.positions())
}

/// [`compare_rng`] on already-read positions.
pub fn compare_positions(area: Area, label: &str, string_side: &RngPositions, core_side: &RngPositions) -> bool {
    let diffs = if string_side == core_side {
        Vec::new()
    } else {
        let a: Value = string_side.iter().map(|(k, v)| (k.clone(), Value::String(v.to_string()))).collect();
        let b: Value = core_side.iter().map(|(k, v)| (k.clone(), Value::String(v.to_string()))).collect();
        found(diff_json(&a, &b, LEAVES))
    };
    tally(area, Category::Rng, label, diffs)
}

/// The dice history, roll by roll.
pub fn compare_dice(area: Area, label: &str, string_side: &[Roll], core_side: &[Roll]) -> bool {
    compare_json(area, Category::Dice, label, &string_side, &core_side)
}

// --- the call-site forms -----------------------------------------------------------------------

/// What one side of a stateful call produced beyond the state: the value and whatever the call
/// opens or draws. Build with [`Run::new`] and the `with_*` setters; unset parts are not compared.
#[derive(Debug, Clone)]
pub struct Run<R> {
    pub value: R,
    pub choice: Option<Option<Choice>>,
    pub events: Option<Vec<Event>>,
    pub rng: Option<RngPositions>,
    pub dice: Option<Vec<Roll>>,
    pub record: Option<DecisionRecord>,
}

impl<R> Run<R> {
    #[must_use]
    pub const fn new(value: R) -> Self {
        Self { value, choice: None, events: None, rng: None, dice: None, record: None }
    }

    /// The decision this call opened (`None`: it opened none).
    #[must_use]
    pub fn with_choice(mut self, choice: Option<Choice>) -> Self {
        self.choice = Some(choice);
        self
    }

    #[must_use]
    pub fn with_events(mut self, events: &[Event]) -> Self {
        self.events = Some(events.to_vec());
        self
    }

    #[must_use]
    pub fn with_rng(mut self, rng: &GameRng) -> Self {
        self.rng = Some(rng.positions());
        self
    }

    #[must_use]
    pub fn with_dice(mut self, dice: &[Roll]) -> Self {
        self.dice = Some(dice.to_vec());
        self
    }

    /// The record of the decision this call answered.
    #[must_use]
    pub fn with_record(mut self, record: DecisionRecord) -> Self {
        self.record = Some(record);
        self
    }
}

/// Compare two runs (and the states they leave); returns whether everything was equal.
pub fn compare_runs<R: PartialEq + Debug>(
    area: Area,
    label: &str,
    string_state: &GameState,
    string_run: &Run<R>,
    core_state: &GameState,
    core_run: &Run<R>,
) -> bool {
    note_call(area);
    let mut ok = compare_value(area, label, &string_run.value, &core_run.value);
    ok &= compare_state(area, label, string_state, core_state);
    if let (Some(a), Some(b)) = (&string_run.choice, &core_run.choice) {
        ok &= compare_choice(area, label, a.as_ref(), b.as_ref());
    }
    if let (Some(a), Some(b)) = (&string_run.record, &core_run.record) {
        ok &= compare_records(area, label, a, b);
    }
    if let (Some(a), Some(b)) = (&string_run.events, &core_run.events) {
        ok &= compare_events(area, label, a, b);
    }
    if let (Some(a), Some(b)) = (&string_run.rng, &core_run.rng) {
        ok &= compare_positions(area, label, a, b);
    }
    if let (Some(a), Some(b)) = (&string_run.dice, &core_run.dice) {
        ok &= compare_dice(area, label, a, b);
    }
    ok
}

/// The `Dual` path of [`dual!`]: both closures on their own copy of `state`; the string side's
/// state and value are kept.
pub fn run_dual<R: PartialEq + Debug>(
    area: Area,
    label: &str,
    state: &mut GameState,
    string: impl FnOnce(&mut GameState) -> Run<R>,
    core: impl FnOnce(&mut GameState) -> Run<R>,
) -> R {
    let mut core_state = state.clone();
    let string_run = string(state);
    let core_run = core(&mut core_state);
    compare_runs(area, label, state, &string_run, &core_state, &core_run);
    string_run.value
}

/// The `Dual` path of [`dual_query!`]: a read-only call compared by its returned value.
pub fn run_dual_query<R: PartialEq + Debug>(
    area: Area,
    label: &str,
    string: impl FnOnce() -> R,
    core: impl FnOnce() -> R,
) -> R {
    let s = string();
    let c = core();
    note_call(area);
    compare_value(area, label, &s, &c);
    s
}

/// The `Dual` path of [`dual_typed!`]: the string answer, compared with the typed one when it has
/// one (`None`: the core could not express the question and the string answer stands).
pub fn run_dual_typed<R: PartialEq + Debug>(
    area: Area,
    label: &str,
    typed: impl FnOnce() -> Option<R>,
    string: impl FnOnce() -> R,
) -> R {
    let s = string();
    if let Some(c) = typed() {
        note_call(area);
        compare_value(area, label, &s, &c);
    }
    s
}

/// A call site where the core may decline (`None` = not mapped, fall back to the string side):
/// `dual_typed!(Area::X, "label", typed_expr_option, string_expr)`. Each expression is evaluated
/// only on its backend; `Dual` evaluates both and compares when the core answered.
#[macro_export]
macro_rules! dual_typed {
    ($area:expr, $label:expr, $typed:expr, $string:expr) => {
        match $crate::dual::backend($area) {
            $crate::dual::Backend::String => $string,
            $crate::dual::Backend::Core => ($typed).unwrap_or_else(|| $string),
            $crate::dual::Backend::Dual => {
                $crate::dual::run_dual_typed($area, $label, || $typed, || $string)
            }
        }
    };
}

/// A stateful call site: `dual!(Area::X, "label", state, |s| string side, |s| core side)`, both
/// closures taking `&mut GameState` and returning a [`Run`]. `String` and `Core` run one closure;
/// `Dual` runs both on copies and returns the string result.
#[macro_export]
macro_rules! dual {
    ($area:expr, $label:expr, $state:expr, $string:expr, $core:expr) => {
        match $crate::dual::backend($area) {
            $crate::dual::Backend::String => ($string)($state).value,
            $crate::dual::Backend::Core => ($core)($state).value,
            $crate::dual::Backend::Dual => {
                $crate::dual::run_dual($area, $label, $state, $string, $core)
            }
        }
    };
}

/// A read-only call site: `dual_query!(Area::X, "label", string_expr, core_expr)`; the expressions
/// are evaluated only on their backend (both in `Dual`, compared by `PartialEq`).
#[macro_export]
macro_rules! dual_query {
    ($area:expr, $label:expr, $string:expr, $core:expr) => {
        match $crate::dual::backend($area) {
            $crate::dual::Backend::String => $string,
            $crate::dual::Backend::Core => $core,
            $crate::dual::Backend::Dual => {
                $crate::dual::run_dual_query($area, $label, || $string, || $core)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::choice::ChoiceOption;

    /// The report is process-wide: tests that read it hold this lock.
    static SERIAL: Mutex<()> = Mutex::new(());

    /// Holds the report lock and turns fail-fast off (the tests inject differences on purpose, also
    /// when the suite runs with `TI4_DUAL_FAIL_FAST`); restored on drop.
    struct Serial(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>, bool);

    impl Drop for Serial {
        fn drop(&mut self) {
            set_fail_fast(self.1);
        }
    }

    fn serial() -> Serial {
        let guard = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let before = fail_fast();
        set_fail_fast(false);
        Serial(guard, before)
    }

    #[test]
    fn mask_parses_and_resolves() {
        let m = AreaMask::parse("all=core, movement=dual, combat=string", AreaMask::STRING).unwrap();
        assert_eq!(m.backend(Area::Movement), Backend::Dual);
        assert_eq!(m.backend(Area::Combat), Backend::String);
        assert_eq!(m.backend(Area::Driver), Backend::Core);
        assert!(AreaMask::parse("nope=core", AreaMask::STRING).is_err());
        assert_eq!(with_mask(AreaMask::all(Backend::Dual), || mask().backend(Area::Kernel)), Backend::Dual);
    }

    #[test]
    fn diff_paths() {
        let a = json!({"players": [{"tech": [1, 2]}, {"tech": [3, 4]}], "board": {"38": {"units": [{"owner": "a"}, {"owner": "b"}]}}, "x": 1});
        let mut b = a.clone();
        b["board"]["38"]["units"][1]["owner"] = json!("c");
        b["players"][1]["tech"][0] = json!(9);
        b["y"] = json!(true);
        let d = diff_json(&a, &b, 10);
        let paths: Vec<_> = d.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(paths, ["board.38.units[1].owner", "players[1].tech[0]", "y"]);
        assert_eq!(d[0].left, "\"b\"");
        assert_eq!(d[2].left, ABSENT);
    }

    fn state() -> GameState {
        crate::fixtures::game(&["p1", "p2"])
    }

    fn choice(label: &str, id: &str) -> Choice {
        let option = ChoiceOption {
            id: id.into(),
            kind: "k".into(),
            label: label.into(),
            payload: BTreeMap::new(),
            preview: None,
            auto_resolved: false,
        };
        Choice {
            player: ti4_model::id::PlayerId::new("p1"),
            prompt: "pick".into(),
            options: vec![option],
            context: None,
            details: serde_json::Map::new(),
            free_form: None,
        }
    }

    fn event(payload: i64) -> Event {
        Event::new(1, "T", BTreeMap::from([("n".to_owned(), json!(payload))]))
    }

    /// Identical sides compare clean; one injected difference per category is caught.
    #[test]
    fn detects_an_injected_difference_in_each_category() {
        let _guard = serial();
        let _ = take_report();
        let area = Area::BoardQueries;
        let s = state();
        // clean
        assert!(compare_state(area, "t", &s, &s.clone()));
        assert!(compare_value(area, "t", &1, &1));
        assert!(compare_choice(area, "t", Some(&choice("a", "x")), Some(&choice("a", "x"))));
        assert!(compare_events(area, "t", &[event(1)], &[event(1)]));
        assert!(compare_rng(area, "t", &GameRng::new(7), &GameRng::new(7)));
        assert_eq!(report().total_differences(), 0);

        // state
        let mut t = s.clone();
        t.players[0].trade_goods += 1;
        assert!(!compare_state(area, "t", &s, &t));
        assert_eq!(report().differing(Category::State), 1);
        assert!(report().differences[0].path.starts_with("players[0]"), "{}", report().differences[0]);
        // value
        assert!(!compare_value(area, "t", &1, &2));
        // decision: a label (visible, not hashed), then an id (hashed, V1 and V2)
        assert!(!compare_choice(area, "t", Some(&choice("a", "x")), Some(&choice("b", "x"))));
        assert_eq!(report().differing(Category::DecisionHash), 0);
        assert!(!compare_choice(area, "t", Some(&choice("a", "x")), Some(&choice("a", "y"))));
        assert_eq!(report().differing(Category::DecisionHash), 1);
        // event
        assert!(!compare_events(area, "t", &[event(1)], &[event(2)]));
        // rng
        let mut drawn = GameRng::new(7);
        let _ = drawn.die("dice", 10);
        assert!(!compare_rng(area, "t", &GameRng::new(7), &drawn));
        // dice
        assert!(compare_dice(area, "t", &[], &[]));
        for c in [Category::State, Category::Value, Category::Decision, Category::DecisionHash, Category::Event, Category::Rng] {
            assert!(report().differing(c) >= 1, "{c:?} not detected");
        }
        let _ = take_report();
    }

    #[test]
    fn macros_return_the_string_side_and_count() {
        let _guard = serial();
        let _ = take_report();
        let both = AreaMask::all(Backend::Dual);
        let mut s = state();
        let out = with_mask(both, || {
            dual!(
                Area::Combat,
                "t::mutate",
                &mut s,
                |g: &mut GameState| {
                    g.players[0].trade_goods += 1;
                    Run::new(1)
                },
                |g: &mut GameState| {
                    g.players[0].trade_goods += 1;
                    Run::new(1)
                }
            )
        });
        assert_eq!(out, 1);
        let q = with_mask(both, || dual_query!(Area::Combat, "t::q", 5, 5));
        assert_eq!(q, 5);
        assert_eq!(report().total_differences(), 0);
        assert!(report().areas[&Area::Combat].calls >= 2);

        // A bug on the core side leaves the game on the string result.
        let before = s.players[0].trade_goods;
        let out = with_mask(both, || {
            dual!(
                Area::Combat,
                "t::bug",
                &mut s,
                |g: &mut GameState| {
                    g.players[0].trade_goods += 1;
                    Run::new(1)
                },
                |g: &mut GameState| {
                    g.players[0].trade_goods += 5;
                    Run::new(2)
                }
            )
        });
        assert_eq!(out, 1);
        assert_eq!(s.players[0].trade_goods, before + 1);
        assert_eq!(report().total_differences(), 2);
        let _ = take_report();
    }
}
