//! Shards of the golden dual run (`dual_run --shard i/N`, `dual_run --merge FILE...`).
//!
//! A shard plays the games whose index is `i` modulo `N`; for every game it keeps the dual run's
//! [`DualReport`], the decision hash of the dual run and the decision hash of the string run. The shard file is
//! JSON. The merge checks that the files agree on config, mask and game count, that each shard appears once
//! and that together they cover every game exactly once; it then folds the per-game reports in game order
//! (so the first differences kept and the count of dropped ones are those of one process playing the games in
//! order) and prints what the unsharded `dual_run` prints. Hashes are compared per game, as unsharded.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use ti4_engine::dual::{Area, AreaCounts, Category, Difference, DualReport, HookCounts, HookGroup, WindowCounts};

/// The file format tag.
pub const FORMAT: &str = "ti4-dual-shard-v1";

/// A shard selector `i/N`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shard {
    pub index: usize,
    pub of: usize,
}

impl Shard {
    /// Parse `i/N` (`0 <= i < N`).
    ///
    /// # Errors
    /// Anything else.
    pub fn parse(text: &str) -> Result<Self, String> {
        let bad = || format!("shard `{text}`: expected i/N with 0 <= i < N");
        let (i, n) = text.split_once('/').ok_or_else(bad)?;
        let (index, of) = (i.trim().parse::<usize>().map_err(|_| bad())?, n.trim().parse::<usize>().map_err(|_| bad())?);
        if of == 0 || index >= of {
            return Err(bad());
        }
        Ok(Self { index, of })
    }

    /// Whether this shard plays game `game`.
    #[must_use]
    pub fn includes(self, game: usize) -> bool {
        game % self.of == self.index
    }
}

/// One game of a shard.
#[derive(Debug, Clone)]
pub struct GameRecord {
    pub index: usize,
    pub dual_hash: String,
    pub string_hash: String,
    pub report: DualReport,
    /// Why the scenario was not played (the `oracle` skips scenarios of a switched-off feature); `None` for a played game.
    /// Written only when set, so the files of `dual_run` keep their bytes.
    pub skipped: Option<String>,
}

/// A shard file.
#[derive(Debug, Clone)]
pub struct ShardFile {
    pub config: String,
    /// SHA-256 of the config file's bytes (shards of different configs must not merge).
    pub config_hash: String,
    /// The games the run covers (index 0..games).
    pub games: usize,
    /// The area spec as given on the command line.
    pub spec: String,
    pub areas_on_dual: Vec<String>,
    pub shard: Shard,
    pub records: Vec<GameRecord>,
}

/// SHA-256 hex of a file.
///
/// # Errors
/// An unreadable file.
pub fn file_hash(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

// --- DualReport <-> JSON --------------------------------------------------------------------------------

fn category_from_name(name: &str) -> Option<Category> {
    Category::ALL.into_iter().find(|c| format!("{c:?}") == name)
}

/// The report as JSON (every field of [`DualReport`]).
#[must_use]
pub fn report_to_json(r: &DualReport) -> Value {
    let areas: Map<String, Value> = r
        .areas
        .iter()
        .map(|(a, c)| {
            (
                a.name().to_owned(),
                json!({
                    "calls": c.calls, "compared": c.compared, "differing": c.differing,
                    "lends": c.lends, "lend_slices": c.lend_slices, "lend_bytes": c.lend_bytes, "lend_ns": c.lend_ns,
                }),
            )
        })
        .collect();
    let hooks: Map<String, Value> = r
        .hooks
        .iter()
        .map(|(slot, h)| {
            ((*slot).to_owned(), json!({ "group": h.group.map(HookGroup::name), "calls": h.calls, "differing": h.differing }))
        })
        .collect();
    let windows: Map<String, Value> =
        r.windows.iter().map(|(l, w)| (l.clone(), json!({ "compared": w.compared, "bailed": w.bailed }))).collect();
    let nested: Map<String, Value> = r.nested.iter().map(|(k, n)| (k.clone(), json!(n))).collect();
    let plain: Map<String, Value> = r.plain.iter().map(|(k, n)| (k.clone(), json!(n))).collect();
    let unmapped: Map<String, Value> = r.unmapped.iter().map(|(k, n)| (k.clone(), json!(n))).collect();
    let differences: Vec<Value> = r
        .differences
        .iter()
        .map(|d| {
            json!({
                "area": d.area.name(), "category": format!("{:?}", d.category), "label": d.label, "path": d.path,
                "string_side": d.string_side, "core_side": d.core_side,
            })
        })
        .collect();
    json!({ "areas": areas, "hooks": hooks, "windows": windows, "nested": nested, "plain": plain, "unmapped": unmapped, "differences": differences, "dropped": r.dropped })
}

fn u64_of(v: &Value, what: &str) -> Result<u64, String> {
    v.as_u64().ok_or_else(|| format!("{what}: not an integer"))
}

fn str_of<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v.get(key).and_then(Value::as_str).ok_or_else(|| format!("missing string `{key}`"))
}

fn u64_at(v: &Value, key: &str) -> Result<u64, String> {
    u64_of(v.get(key).ok_or_else(|| format!("missing `{key}`"))?, key)
}

fn object_at<'a>(v: &'a Value, key: &str) -> Result<&'a Map<String, Value>, String> {
    v.get(key).and_then(Value::as_object).ok_or_else(|| format!("missing object `{key}`"))
}

fn counts_array(v: &Value, key: &str) -> Result<[u64; 8], String> {
    let items = v.get(key).and_then(Value::as_array).filter(|a| a.len() == 8).ok_or_else(|| format!("`{key}`: expected 8 numbers"))?;
    let mut out = [0; 8];
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = u64_of(item, key)?;
    }
    Ok(out)
}

/// The report back from [`report_to_json`].
///
/// # Errors
/// A document that is not a report.
pub fn report_from_json(v: &Value) -> Result<DualReport, String> {
    let mut r = DualReport::default();
    for (name, c) in object_at(v, "areas")? {
        let area = Area::from_name(name).ok_or_else(|| format!("unknown area `{name}`"))?;
        r.areas.insert(
            area,
            AreaCounts {
                calls: u64_at(c, "calls")?,
                compared: counts_array(c, "compared")?,
                differing: counts_array(c, "differing")?,
                lends: u64_at(c, "lends")?,
                lend_slices: u64_at(c, "lend_slices")?,
                lend_bytes: u64_at(c, "lend_bytes")?,
                lend_ns: u64_at(c, "lend_ns")?,
            },
        );
    }
    for (slot, h) in object_at(v, "hooks")? {
        let group = match h.get("group") {
            Some(Value::String(g)) => Some(HookGroup::from_name(g).ok_or_else(|| format!("unknown hook group `{g}`"))?),
            _ => None,
        };
        // A tool process: the few slot names live as long as it does.
        let slot: &'static str = Box::leak(slot.clone().into_boxed_str());
        r.hooks.insert(slot, HookCounts { group, calls: u64_at(h, "calls")?, differing: u64_at(h, "differing")? });
    }
    for (label, w) in object_at(v, "windows")? {
        r.windows.insert(label.clone(), WindowCounts { compared: u64_at(w, "compared")?, bailed: u64_at(w, "bailed")? });
    }
    for (sig, n) in object_at(v, "nested")? {
        r.nested.insert(sig.clone(), u64_of(n, sig)?);
    }
    // Written since package P1.1; a file without it has no census.
    if let Some(plain) = v.get("plain").and_then(Value::as_object) {
        for (key, n) in plain {
            r.plain.insert(key.clone(), u64_of(n, key)?);
        }
    }
    // Written since package D2-1; a file without it counted no refusals.
    if let Some(unmapped) = v.get("unmapped").and_then(Value::as_object) {
        for (reason, n) in unmapped {
            r.unmapped.insert(reason.clone(), u64_of(n, reason)?);
        }
    }
    for d in v.get("differences").and_then(Value::as_array).ok_or("missing `differences`")? {
        let area_name = str_of(d, "area")?;
        let category = str_of(d, "category")?;
        r.differences.push(Difference {
            area: Area::from_name(area_name).ok_or_else(|| format!("unknown area `{area_name}`"))?,
            category: category_from_name(category).ok_or_else(|| format!("unknown category `{category}`"))?,
            label: str_of(d, "label")?.to_owned(),
            path: str_of(d, "path")?.to_owned(),
            string_side: str_of(d, "string_side")?.to_owned(),
            core_side: str_of(d, "core_side")?.to_owned(),
        });
    }
    r.dropped = u64_at(v, "dropped")?;
    Ok(r)
}

// --- shard files ----------------------------------------------------------------------------------------

impl ShardFile {
    #[must_use]
    pub fn to_json(&self) -> Value {
        let games: Vec<Value> = self
            .records
            .iter()
            .map(|g| {
                let mut game = json!({ "index": g.index, "dual_hash": g.dual_hash, "string_hash": g.string_hash, "report": report_to_json(&g.report) });
                if let Some(why) = &g.skipped {
                    game["skipped"] = json!(why);
                }
                game
            })
            .collect();
        json!({
            "format": FORMAT, "config": self.config, "config_hash": self.config_hash, "games": self.games,
            "spec": self.spec, "areas_on_dual": self.areas_on_dual,
            "shard": self.shard.index, "shards": self.shard.of, "records": games,
        })
    }

    /// # Errors
    /// A document that is not a shard file.
    pub fn from_json(v: &Value) -> Result<Self, String> {
        if v.get("format").and_then(Value::as_str) != Some(FORMAT) {
            return Err(format!("not a {FORMAT} file"));
        }
        let usize_at = |key: &str| u64_at(v, key).and_then(|n| usize::try_from(n).map_err(|e| format!("`{key}`: {e}")));
        let shard = Shard { index: usize_at("shard")?, of: usize_at("shards")? };
        if shard.of == 0 || shard.index >= shard.of {
            return Err(format!("shard {}/{} is not a shard", shard.index, shard.of));
        }
        let mut records = Vec::new();
        for g in v.get("records").and_then(Value::as_array).ok_or("missing `records`")? {
            records.push(GameRecord {
                index: usize::try_from(u64_at(g, "index")?).map_err(|e| e.to_string())?,
                dual_hash: str_of(g, "dual_hash")?.to_owned(),
                string_hash: str_of(g, "string_hash")?.to_owned(),
                report: report_from_json(g.get("report").ok_or("missing `report`")?)?,
                skipped: g.get("skipped").and_then(Value::as_str).map(str::to_owned),
            });
        }
        Ok(Self {
            config: str_of(v, "config")?.to_owned(),
            config_hash: str_of(v, "config_hash")?.to_owned(),
            games: usize_at("games")?,
            spec: str_of(v, "spec")?.to_owned(),
            areas_on_dual: v
                .get("areas_on_dual")
                .and_then(Value::as_array)
                .ok_or("missing `areas_on_dual`")?
                .iter()
                .map(|a| a.as_str().map(str::to_owned).ok_or("areas_on_dual: not a string"))
                .collect::<Result<_, _>>()?,
            shard,
            records,
        })
    }

    /// # Errors
    /// An unreadable or malformed file.
    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
        Self::from_json(&v).map_err(|e| format!("{path}: {e}"))
    }

    /// # Errors
    /// A file that cannot be written.
    pub fn save(&self, path: &str) -> Result<(), String> {
        let text = serde_json::to_string(&self.to_json()).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("{path}: {e}"))
    }
}

// --- the merge ------------------------------------------------------------------------------------------

/// The merged run.
#[derive(Debug)]
pub struct Merged {
    pub games: usize,
    pub areas_on_dual: Vec<String>,
    pub report: DualReport,
    /// `(game, dual hash, string hash)` of the games whose hashes differ.
    pub hash_changes: Vec<(usize, String, String)>,
    /// `(game, reason)` of the scenarios that were not played.
    pub skipped: Vec<(usize, String)>,
}

impl Merged {
    /// Whether the run is clean (no difference, no hash change).
    #[must_use]
    pub fn ok(&self) -> bool {
        self.report.total_differences() == 0 && self.hash_changes.is_empty()
    }

    /// What the unsharded `dual_run` prints (the lines `tools/engine_core/verify_package.sh` keeps).
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for (i, d, s) in &self.hash_changes {
            let _ = writeln!(out, "HASH CHANGED game {i}: dual {d} string {s}");
        }
        let _ = writeln!(
            out,
            "games {} | areas on dual: {:?} | hashes equal: {}",
            self.games,
            self.areas_on_dual,
            self.hash_changes.is_empty()
        );
        if !self.skipped.is_empty() {
            let mut by_reason: BTreeMap<&str, usize> = BTreeMap::new();
            for (_, why) in &self.skipped {
                *by_reason.entry(why.as_str()).or_default() += 1;
            }
            let _ = writeln!(out, "skipped {} of {} scenarios (not played, not compared): {by_reason:?}", self.skipped.len(), self.games);
        }
        let _ = write!(out, "{}", self.report);
        out
    }
}

/// Merge shard files.
///
/// # Errors
/// Files that disagree on config, mask or game count, a shard missing or given twice, a game missing, played twice
/// or outside the run, or a game played by a shard that does not own it.
pub fn merge(files: &[ShardFile]) -> Result<Merged, String> {
    let first = files.first().ok_or("no shard files")?;
    let mut shards_seen: BTreeMap<usize, usize> = BTreeMap::new();
    for (n, f) in files.iter().enumerate() {
        if f.config_hash != first.config_hash || f.games != first.games || f.spec != first.spec || f.shard.of != first.shard.of {
            return Err(format!(
                "shard file {n} ({}/{}, config {}, {} games, `{}`) does not belong to the run of file 0 ({}/{}, config {}, {} games, `{}`)",
                f.shard.index, f.shard.of, f.config, f.games, f.spec, first.shard.index, first.shard.of, first.config, first.games, first.spec
            ));
        }
        if let Some(prev) = shards_seen.insert(f.shard.index, n) {
            return Err(format!("shard {}/{} given twice (files {prev} and {n})", f.shard.index, f.shard.of));
        }
    }
    let missing: Vec<_> = (0..first.shard.of).filter(|i| !shards_seen.contains_key(i)).collect();
    if !missing.is_empty() {
        return Err(format!("shards missing: {missing:?} of {}", first.shard.of));
    }
    let mut by_game: BTreeMap<usize, &GameRecord> = BTreeMap::new();
    for f in files {
        for g in &f.records {
            if g.index >= f.games {
                return Err(format!("shard {}/{} has game {} but the run has {} games", f.shard.index, f.shard.of, g.index, f.games));
            }
            if !f.shard.includes(g.index) {
                return Err(format!("shard {}/{} holds game {} that belongs to shard {}", f.shard.index, f.shard.of, g.index, g.index % f.shard.of));
            }
            if by_game.insert(g.index, g).is_some() {
                return Err(format!("game {} appears twice", g.index));
            }
        }
    }
    let absent: Vec<_> = (0..first.games).filter(|g| !by_game.contains_key(g)).collect();
    if !absent.is_empty() {
        return Err(format!("games missing: {absent:?} of {}", first.games));
    }
    let mut report = DualReport::default();
    let mut hash_changes = Vec::new();
    let mut skipped = Vec::new();
    for (index, g) in &by_game {
        report.add(&g.report);
        if let Some(why) = &g.skipped {
            skipped.push((*index, why.clone()));
        }
        if g.dual_hash != g.string_hash {
            hash_changes.push((*index, g.dual_hash.clone(), g.string_hash.clone()));
        }
    }
    Ok(Merged { games: first.games, areas_on_dual: first.areas_on_dual.clone(), report, hash_changes, skipped })
}
