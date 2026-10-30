//! The allow-list of intended differences between the string engine (the oracle) and the core (package D2-6a,
//! `docs/todo/D2-prereqs.md`; KNOWN_ISSUES A6 and D13).
//!
//! After the flip the legacy engine stays frozen as the oracle and rule fixes go into the core only, so the two diverge
//! on purpose. Each intended divergence is an [`Entry`] of `tools/engine_core/allow_list.json`; a [`Difference`]
//! (`ti4_engine::dual`) that matches an entry is counted for that entry and does not fail the run, any other fails. An
//! entry that matched nothing in a full run is *stale* and fails [`Tally::check_stale`]: a stale entry hides a
//! regression the day the same path differs for a different reason. Nothing here plays games: the harness (D2-6b)
//! builds a [`Tally`], calls [`AllowList::judge`] per scenario and [`Tally::check_stale`] at the end of a full run.
//!
//! # File
//!
//! ```json
//! { "version": 1, "entries": [ { "id": "AL-001", "reason": "...", "ticket": "B1",
//!     "rules_version_min": 36, "rules_version_max": null,
//!     "area": "combat", "label": "combat::roll_*", "path": "players[*].relics*", "scenario": "*" } ] }
//! ```
//!
//! * Required: `id` (unique), `reason`, `ticket` (both non-blank), `rules_version_min`, `area` (an [`Area`] name,
//!   `Area::name`), `label`, `path`. Optional: `rules_version_max` (`null` or absent = open end), `scenario` (absent = `*`).
//!   Unknown fields are refused (a typo must not widen an entry).
//! * Versions: the entry applies at `rules_version` in `[rules_version_min, rules_version_max)`; `max` must exceed `min`.
//!   An entry whose `max` is reached is *expired* ([`AllowList::expired`]) and is reported with the stale ones.
//! * `label` and `scenario` are globs: a whole `*`, a trailing `*` (`combat::roll_*`) or an exact name; a `*` anywhere else is refused.
//! * `path` is a glob on `Difference::path` (`players[2].technologies[3]`, empty for a whole value): literal text; `[*]` any
//!   index; `*` inside the pattern any run of one key (no `.`, `[`, `]`); a `*` at the very end the whole rest of the
//!   path, so `players[*].relics*` covers `players[2].relics`, `players[2].relicsX` and `players[2].relics[1].id`, and a lone `*` everything.
//!   `[` is only valid as `[*]` or `[digits]`.
//! * The first entry (file order) that matches a difference takes it; the entries after it do not count it.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::{Map, Value, json};
use ti4_engine::dual::{Area, Difference, DualReport};

/// The file format version.
pub const VERSION: u64 = 1;

/// The shipped list, relative to the repository root.
pub const DEFAULT_PATH: &str = "tools/engine_core/allow_list.json";

/// The format tag of a hits file ([`Tally::to_json`]).
pub const HITS_FORMAT: &str = "ti4-allow-hits-v1";

const FIELDS: [&str; 9] = ["id", "reason", "ticket", "rules_version_min", "rules_version_max", "area", "label", "path", "scenario"];

// --- globs ---------------------------------------------------------------------------------------

/// A `label` / `scenario` glob: `*`, `prefix*` or exact.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NameGlob {
    Any,
    Prefix(String),
    Exact(String),
}

impl NameGlob {
    fn parse(text: &str) -> Result<Self, String> {
        if text.is_empty() {
            return Err("empty".to_owned());
        }
        if text == "*" {
            return Ok(Self::Any);
        }
        match text.find('*') {
            None => Ok(Self::Exact(text.to_owned())),
            Some(i) if i + 1 == text.len() => Ok(Self::Prefix(text[..i].to_owned())),
            Some(_) => Err(format!("`{text}`: `*` is only allowed as the whole pattern or at its end")),
        }
    }

    fn matches(&self, name: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Prefix(p) => name.starts_with(p.as_str()),
            Self::Exact(e) => name == e,
        }
    }
}

/// One token of a path glob.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Lit(String),
    /// `[*]`: `[` digits `]`.
    AnyIndex,
    /// `*` inside the pattern: a run of key characters (none of `.[]`), possibly empty.
    Key,
    /// `*` at the end: the whole rest of the path, possibly empty.
    Rest,
}

/// A path glob (see the module docs).
#[derive(Debug, Clone, PartialEq, Eq)]
struct PathGlob(Vec<Token>);

impl PathGlob {
    fn parse(text: &str) -> Result<Self, String> {
        let mut tokens: Vec<Token> = Vec::new();
        let mut lit = String::new();
        let mut rest = text;
        let flush = |lit: &mut String, tokens: &mut Vec<Token>| {
            if !lit.is_empty() {
                tokens.push(Token::Lit(std::mem::take(lit)));
            }
        };
        while let Some(c) = rest.chars().next() {
            if let Some(after) = rest.strip_prefix("[*]") {
                flush(&mut lit, &mut tokens);
                tokens.push(Token::AnyIndex);
                rest = after;
            } else if c == '[' {
                let close = rest.find(']').ok_or_else(|| format!("`{text}`: `[` without `]`"))?;
                let digits = &rest[1..close];
                if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(format!("`{text}`: `[{digits}]` is neither `[*]` nor an index"));
                }
                lit.push_str(&rest[..=close]);
                rest = &rest[close + 1..];
            } else if c == ']' {
                return Err(format!("`{text}`: `]` without `[`"));
            } else if c == '*' {
                flush(&mut lit, &mut tokens);
                rest = &rest[1..];
                tokens.push(if rest.is_empty() { Token::Rest } else { Token::Key });
            } else {
                lit.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
        flush(&mut lit, &mut tokens);
        Ok(Self(tokens))
    }

    fn matches(&self, path: &str) -> bool {
        Self::go(&self.0, path)
    }

    fn go(tokens: &[Token], path: &str) -> bool {
        let Some((first, tail)) = tokens.split_first() else {
            return path.is_empty();
        };
        match first {
            Token::Lit(l) => path.strip_prefix(l.as_str()).is_some_and(|rest| Self::go(tail, rest)),
            Token::Rest => true,
            Token::AnyIndex => {
                let Some(rest) = path.strip_prefix('[') else { return false };
                let Some(close) = rest.find(']') else { return false };
                let digits = &rest[..close];
                !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) && Self::go(tail, &rest[close + 1..])
            }
            Token::Key => {
                // Any prefix of the leading key characters, shortest first.
                let key_len = path.find(['.', '[', ']']).unwrap_or(path.len());
                (0..=key_len).filter(|i| path.is_char_boundary(*i)).any(|i| Self::go(tail, &path[i..]))
            }
        }
    }
}

// --- entries -------------------------------------------------------------------------------------

/// One intended difference.
#[derive(Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub reason: String,
    /// The issue / package the difference belongs to (`"B1"`).
    pub ticket: String,
    pub rules_version_min: u32,
    /// Exclusive; `None` = open end.
    pub rules_version_max: Option<u32>,
    pub area: Area,
    pub label: String,
    pub path: String,
    /// `"*"` when the file leaves it out.
    pub scenario: String,
    label_glob: NameGlob,
    path_glob: PathGlob,
    scenario_glob: NameGlob,
}

impl Entry {
    /// Whether `rules_version` lies in `[min, max)`.
    #[must_use]
    pub fn applies_at(&self, rules_version: u32) -> bool {
        rules_version >= self.rules_version_min && self.rules_version_max.is_none_or(|max| rules_version < max)
    }

    fn matches(&self, d: &Difference, rules_version: u32, scenario: &str) -> bool {
        self.applies_at(rules_version)
            && self.area == d.area
            && self.label_glob.matches(&d.label)
            && self.path_glob.matches(&d.path)
            && self.scenario_glob.matches(scenario)
    }

    fn parse(index: usize, value: &Value) -> Result<Self, String> {
        let at = |what: &str| format!("entry {index}: {what}");
        let object = value.as_object().ok_or_else(|| at("not an object"))?;
        let id_for_errors = object.get("id").and_then(Value::as_str).unwrap_or("?").to_owned();
        let at = |what: &str| format!("entry {index} (`{id_for_errors}`): {what}");
        if let Some(unknown) = object.keys().find(|k| !FIELDS.contains(&k.as_str())) {
            return Err(at(&format!("unknown field `{unknown}`")));
        }
        let text = |key: &str, blank_ok: bool| -> Result<String, String> {
            let v = object.get(key).ok_or_else(|| at(&format!("missing `{key}`")))?;
            let s = v.as_str().ok_or_else(|| at(&format!("`{key}` is not a string")))?;
            if !blank_ok && s.trim().is_empty() {
                return Err(at(&format!("`{key}` is blank")));
            }
            Ok(s.to_owned())
        };
        let id = text("id", false)?;
        let reason = text("reason", false)?;
        let ticket = text("ticket", false)?;
        let version = |key: &str| -> Result<u32, String> {
            let n = object.get(key).and_then(Value::as_u64).ok_or_else(|| at(&format!("`{key}` is not a non-negative integer")))?;
            u32::try_from(n).map_err(|_| at(&format!("`{key}` is out of range")))
        };
        if !object.contains_key("rules_version_min") {
            return Err(at("missing `rules_version_min`"));
        }
        let rules_version_min = version("rules_version_min")?;
        let rules_version_max = match object.get("rules_version_max") {
            None | Some(Value::Null) => None,
            Some(_) => Some(version("rules_version_max")?),
        };
        if let Some(max) = rules_version_max
            && max <= rules_version_min
        {
            return Err(at(&format!("`rules_version_max` ({max}) must exceed `rules_version_min` ({rules_version_min})")));
        }
        let area_name = text("area", false)?;
        let area = Area::from_name(&area_name).ok_or_else(|| at(&format!("unknown area `{area_name}`")))?;
        let label = text("label", false)?;
        let path = text("path", true)?;
        let scenario = match object.get("scenario") {
            None => "*".to_owned(),
            Some(_) => text("scenario", false)?,
        };
        let label_glob = NameGlob::parse(&label).map_err(|e| at(&format!("label {e}")))?;
        let path_glob = PathGlob::parse(&path).map_err(|e| at(&format!("path {e}")))?;
        let scenario_glob = NameGlob::parse(&scenario).map_err(|e| at(&format!("scenario {e}")))?;
        Ok(Self { id, reason, ticket, rules_version_min, rules_version_max, area, label, path, scenario, label_glob, path_glob, scenario_glob })
    }

    fn to_json(&self) -> Value {
        json!({
            "id": self.id, "reason": self.reason, "ticket": self.ticket,
            "rules_version_min": self.rules_version_min, "rules_version_max": self.rules_version_max,
            "area": self.area.name(), "label": self.label, "path": self.path, "scenario": self.scenario,
        })
    }
}

/// The parsed file.
#[derive(Debug, Clone, Default)]
pub struct AllowList {
    entries: Vec<Entry>,
}

/// The outcome of [`AllowList::judge`] for one report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Verdict {
    /// Differences that matched an entry (counted in the [`Tally`]).
    pub allowed: u64,
    /// Differences no entry covers: each of these fails the run.
    pub unallowed: Vec<Difference>,
    /// Differences the report only counted (`DualReport::dropped`, beyond `MAX_KEPT`): they cannot be matched, so they fail.
    pub unverifiable: u64,
}

impl Verdict {
    /// No difference is left that the list does not cover.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.unallowed.is_empty() && self.unverifiable == 0
    }
}

impl AllowList {
    /// The list with no entry (what ships before D2).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Parse the text of the file.
    ///
    /// # Errors
    /// Bad JSON, a wrong `version`, a missing or malformed field, an unknown field, a duplicate `id`, a bad glob or version range.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(text).map_err(|e| format!("allow-list: not JSON: {e}"))?;
        let object = value.as_object().ok_or("allow-list: top level is not an object")?;
        if let Some(unknown) = object.keys().find(|k| !["version", "entries"].contains(&k.as_str())) {
            return Err(format!("allow-list: unknown top-level field `{unknown}`"));
        }
        match object.get("version").and_then(Value::as_u64) {
            Some(VERSION) => {}
            other => return Err(format!("allow-list: version {other:?}, expected {VERSION}")),
        }
        let list = object.get("entries").and_then(Value::as_array).ok_or("allow-list: `entries` is not an array")?;
        let mut entries: Vec<Entry> = Vec::with_capacity(list.len());
        for (i, v) in list.iter().enumerate() {
            let entry = Entry::parse(i, v)?;
            if entries.iter().any(|e| e.id == entry.id) {
                return Err(format!("allow-list: duplicate id `{}`", entry.id));
            }
            entries.push(entry);
        }
        Ok(Self { entries })
    }

    /// Read and parse a file.
    ///
    /// # Errors
    /// An unreadable file or anything [`AllowList::parse`] refuses.
    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        Self::parse(&text).map_err(|e| format!("{path}: {e}"))
    }

    /// The entries in file order.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The file text of this list (canonical form: every field written).
    #[must_use]
    pub fn to_json(&self) -> String {
        let value = json!({ "version": VERSION, "entries": self.entries.iter().map(Entry::to_json).collect::<Vec<_>>() });
        serde_json::to_string_pretty(&value).expect("a list is JSON") + "\n"
    }

    /// The first entry that covers `d` at `rules_version` in `scenario`.
    #[must_use]
    pub fn matches(&self, d: &Difference, rules_version: u32, scenario: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.matches(d, rules_version, scenario))
    }

    /// The entries whose version range ended at or before `rules_version`: they can never match again and must be deleted.
    pub fn expired(&self, rules_version: u32) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(move |e| e.rules_version_max.is_some_and(|max| rules_version >= max))
    }

    /// Sort the differences of one report into allowed (counted in `tally`) and not.
    pub fn judge(&self, report: &DualReport, rules_version: u32, scenario: &str, tally: &mut Tally) -> Verdict {
        let mut verdict = Verdict { unverifiable: report.dropped, ..Verdict::default() };
        for d in &report.differences {
            match self.matches(d, rules_version, scenario) {
                Some(entry) => {
                    tally.record(&entry.id);
                    verdict.allowed += 1;
                }
                None => verdict.unallowed.push(d.clone()),
            }
        }
        verdict
    }
}

// --- hits and the stale check --------------------------------------------------------------------

/// Hits per entry id over a run (shards add up: [`Tally::merge`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally {
    hits: BTreeMap<String, u64>,
}

impl Tally {
    /// A tally with every entry of `list` at 0 (so an entry that never matched is listed).
    #[must_use]
    pub fn new(list: &AllowList) -> Self {
        Self { hits: list.entries.iter().map(|e| (e.id.clone(), 0)).collect() }
    }

    /// Count one difference for entry `id`.
    pub fn record(&mut self, id: &str) {
        *self.hits.entry(id.to_owned()).or_insert(0) += 1;
    }

    /// Hits of `id`.
    #[must_use]
    pub fn hits(&self, id: &str) -> u64 {
        self.hits.get(id).copied().unwrap_or(0)
    }

    /// Add another tally (another shard).
    pub fn merge(&mut self, other: &Self) {
        for (id, n) in &other.hits {
            *self.hits.entry(id.clone()).or_insert(0) += n;
        }
    }

    /// The entries of `list` that matched 0 differences, and the entries expired at `rules_version`.
    ///
    /// # Errors
    /// Names every such entry. Only meaningful for a *full* run: a partial run (one corpus, one shard) legitimately leaves entries unhit.
    pub fn check_stale(&self, list: &AllowList, rules_version: u32) -> Result<(), String> {
        let mut problems = String::new();
        for e in list.entries() {
            if self.hits(&e.id) == 0 {
                let _ = writeln!(problems, "STALE allow-list entry {} ({}, ticket {}): matched 0 differences", e.id, e.reason, e.ticket);
            }
        }
        for e in list.expired(rules_version) {
            let _ = writeln!(problems, "EXPIRED allow-list entry {}: rules_version_max {:?} reached at {rules_version}; delete it", e.id, e.rules_version_max);
        }
        let unknown: Vec<&String> = self.hits.keys().filter(|id| !list.entries().iter().any(|e| &e.id == *id)).collect();
        if !unknown.is_empty() {
            let _ = writeln!(problems, "hits for ids that are not in the allow-list: {unknown:?}");
        }
        if problems.is_empty() { Ok(()) } else { Err(problems) }
    }

    /// The summary of a full run: hits per entry.
    #[must_use]
    pub fn render(&self, list: &AllowList) -> String {
        let mut out = format!("allow-list: {} entries\n", list.entries().len());
        for e in list.entries() {
            let _ = writeln!(out, "  {} [{}] {}::{} at `{}`: {} hits", e.id, e.ticket, e.area.name(), e.label, e.path, self.hits(&e.id));
        }
        out
    }

    /// The hits as a file (shards write one, the merge adds them).
    #[must_use]
    pub fn to_json(&self) -> String {
        let hits: Map<String, Value> = self.hits.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        serde_json::to_string_pretty(&json!({ "format": HITS_FORMAT, "hits": hits })).expect("hits are JSON") + "\n"
    }

    /// Read a hits file.
    ///
    /// # Errors
    /// Bad JSON, another format tag, a non-integer count.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(text).map_err(|e| format!("hits: not JSON: {e}"))?;
        if value.get("format").and_then(Value::as_str) != Some(HITS_FORMAT) {
            return Err(format!("hits: format is not {HITS_FORMAT}"));
        }
        let hits = value.get("hits").and_then(Value::as_object).ok_or("hits: `hits` is not an object")?;
        let mut tally = Self::default();
        for (id, n) in hits {
            let n = n.as_u64().ok_or_else(|| format!("hits: `{id}` is not a count"))?;
            tally.hits.insert(id.clone(), n);
        }
        Ok(tally)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine::dual::Category;

    fn diff(area: Area, label: &str, path: &str) -> Difference {
        Difference { area, category: Category::State, label: label.to_owned(), path: path.to_owned(), string_side: "a".to_owned(), core_side: "b".to_owned() }
    }

    fn entry_json(patch: &str) -> String {
        // A full valid entry; `patch` is `"field": value` pairs that replace or add fields.
        let mut base: Value = json!({ "id": "AL-001", "reason": "Heart of Ixth fix", "ticket": "B1", "rules_version_min": 36,
            "area": "combat", "label": "combat::roll_*", "path": "players[*].relics*" });
        let patch: Value = serde_json::from_str(&format!("{{{patch}}}")).unwrap();
        for (k, v) in patch.as_object().unwrap() {
            if v.is_null() && k.starts_with('-') {
                base.as_object_mut().unwrap().remove(&k[1..]);
            } else {
                base[k] = v.clone();
            }
        }
        json!({ "version": 1, "entries": [base] }).to_string()
    }

    fn one(patch: &str) -> AllowList {
        AllowList::parse(&entry_json(patch)).unwrap()
    }

    fn err(patch: &str) -> String {
        AllowList::parse(&entry_json(patch)).unwrap_err()
    }

    #[test]
    fn matches_area_label_path_and_scenario() {
        let list = one(r#""scenario": "golden-*""#);
        let d = diff(Area::Combat, "combat::roll_hits", "players[2].relics");
        assert_eq!(list.matches(&d, 36, "golden-7").map(|e| e.id.as_str()), Some("AL-001"));
        assert!(list.matches(&diff(Area::Invasion, "combat::roll_hits", "players[2].relics"), 36, "golden-7").is_none(), "area");
        assert!(list.matches(&diff(Area::Combat, "combat::other", "players[2].relics"), 36, "golden-7").is_none(), "label");
        assert!(list.matches(&diff(Area::Combat, "combat::roll_hits", "players[2].tech"), 36, "golden-7").is_none(), "path");
        assert!(list.matches(&d, 36, "bench-7").is_none(), "scenario");
    }

    #[test]
    fn the_scenario_defaults_to_any() {
        let list = one("");
        assert_eq!(list.entries()[0].scenario, "*");
        assert!(list.matches(&diff(Area::Combat, "combat::roll_x", "players[0].relics"), 36, "anything").is_some());
    }

    #[test]
    fn version_bounds_are_min_inclusive_max_exclusive() {
        let list = one(r#""rules_version_min": 36, "rules_version_max": 38"#);
        let d = diff(Area::Combat, "combat::roll_x", "players[0].relics");
        assert!(list.matches(&d, 35, "s").is_none());
        assert!(list.matches(&d, 36, "s").is_some());
        assert!(list.matches(&d, 37, "s").is_some());
        assert!(list.matches(&d, 38, "s").is_none());
        let open = one(r#""rules_version_max": null"#);
        assert!(open.matches(&d, 4_000_000, "s").is_some(), "an open end never expires");
        assert!(open.matches(&d, 35, "s").is_none());
    }

    #[test]
    fn a_range_that_ends_before_it_starts_is_refused() {
        assert!(err(r#""rules_version_max": 36"#).contains("must exceed"));
        assert!(err(r#""rules_version_max": 3"#).contains("must exceed"));
    }

    #[test]
    fn path_globs() {
        let m = |pattern: &str, path: &str| PathGlob::parse(pattern).unwrap().matches(path);
        // `[*]` is any index, only an index.
        assert!(m("players[*].relics", "players[0].relics"));
        assert!(m("players[*].relics", "players[17].relics"));
        assert!(!m("players[*].relics", "players[].relics"));
        assert!(!m("players[*].relics", "players[x].relics"));
        assert!(!m("players[*].relics", "players.relics"));
        // A literal index is exact.
        assert!(m("players[2].relics", "players[2].relics"));
        assert!(!m("players[2].relics", "players[3].relics"));
        // A trailing `*` is the whole rest, even empty.
        assert!(m("players[*].relics*", "players[2].relics"));
        assert!(m("players[*].relics*", "players[2].relicsX"));
        assert!(m("players[*].relics*", "players[2].relics[1].id"));
        assert!(!m("players[*].relics*", "players[2].tech"));
        assert!(!m("players[*].relics*", "players[2]"));
        assert!(m("*", ""));
        assert!(m("*", "players[2].relics"));
        // A `*` inside stays within one key.
        assert!(m("players[*].*_tokens.total", "players[1].command_tokens.total"));
        assert!(m("players[*].*_tokens.total", "players[1]._tokens.total"));
        assert!(!m("players[*].*_tokens.total", "players[1].a.b_tokens.total"));
        assert!(!m("players[*].*_tokens.total", "players[1].command_tokens.other"));
        // Exact paths and the whole-value path.
        assert!(m("", ""));
        assert!(!m("", "a"));
        assert!(m("rng", "rng"));
        assert!(!m("rng", "rng.x"));
        assert!(!m("rng", "rn"));
    }

    #[test]
    fn name_globs() {
        let g = |s: &str| NameGlob::parse(s).unwrap();
        assert!(g("*").matches("anything"));
        assert!(g("*").matches(""));
        assert!(g("combat::roll_*").matches("combat::roll_hits"));
        assert!(g("combat::roll_*").matches("combat::roll_"));
        assert!(!g("combat::roll_*").matches("combat::roll"));
        assert!(g("combat::roll").matches("combat::roll"));
        assert!(!g("combat::roll").matches("combat::roll_x"));
        for bad in ["a*b", "*a", "a**", "**", ""] {
            assert!(NameGlob::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn globs_with_a_star_in_the_wrong_place_are_refused() {
        assert!(err(r#""label": "*roll""#).contains("label"));
        assert!(err(r#""label": "a*b""#).contains("label"));
        assert!(err(r#""scenario": "a*b""#).contains("scenario"));
        assert!(err(r#""path": "players[x].relics""#).contains("path"));
        assert!(err(r#""path": "players[.relics""#).contains("path"));
        assert!(err(r#""path": "players].relics""#).contains("path"));
        assert!(err(r#""path": "players[].relics""#).contains("path"));
    }

    #[test]
    fn missing_required_fields_are_refused() {
        for field in ["id", "reason", "ticket", "rules_version_min", "area", "label", "path"] {
            let e = err(&format!(r#""-{field}": null"#));
            assert!(e.contains("missing") && e.contains(field), "{field}: {e}");
        }
    }

    #[test]
    fn blank_reason_ticket_and_id_are_refused() {
        for field in ["id", "reason", "ticket", "label", "area"] {
            for blank in [r#""""#, r#""  ""#] {
                let e = err(&format!(r#""{field}": {blank}"#));
                assert!(e.contains("blank"), "{field}: {e}");
            }
        }
        // The path may be empty (a whole value); the scenario may not be.
        assert!(AllowList::parse(&entry_json(r#""path": """#)).is_ok());
        assert!(err(r#""scenario": """#).contains("blank"));
    }

    #[test]
    fn wrong_types_are_refused() {
        assert!(err(r#""rules_version_min": "36""#).contains("rules_version_min"));
        assert!(err(r#""rules_version_min": -1"#).contains("rules_version_min"));
        assert!(err(r#""rules_version_min": 1.5"#).contains("rules_version_min"));
        assert!(err(r#""rules_version_min": 99999999999"#).contains("out of range"));
        assert!(err(r#""rules_version_max": "x""#).contains("rules_version_max"));
        assert!(err(r#""reason": 5"#).contains("not a string"));
    }

    #[test]
    fn an_unknown_field_an_unknown_area_and_a_duplicate_id_are_refused() {
        assert!(err(r#""rules_version_mix": 3"#).contains("unknown field"));
        assert!(err(r#""area": "no_such_area""#).contains("unknown area"));
        let dup = format!(
            r#"{{"version":1,"entries":[{e},{e}]}}"#,
            e = serde_json::to_string(&serde_json::from_str::<Value>(&entry_json("")).unwrap()["entries"][0]).unwrap()
        );
        assert!(AllowList::parse(&dup).unwrap_err().contains("duplicate id"));
    }

    #[test]
    fn the_file_envelope_is_checked() {
        assert!(AllowList::parse("not json").unwrap_err().contains("not JSON"));
        assert!(AllowList::parse("[]").unwrap_err().contains("not an object"));
        assert!(AllowList::parse(r#"{"entries":[]}"#).unwrap_err().contains("version"));
        assert!(AllowList::parse(r#"{"version":2,"entries":[]}"#).unwrap_err().contains("version"));
        assert!(AllowList::parse(r#"{"version":1}"#).unwrap_err().contains("entries"));
        assert!(AllowList::parse(r#"{"version":1,"entries":{}}"#).unwrap_err().contains("entries"));
        assert!(AllowList::parse(r#"{"version":1,"entries":[3]}"#).unwrap_err().contains("not an object"));
        assert!(AllowList::parse(r#"{"version":1,"entries":[],"extra":1}"#).unwrap_err().contains("unknown top-level"));
        assert!(AllowList::parse(r#"{"version":1,"entries":[]}"#).unwrap().entries().is_empty());
    }

    #[test]
    fn the_first_matching_entry_takes_the_difference() {
        let text = json!({ "version": 1, "entries": [
            { "id": "A", "reason": "r", "ticket": "t", "rules_version_min": 1, "area": "combat", "label": "*", "path": "*" },
            { "id": "B", "reason": "r", "ticket": "t", "rules_version_min": 1, "area": "combat", "label": "combat::x", "path": "p" },
        ] })
        .to_string();
        let list = AllowList::parse(&text).unwrap();
        let d = diff(Area::Combat, "combat::x", "p");
        assert_eq!(list.matches(&d, 5, "s").unwrap().id, "A");
        let mut tally = Tally::new(&list);
        let report = DualReport { differences: vec![d], ..DualReport::default() };
        assert!(list.judge(&report, 5, "s", &mut tally).ok());
        assert_eq!((tally.hits("A"), tally.hits("B")), (1, 0));
        // The shadowed entry is stale.
        assert!(tally.check_stale(&list, 5).unwrap_err().contains("entry B"));
    }

    #[test]
    fn judge_counts_allowed_and_fails_the_rest() {
        let list = one("");
        let mut tally = Tally::new(&list);
        let allowed = diff(Area::Combat, "combat::roll_x", "players[0].relics");
        let other = diff(Area::Combat, "combat::roll_x", "players[0].tech");
        let report = DualReport { differences: vec![allowed.clone(), other.clone(), allowed], ..DualReport::default() };
        let verdict = list.judge(&report, 36, "s", &mut tally);
        assert!(!verdict.ok());
        assert_eq!(verdict.allowed, 2);
        assert_eq!(verdict.unallowed, vec![other]);
        assert_eq!(tally.hits("AL-001"), 2);
        // Before the entry's version nothing is allowed.
        let mut tally = Tally::new(&list);
        let verdict = list.judge(&report, 35, "s", &mut tally);
        assert_eq!((verdict.allowed, verdict.unallowed.len()), (0, 3));
        assert_eq!(tally.hits("AL-001"), 0);
    }

    #[test]
    fn an_empty_list_allows_nothing_and_an_empty_report_passes() {
        let list = AllowList::empty();
        let mut tally = Tally::new(&list);
        assert!(list.judge(&DualReport::default(), 36, "s", &mut tally).ok());
        let report = DualReport { differences: vec![diff(Area::Combat, "l", "p")], ..DualReport::default() };
        assert!(!list.judge(&report, 36, "s", &mut tally).ok());
        assert!(tally.check_stale(&list, 36).is_ok(), "nothing to be stale");
    }

    #[test]
    fn differences_beyond_the_kept_ones_cannot_be_allowed() {
        let list = one("");
        let mut tally = Tally::new(&list);
        let report = DualReport { dropped: 3, ..DualReport::default() };
        let verdict = list.judge(&report, 36, "s", &mut tally);
        assert_eq!(verdict.unverifiable, 3);
        assert!(!verdict.ok());
    }

    #[test]
    fn stale_check_fails_on_an_unhit_entry_and_passes_on_a_hit_one() {
        let list = one("");
        let mut tally = Tally::new(&list);
        let e = tally.check_stale(&list, 36).unwrap_err();
        assert!(e.contains("STALE") && e.contains("AL-001") && e.contains("B1"), "{e}");
        tally.record("AL-001");
        assert!(tally.check_stale(&list, 36).is_ok());
        // Hits for an id the file does not have are a mismatch of file and run.
        tally.record("AL-999");
        assert!(tally.check_stale(&list, 36).unwrap_err().contains("AL-999"));
    }

    #[test]
    fn an_expired_entry_is_reported_with_the_stale_ones() {
        let list = one(r#""rules_version_max": 40"#);
        let mut tally = Tally::new(&list);
        tally.record("AL-001");
        assert!(tally.check_stale(&list, 39).is_ok());
        assert_eq!(list.expired(39).count(), 0);
        let e = tally.check_stale(&list, 40).unwrap_err();
        assert!(e.contains("EXPIRED") && e.contains("AL-001"), "{e}");
        assert_eq!(list.expired(40).count(), 1);
    }

    #[test]
    fn shard_tallies_add_up_and_round_trip() {
        let list = one("");
        let (mut a, mut b) = (Tally::new(&list), Tally::new(&list));
        a.record("AL-001");
        b.record("AL-001");
        b.record("AL-001");
        a.merge(&b);
        assert_eq!(a.hits("AL-001"), 3);
        let back = Tally::from_json(&a.to_json()).unwrap();
        assert_eq!(back, a);
        assert!(a.render(&list).contains("3 hits"));
        assert!(Tally::from_json("{}").is_err());
        assert!(Tally::from_json(r#"{"format":"ti4-allow-hits-v1","hits":{"x":-1}}"#).is_err());
    }

    #[test]
    fn the_canonical_text_parses_back() {
        let list = one(r#""rules_version_max": 40, "scenario": "golden-*""#);
        let again = AllowList::parse(&list.to_json()).unwrap();
        assert_eq!(again.to_json(), list.to_json());
        assert_eq!(again.entries()[0].rules_version_max, Some(40));
    }

    /// The shipped file: valid, and no entry from the future or already expired (an entry is added together with the
    /// `RULES_VERSION` bump that makes the difference intended, A6/D13).
    #[test]
    fn the_shipped_file_is_valid_and_current() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/engine_core/allow_list.json");
        let list = AllowList::load(path).unwrap();
        let now = ti4_model::state::RULES_VERSION;
        for e in list.entries() {
            assert!(e.rules_version_min <= now, "{} starts at {} but RULES_VERSION is {now}", e.id, e.rules_version_min);
        }
        assert_eq!(list.expired(now).count(), 0, "expired entries must be deleted");
    }
}
