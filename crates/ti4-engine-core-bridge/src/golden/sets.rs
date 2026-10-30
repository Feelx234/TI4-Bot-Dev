//! Sets: a directory of game files with a `set.json`, and the index `sets.json` over all of them.
//!
//! ```text
//! tools/engine_core/golden/
//!   sets.json              index (written by `golden index`, which every command that adds or changes a set runs)
//!   CHANGES.md             append-only bless log
//!   <set>/set.json         one set: tier, rules version, frozen flag, the games with sizes and sha256
//!   <set>/gNNN.jsonl.zst   full-tier games   (or gNNN.tsv.zst, slim)
//!   legacy-sources/<source>/...  the imported X2 sources, one set each
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::format::Tier;

/// One game of a set as `set.json` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameEntry {
    pub file: String,
    pub steps: usize,
    pub decisions: usize,
    pub decisions_hash: String,
    /// Compressed size.
    pub bytes: u64,
    /// SHA-256 of the decompressed text.
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetMeta {
    pub name: String,
    pub tier: Tier,
    /// The `RULES_VERSION` every game of the set is played at; `None` only for a static set (nothing to play).
    pub rules_version: Option<u32>,
    /// A frozen set is never re-blessed except by `bless --bugfix --issue ID`. Every set below the current
    /// `RULES_VERSION` is frozen whatever this says; `golden index` writes the flag for them.
    #[serde(default)]
    pub frozen: bool,
    pub source: String,
    #[serde(default)]
    pub note: String,
    /// The ladder: one game per rules version, each game header pins its own; `rules_version` is `None`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ladder: bool,
    pub games: Vec<GameEntry>,
    /// The files `cargo test` replays: the smallest games of the set.
    pub smoke: Vec<String>,
}

impl SetMeta {
    #[must_use]
    pub fn decisions(&self) -> usize {
        self.games.iter().map(|g| g.decisions).sum()
    }

    /// Whether `bless` must refuse this set without `--bugfix`.
    #[must_use]
    pub fn is_frozen(&self, current_rules_version: u32) -> bool {
        self.frozen || self.ladder || self.rules_version.is_some_and(|v| v < current_rules_version)
    }

    /// The `n` smallest games by decision count (ties by file name), file names sorted.
    #[must_use]
    pub fn pick_smoke(games: &[GameEntry], n: usize) -> Vec<String> {
        let mut by_size: Vec<&GameEntry> = games.iter().collect();
        by_size.sort_by(|a, b| a.decisions.cmp(&b.decisions).then(a.file.cmp(&b.file)));
        let mut v: Vec<String> = by_size.into_iter().take(n).map(|g| g.file.clone()).collect();
        v.sort();
        v
    }

    /// # Errors
    /// An unreadable or malformed `set.json`.
    pub fn read(dir: &Path) -> Result<Self, String> {
        let p = dir.join("set.json");
        serde_json::from_str(&std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?)
            .map_err(|e| format!("{}: {e}", p.display()))
    }

    /// # Errors
    /// An unwritable directory.
    pub fn write(&self, dir: &Path) -> Result<(), String> {
        let p = dir.join("set.json");
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())? + "\n";
        std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))
    }
}

/// One row of `sets.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetEntry {
    pub name: String,
    /// Relative to the golden directory.
    pub dir: String,
    pub tier: Tier,
    pub rules_version: Option<u32>,
    pub frozen: bool,
    pub source: String,
    pub games: usize,
    pub decisions: usize,
    /// Mean pure share over the games that recorded one, per mille.
    pub pure_share_at_record: Option<u32>,
}

/// # Errors
/// An unreadable or malformed `sets.json`.
pub fn read_index(root: &Path) -> Result<Vec<SetEntry>, String> {
    let p = root.join("sets.json");
    serde_json::from_str(&std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?)
        .map_err(|e| format!("{}: {e}", p.display()))
}

/// The set directories under `root` (a directory holding a `set.json`, at most two levels deep), sorted.
#[must_use]
pub fn find_set_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if dir.join("set.json").is_file() {
            out.push(dir);
            continue;
        }
        if depth >= 2 {
            continue;
        }
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    stack.push((e.path(), depth + 1));
                }
            }
        }
    }
    out.sort();
    out
}

/// Rewrite `sets.json` from the `set.json` files and freeze every set below `current`. Returns the index.
///
/// # Errors
/// An unreadable set or an unwritable index.
pub fn reindex(root: &Path, current: u32, pure_milli: &dyn Fn(&Path, &SetMeta) -> Option<u32>) -> Result<Vec<SetEntry>, String> {
    let mut entries = Vec::new();
    for dir in find_set_dirs(root) {
        let mut meta = SetMeta::read(&dir)?;
        if !meta.frozen && meta.rules_version.is_some_and(|v| v < current) {
            meta.frozen = true;
            meta.write(&dir)?;
        }
        let rel = dir.strip_prefix(root).unwrap_or(&dir).to_string_lossy().into_owned();
        entries.push(SetEntry {
            name: meta.name.clone(),
            dir: rel,
            tier: meta.tier,
            rules_version: meta.rules_version,
            frozen: meta.frozen,
            source: meta.source.clone(),
            games: meta.games.len(),
            decisions: meta.decisions(),
            pure_share_at_record: pure_milli(&dir, &meta),
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let text = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())? + "\n";
    std::fs::write(root.join("sets.json"), text).map_err(|e| e.to_string())?;
    Ok(entries)
}

/// The T7 rule: the index must hold a set recorded at `current`, and the `ladder` set must hold a game at every
/// version `0..=current`.
///
/// # Errors
/// What is missing.
pub fn every_version_has_a_set(root: &Path, current: u32) -> Result<(), String> {
    let index = read_index(root)?;
    if !index.iter().any(|e| e.rules_version == Some(current)) {
        return Err(format!("RULES_VERSION is {current} and no set in sets.json was recorded at it: run `golden bless-new --set v{current}-std ...` and `golden index`"));
    }
    let ladder = index.iter().find(|e| e.name == "ladder").ok_or("no set named `ladder` in sets.json")?;
    let dir = root.join(&ladder.dir);
    let mut have = std::collections::BTreeSet::new();
    for g in SetMeta::read(&dir)?.games {
        let (file, _) = super::format::GameFile::read(&dir.join(&g.file))?;
        if let Some(v) = file.header.rules_version {
            have.insert(v);
        }
    }
    let missing: Vec<u32> = (0..=current).filter(|v| !have.contains(v)).collect();
    if missing.is_empty() { Ok(()) } else { Err(format!("the ladder has no game at rules version(s) {missing:?}")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden::format::tests::scratch;

    fn entry(file: &str, decisions: usize) -> GameEntry {
        GameEntry { file: file.into(), steps: decisions * 2, decisions, decisions_hash: "h".into(), bytes: 1, sha256: "s".into() }
    }

    fn write_set(root: &Path, name: &str, version: Option<u32>, frozen: bool) {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let games = vec![entry("g000.tsv.zst", 30), entry("g001.tsv.zst", 10), entry("g002.tsv.zst", 20)];
        let smoke = SetMeta::pick_smoke(&games, 2);
        SetMeta { name: name.into(), tier: Tier::Slim, rules_version: version, frozen, source: "test".into(), note: String::new(), ladder: false, games, smoke }
            .write(&dir)
            .unwrap();
    }

    #[test]
    fn smoke_is_the_smallest_games_sorted_by_name() {
        let games = vec![entry("g000", 30), entry("g001", 10), entry("g002", 20)];
        assert_eq!(SetMeta::pick_smoke(&games, 2), ["g001", "g002"]);
        assert_eq!(SetMeta::pick_smoke(&games, 9), ["g000", "g001", "g002"]);
    }

    #[test]
    fn reindex_freezes_old_sets_and_counts() {
        let root = scratch("reindex");
        write_set(&root, "v34-std", Some(34), false);
        write_set(&root, "v35-std", Some(35), false);
        std::fs::create_dir_all(root.join("legacy-sources")).unwrap();
        write_set(&root.join("legacy-sources"), "legacy-te", None, false);
        let index = reindex(&root, 35, &|_, _| None).unwrap();
        let by = |n: &str| index.iter().find(|e| e.name == n).unwrap().clone();
        assert!(by("v34-std").frozen && !by("v35-std").frozen && !by("legacy-te").frozen);
        assert_eq!(by("v35-std").games, 3);
        assert_eq!(by("v35-std").decisions, 60);
        assert_eq!(by("legacy-te").dir, "legacy-sources/legacy-te");
        assert!(SetMeta::read(&root.join("v34-std")).unwrap().frozen, "the flag is written into set.json");
        assert!(SetMeta::read(&root.join("v35-std")).unwrap().is_frozen(36), "a set below the current version is frozen");
        assert_eq!(read_index(&root).unwrap(), index);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_new_rules_version_without_a_set_is_reported() {
        let root = scratch("every_version");
        write_set(&root, "v35-std", Some(35), false);
        reindex(&root, 35, &|_, _| None).unwrap();
        // no `ladder` set yet
        assert!(every_version_has_a_set(&root, 35).unwrap_err().contains("ladder"));
        assert!(every_version_has_a_set(&root, 36).unwrap_err().contains("no set in sets.json was recorded at it"));
        let _ = std::fs::remove_dir_all(root);
    }
}
