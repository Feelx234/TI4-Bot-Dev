//! `golden bless` and its guards (docs/todo/D4-core-tests.md 4.5), and the store helpers that write sets.
//!
//! `bless` re-records ONLY the games that differ, bot-driven with the header's pinned bot (a changed rule changes what
//! is offered, so the old answers may be illegal; the whole game file is replaced). It refuses:
//! (a) a frozen set (below the current `RULES_VERSION`, or flagged) unless `--bugfix --issue ID`;
//! (b) changed decisions whose subtype is not in `--expect` (collateral change; `--expect '*'` accepts any, explicitly);
//! (c) a missing `--reason`;
//! (d) uncommitted changes under the golden directory (a bless must be reviewable as its own diff; the rule fix
//!     itself is expected to be in the tree, outside the golden directory).
//! and appends one block to `CHANGES.md`: date (Berlin), commit, set, games, histogram of changed subtypes, reason, issue.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use super::format::{GameFile, Tier};
use super::replay::{self, Divergence, RecordReq, Recording, Replayer};
use super::sets::{GameEntry, SetMeta, find_set_dirs};

/// Write `games` (stem, file) into `dir` and return the set's `games` list.
///
/// # Errors
/// An unwritable directory.
pub fn save_games(dir: &Path, games: &[(String, GameFile)]) -> Result<Vec<GameEntry>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out = Vec::new();
    for (stem, g) in games {
        let (file, sha256, bytes) = g.write(dir, stem)?;
        out.push(GameEntry { file, steps: g.header.steps, decisions: g.decs.len(), decisions_hash: g.header.decisions_hash.clone(), bytes, sha256 });
    }
    Ok(out)
}

/// The decisions that differ between two recordings of the same game, counted per subtype.
#[must_use]
pub fn changed_decisions(old: &GameFile, new: &GameFile) -> BTreeMap<String, usize> {
    let mut h: BTreeMap<String, usize> = BTreeMap::new();
    let key = |s: &str| if s.is_empty() { "(none)".to_owned() } else { s.to_owned() };
    for i in 0..old.decs.len().max(new.decs.len()) {
        match (old.decs.get(i), new.decs.get(i)) {
            (Some(a), Some(b)) if (a.step, a.k, &a.chosen, &a.v2) == (b.step, b.k, &b.chosen, &b.v2) => {}
            (_, Some(b)) => *h.entry(key(&b.subtype)).or_default() += 1,
            (Some(a), None) => *h.entry(key(&a.subtype)).or_default() += 1,
            (None, None) => {}
        }
    }
    h
}

pub trait BlessEnv {
    /// Uncommitted changes under the golden directory.
    ///
    /// # Errors
    /// `git` not usable.
    fn dirty(&self) -> Result<bool, String>;
    /// # Errors
    /// `git` not usable.
    fn commit(&self) -> Result<String, String>;
    /// Date and time in Berlin.
    ///
    /// # Errors
    /// `date` not usable.
    fn now_berlin(&self) -> Result<String, String>;
}

pub struct GitEnv {
    pub golden_dir: PathBuf,
}

fn run(cmd: &str, args: &[&str], dir: &Path) -> Result<String, String> {
    let out = std::process::Command::new(cmd)
        .args(args)
        .current_dir(dir)
        .env("TZ", "Europe/Berlin")
        .output()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{cmd} {args:?}: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

impl BlessEnv for GitEnv {
    fn dirty(&self) -> Result<bool, String> {
        Ok(!run("git", &["status", "--porcelain", "--", "."], &self.golden_dir)?.is_empty())
    }
    fn commit(&self) -> Result<String, String> {
        run("git", &["rev-parse", "--short", "HEAD"], &self.golden_dir)
    }
    fn now_berlin(&self) -> Result<String, String> {
        run("date", &["+%F %H:%M"], &self.golden_dir)
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlessArgs {
    pub reason: String,
    pub expect: Vec<String>,
    pub issue: Option<String>,
    pub bugfix: bool,
    /// Only games whose file name contains one of these (empty: all).
    pub only: Vec<String>,
}

#[derive(Debug, Default)]
pub struct BlessReport {
    pub changed_files: Vec<String>,
    pub histogram: BTreeMap<String, usize>,
    pub block: String,
}

/// Find the set directory by name under `root`.
///
/// # Errors
/// No such set.
pub fn find_set(root: &Path, name: &str) -> Result<(PathBuf, SetMeta), String> {
    for dir in find_set_dirs(root) {
        let meta = SetMeta::read(&dir)?;
        if meta.name == name {
            return Ok((dir, meta));
        }
    }
    Err(format!("no set `{name}` under {}", root.display()))
}

/// The per-game table of `golden diff`: the games that do not reproduce.
///
/// # Errors
/// A set that cannot be read or a game that errors (not diverges).
pub fn diff_set(root: &Path, set: &str, replayer: &dyn Replayer, only: &[String], threads: usize) -> Result<(PathBuf, SetMeta, Vec<(String, Divergence)>), String> {
    let (dir, meta) = find_set(root, set)?;
    let mut diffs = Vec::new();
    for r in replay::check_set(&dir, &meta, replayer, only, false, threads)? {
        match r.outcome {
            Ok(None) => {}
            Ok(Some(d)) => diffs.push((r.file, d)),
            Err(e) => return Err(format!("{}: {e}", r.file)),
        }
    }
    Ok((dir, meta, diffs))
}

/// # Errors
/// Any guard that refuses, or a failure to re-record or write; nothing is written unless all guards pass.
pub fn bless(
    root: &Path,
    set: &str,
    args: &BlessArgs,
    current_rules_version: u32,
    replayer: &dyn Replayer,
    env: &dyn BlessEnv,
    threads: usize,
) -> Result<BlessReport, String> {
    if args.reason.trim().is_empty() {
        return Err("refused: --reason is required (it goes into CHANGES.md)".to_owned());
    }
    let (dir, mut meta) = find_set(root, set)?;
    if meta.rules_version.is_none() {
        return Err(format!("refused: `{set}` is a static set (no spec): there is nothing to re-record"));
    }
    if meta.is_frozen(current_rules_version) && !(args.bugfix && args.issue.is_some()) {
        return Err(format!(
            "refused: `{set}` is frozen (rules version {:?}, current {current_rules_version}): only an ungated fix may re-bless it, with --bugfix --issue ENGINE_KNOWN_ISSUES-ID; otherwise record a new set with `golden bless-new`",
            meta.rules_version
        ));
    }
    if args.expect.is_empty() {
        return Err("refused: --expect SUBTYPE[,SUBTYPE..] is required (the subtypes the fix is meant to change; '*' accepts any)".to_owned());
    }
    if env.dirty()? {
        return Err(format!("refused: uncommitted changes under {}: commit or stash them so the bless is its own diff", root.display()));
    }
    let (_, _, diffs) = diff_set(root, set, replayer, &args.only, threads)?;
    if diffs.is_empty() {
        return Ok(BlessReport::default());
    }
    // Re-record every differing game, compute the collateral before anything is written.
    let mut fresh: Vec<(String, GameFile, BTreeMap<String, usize>)> = Vec::new();
    for (file, _) in &diffs {
        let (old, _) = GameFile::read(&dir.join(file))?;
        let spec = old.header.spec.clone().ok_or_else(|| format!("{file}: no spec in the header"))?;
        let req = RecordReq { spec, bot: old.header.bot.clone(), limits: old.header.limits.clone(), tier: meta.tier, source: old.header.source.clone(), game: old.header.game };
        let new = match replayer.record(&req)? {
            Recording::Game(g) => *g,
            Recording::Excluded(why) => return Err(format!("{file}: re-recording was excluded: {why}")),
        };
        if new.header.rules_version != meta.rules_version {
            return Err(format!("{file}: re-recorded at rules version {:?}, the set is {:?}", new.header.rules_version, meta.rules_version));
        }
        let hist = changed_decisions(&old, &new);
        fresh.push((file.clone(), new, hist));
    }
    let mut histogram: BTreeMap<String, usize> = BTreeMap::new();
    for (_, _, h) in &fresh {
        for (k, n) in h {
            *histogram.entry(k.clone()).or_default() += n;
        }
    }
    if !args.expect.iter().any(|e| e == "*") {
        let collateral: Vec<String> = histogram.iter().filter(|(k, _)| !args.expect.contains(k)).map(|(k, n)| format!("{k} x{n}")).collect();
        if !collateral.is_empty() {
            return Err(format!("refused: collateral change in subtypes outside --expect: {}", collateral.join(", ")));
        }
    }
    // Write.
    for (file, new, _) in &fresh {
        let stem = file.trim_end_matches(&format!(".{}", meta.tier.ext())).to_owned();
        let (name, sha256, bytes) = new.write(&dir, &stem)?;
        let e = meta.games.iter_mut().find(|g| g.file == name).ok_or("game not in set.json")?;
        *e = GameEntry { file: name, steps: new.header.steps, decisions: new.decs.len(), decisions_hash: new.header.decisions_hash.clone(), bytes, sha256 };
    }
    meta.smoke = SetMeta::pick_smoke(&meta.games, meta.smoke.len().max(4));
    meta.write(&dir)?;
    let block = changes_block(&env.now_berlin()?, &env.commit()?, set, &fresh.iter().map(|(f, _, _)| f.clone()).collect::<Vec<_>>(), &histogram, args);
    append_changes(root, &block)?;
    Ok(BlessReport { changed_files: fresh.into_iter().map(|(f, _, _)| f).collect(), histogram, block })
}

fn changes_block(now: &str, commit: &str, set: &str, files: &[String], hist: &BTreeMap<String, usize>, a: &BlessArgs) -> String {
    let h = hist.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ");
    format!(
        "## {now} Berlin  commit {commit}  set {set}\n- games changed ({}): {}\n- changed subtypes: {h}\n- reason: {}\n- issue: {}\n- bugfix of a frozen set: {}\n\n",
        files.len(),
        files.join(", "),
        a.reason.trim(),
        a.issue.as_deref().unwrap_or("-"),
        if a.bugfix { "yes" } else { "no" }
    )
}

/// Append to `CHANGES.md` (created with a header on first use). Never rewrites earlier blocks.
///
/// # Errors
/// An unwritable file.
pub fn append_changes(root: &Path, block: &str) -> Result<(), String> {
    let p = root.join("CHANGES.md");
    let fresh = !p.exists();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    if fresh {
        f.write_all(b"# Golden bless log\n\nAppend-only, written by `golden bless` / `golden bless-new`. Every changed subtype below must be named in the commit message of the rule fix.\n\n")
            .map_err(|e| e.to_string())?;
    }
    f.write_all(block.as_bytes()).map_err(|e| e.to_string())
}

/// Log a `bless-new` / first record in `CHANGES.md`.
///
/// # Errors
/// An unwritable file.
pub fn log_new_set(root: &Path, env: &dyn BlessEnv, set: &str, tier: Tier, version: Option<u32>, games: usize, why: &str) -> Result<(), String> {
    append_changes(
        root,
        &format!("## {} Berlin  commit {}  NEW set {set}\n- tier {tier:?}, rules version {version:?}, {games} games\n- reason: {why}\n\n", env.now_berlin()?, env.commit()?),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden::format::{Dec, blank_header, tests::scratch};
    use crate::golden::replay::{DivKind, Replayer};
    use ti4_selfplay::spec::GameSpec;

    fn spec(seed: u64, version: u32) -> GameSpec {
        GameSpec {
            setup: "t".into(),
            seed,
            players: 3,
            map: "3pLearnToPlayPoK".into(),
            factions: vec!["a".into(), "b".into(), "c".into()],
            speaker: "seat".into(),
            card_set: None,
            rotation: None,
            starting_tech: true,
            thunders_edge: false,
            rules_version: Some(version),
        }
    }

    fn game(seed: u64, version: u32, answers: &[(&str, &str)]) -> GameFile {
        let mut header = blank_header(Some(spec(seed, version)));
        header.steps = 10;
        header.decisions = answers.len();
        header.decisions_hash = format!("hash{seed}");
        let mut subtypes: Vec<String> = Vec::new();
        let decs: Vec<Dec> = answers
            .iter()
            .enumerate()
            .map(|(i, (sub, ans))| {
                if !subtypes.iter().any(|s| s == sub) {
                    subtypes.push((*sub).into());
                }
                Dec { step: i, k: 0, chosen: (*ans).into(), v2: format!("v2{ans}"), subtype: (*sub).into(), state: None, raw: None }
            })
            .collect();
        header.subtypes = Some(subtypes);
        GameFile { tier: Tier::Slim, header, decs, checkpoints: vec![(0, "0000000000000000".into())] }
    }

    /// A fake engine: its "rules" are a table seed -> answers; a game reproduces when its recorded answers equal it.
    struct Fake {
        rules: BTreeMap<u64, Vec<(String, String)>>,
    }
    impl Replayer for Fake {
        fn check(&self, g: &GameFile) -> Result<Option<Divergence>, String> {
            let seed = g.header.spec.as_ref().unwrap().seed;
            let now = self.rules[&seed].clone();
            let rec: Vec<(String, String)> = g.decs.iter().map(|d| (d.subtype.clone(), d.chosen.clone())).collect();
            for (i, (a, b)) in rec.iter().zip(&now).enumerate() {
                if a != b {
                    return Ok(Some(Divergence {
                        step: i,
                        k: 0,
                        kind: DivKind::Answer,
                        recorded_answer: Some(a.1.clone()),
                        offered: vec![],
                        recorded_v2: None,
                        actual_v2: None,
                        subtype: b.0.clone(),
                        detail: String::new(),
                        choice_diff: vec![],
                    }));
                }
            }
            Ok(None)
        }
        fn record(&self, req: &RecordReq) -> Result<Recording, String> {
            let now = self.rules[&req.spec.seed].clone();
            let answers: Vec<(&str, &str)> = now.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            Ok(Recording::Game(Box::new(game(req.spec.seed, req.spec.rules_version.unwrap(), &answers))))
        }
    }

    struct Env {
        dirty: bool,
    }
    impl BlessEnv for Env {
        fn dirty(&self) -> Result<bool, String> {
            Ok(self.dirty)
        }
        fn commit(&self) -> Result<String, String> {
            Ok("abc1234".into())
        }
        fn now_berlin(&self) -> Result<String, String> {
            Ok("2026-10-30 12:00".into())
        }
    }

    fn rules(pairs: &[(u64, &[(&str, &str)])]) -> Fake {
        Fake { rules: pairs.iter().map(|(s, a)| (*s, a.iter().map(|(x, y)| ((*x).to_owned(), (*y).to_owned())).collect())).collect() }
    }

    const G1: &[(&str, &str)] = &[("pick", "p1"), ("move", "m1"), ("move", "m2")];
    const G2: &[(&str, &str)] = &[("pick", "p2"), ("score", "s1")];

    /// A set `v35-t` of two slim games recorded under the rules `G1`/`G2`.
    fn fixture(name: &str, version: u32) -> PathBuf {
        let root = scratch(name);
        let dir = root.join(format!("v{version}-t"));
        let games = vec![("g000".to_owned(), game(1, version, G1)), ("g001".to_owned(), game(2, version, G2))];
        let entries = save_games(&dir, &games).unwrap();
        let smoke = SetMeta::pick_smoke(&entries, 4);
        SetMeta { name: format!("v{version}-t"), tier: Tier::Slim, rules_version: Some(version), frozen: false, source: "test".into(), note: String::new(), games: entries, smoke }
            .write(&dir)
            .unwrap();
        root
    }

    fn args(expect: &[&str]) -> BlessArgs {
        BlessArgs { reason: "the fix".into(), expect: expect.iter().map(|s| (*s).into()).collect(), ..Default::default() }
    }

    #[test]
    fn an_unchanged_engine_blesses_nothing_and_writes_no_log() {
        let root = fixture("bless_nothing", 35);
        let r = bless(&root, "v35-t", &args(&["move"]), 35, &rules(&[(1, G1), (2, G2)]), &Env { dirty: false }, 1).unwrap();
        assert!(r.changed_files.is_empty());
        assert!(!root.join("CHANGES.md").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bless_rerecords_only_the_changed_game_and_logs_it() {
        let root = fixture("bless_one", 35);
        let before = std::fs::read(root.join("v35-t/g001.tsv.zst")).unwrap();
        let changed: &[(&str, &str)] = &[("pick", "p1"), ("move", "m1"), ("move", "m9")];
        let r = bless(&root, "v35-t", &args(&["move"]), 35, &rules(&[(1, changed), (2, G2)]), &Env { dirty: false }, 2).unwrap();
        assert_eq!(r.changed_files, ["g000.tsv.zst"]);
        assert_eq!(r.histogram.get("move"), Some(&1));
        assert_eq!(std::fs::read(root.join("v35-t/g001.tsv.zst")).unwrap(), before, "the game that did not change is not rewritten");
        let log = std::fs::read_to_string(root.join("CHANGES.md")).unwrap();
        assert!(log.contains("commit abc1234") && log.contains("changed subtypes: move 1") && log.contains("reason: the fix"), "{log}");
        // the set reproduces now
        let (dir, meta) = find_set(&root, "v35-t").unwrap();
        assert!(replay::check_set(&dir, &meta, &rules(&[(1, changed), (2, G2)]), &[], false, 1).unwrap().iter().all(replay::GameResult::ok));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bless_refuses_a_frozen_set_without_bugfix_and_issue() {
        let root = fixture("bless_frozen", 34);
        let changed: &[(&str, &str)] = &[("pick", "p9"), ("move", "m1"), ("move", "m2")];
        let fake = rules(&[(1, changed), (2, G2)]);
        let e = bless(&root, "v34-t", &args(&["pick"]), 35, &fake, &Env { dirty: false }, 1).unwrap_err();
        assert!(e.contains("frozen"), "{e}");
        let mut a = args(&["pick"]);
        a.bugfix = true;
        assert!(bless(&root, "v34-t", &a, 35, &fake, &Env { dirty: false }, 1).unwrap_err().contains("frozen"), "--bugfix alone is not enough");
        a.issue = Some("D4-found-01".into());
        let r = bless(&root, "v34-t", &a, 35, &fake, &Env { dirty: false }, 1).unwrap();
        assert_eq!(r.changed_files.len(), 1);
        assert!(std::fs::read_to_string(root.join("CHANGES.md")).unwrap().contains("issue: D4-found-01"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bless_refuses_collateral_a_missing_reason_a_missing_expect_and_a_dirty_tree() {
        let root = fixture("bless_guards", 35);
        let changed: &[(&str, &str)] = &[("pick", "p9"), ("move", "m9"), ("move", "m2")];
        let fake = rules(&[(1, changed), (2, G2)]);
        let e = bless(&root, "v35-t", &args(&["move"]), 35, &fake, &Env { dirty: false }, 1).unwrap_err();
        assert!(e.contains("collateral") && e.contains("pick x1"), "{e}");
        let mut a = args(&["move"]);
        a.reason = "  ".into();
        assert!(bless(&root, "v35-t", &a, 35, &fake, &Env { dirty: false }, 1).unwrap_err().contains("--reason"));
        assert!(bless(&root, "v35-t", &args(&[]), 35, &fake, &Env { dirty: false }, 1).unwrap_err().contains("--expect"));
        assert!(bless(&root, "v35-t", &args(&["move", "pick"]), 35, &fake, &Env { dirty: true }, 1).unwrap_err().contains("uncommitted"));
        // none of the refusals wrote anything
        assert!(!root.join("CHANGES.md").exists());
        let (dir, meta) = find_set(&root, "v35-t").unwrap();
        assert!(replay::check_set(&dir, &meta, &rules(&[(1, G1), (2, G2)]), &[], false, 1).unwrap().iter().all(replay::GameResult::ok));
        // '*' accepts any subtype, explicitly
        assert!(bless(&root, "v35-t", &args(&["*"]), 35, &fake, &Env { dirty: false }, 1).is_ok());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn check_reports_the_first_divergence_of_a_planted_difference() {
        let root = fixture("check_planted", 35);
        let changed: &[(&str, &str)] = &[("pick", "p1"), ("move", "mX"), ("move", "m2")];
        let (dir, meta) = find_set(&root, "v35-t").unwrap();
        let results = replay::check_set(&dir, &meta, &rules(&[(1, changed), (2, G2)]), &[], false, 2).unwrap();
        assert!(results[1].ok());
        let d = results[0].outcome.as_ref().unwrap().as_ref().unwrap();
        assert_eq!((d.step, d.kind, d.recorded_answer.as_deref(), d.subtype.as_str()), (1, DivKind::Answer, Some("m1"), "move"));
        // editing a file outside the tool is caught by the sha in set.json
        let mut meta2 = meta.clone();
        meta2.games[0].sha256 = "0".repeat(64);
        let r = replay::check_set(&dir, &meta2, &rules(&[(1, G1), (2, G2)]), &[], false, 1).unwrap();
        assert!(r[0].outcome.as_ref().unwrap_err().contains("sha256"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn changed_decisions_counts_each_subtype_once_per_decision() {
        let a = game(1, 35, G1);
        let b = game(1, 35, &[("pick", "p1"), ("move", "m5"), ("score", "s1"), ("score", "s2")]);
        let h = changed_decisions(&a, &b);
        assert_eq!(h, BTreeMap::from([("move".to_owned(), 1), ("score".to_owned(), 2)]));
    }
}
