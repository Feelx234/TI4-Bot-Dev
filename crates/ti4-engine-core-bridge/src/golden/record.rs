//! Recording sets: `record` (bot-driven, optionally dual-clean), `ladder`, `finalize`, `import-corpus`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use ti4_selfplay::spec::GameSpec;

use super::bless::save_games;
use super::corpus::{Plan, hex_of};
use super::format::{GameFile, Header, Limits, Tier, blank_header, dec_of_full_line};
use super::replay::{RecordReq, Recording, Replayer};
use super::sets::{GameEntry, SetMeta};

/// A game to record: its index in the set (the file is `g<index>`) and the table.
#[derive(Clone)]
pub struct RecordJob {
    pub index: usize,
    pub spec: GameSpec,
    pub source: Option<String>,
    pub game: Option<usize>,
}

pub struct RecordOpts {
    pub dir: PathBuf,
    pub tier: Tier,
    pub bot: BTreeMap<String, String>,
    pub limits: Limits,
    pub workers: usize,
    /// `(i, n)`: play only the jobs with `index % n == i` (separate processes, one `finalize` afterwards).
    pub shard: Option<(usize, usize)>,
}

#[derive(Debug, Default)]
pub struct RecordSummary {
    pub recorded: Vec<String>,
    /// `(index, reason)` of the games a dual difference kept out of the set.
    pub excluded: Vec<(usize, String)>,
}

/// The file stem of game `index`: `g000`.
#[must_use]
pub fn stem(index: usize) -> String {
    format!("g{index:03}")
}

/// Record the jobs with the bot (and the dual check the replayer is configured with), one file per game.
///
/// # Errors
/// A game that cannot be played or a failed `all=core` comparison (a porting bug); an unwritable directory.
pub fn record_games(opts: &RecordOpts, jobs: &[RecordJob], replayer: &dyn Replayer) -> Result<RecordSummary, String> {
    std::fs::create_dir_all(&opts.dir).map_err(|e| format!("{}: {e}", opts.dir.display()))?;
    let todo: Vec<&RecordJob> = jobs.iter().filter(|j| opts.shard.is_none_or(|(i, n)| j.index % n == i)).collect();
    let workers = if replayer.serial() { 1 } else { opts.workers.max(1) };
    let next = AtomicUsize::new(0);
    // Each game is saved the moment it is recorded (a run that is killed keeps what it has; `golden ladder` skips
    // the versions that exist).
    let results: Vec<Mutex<Option<Result<Option<String>, String>>>> = todo.iter().map(|_| Mutex::new(None)).collect();
    let excluded: Mutex<Vec<(usize, String)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(job) = todo.get(i) else { break };
                    let req = RecordReq { spec: job.spec.clone(), bot: opts.bot.clone(), limits: opts.limits.clone(), tier: opts.tier, source: job.source.clone(), game: job.game };
                    let started = std::time::Instant::now();
                    let r = replayer.record(&req).and_then(|rec| match rec {
                        Recording::Game(g) => save_games(&opts.dir, &[(stem(job.index), *g)]).map(|_| Some(stem(job.index))),
                        Recording::Excluded(why) => {
                            excluded.lock().expect("lock").push((job.index, why));
                            Ok(None)
                        }
                    });
                    eprintln!("recorded {} in {:.1}s", stem(job.index), started.elapsed().as_secs_f64());
                    *results[i].lock().expect("lock") = Some(r);
                }
            });
        }
    });
    let mut summary = RecordSummary::default();
    for cell in results {
        if let Some(name) = cell.into_inner().expect("lock").expect("ran")? {
            summary.recorded.push(name);
        }
    }
    summary.excluded = excluded.into_inner().expect("lock");
    summary.excluded.sort();
    if !summary.excluded.is_empty() {
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(opts.dir.join("excluded.txt")).map_err(|e| e.to_string())?;
        for (i, why) in &summary.excluded {
            writeln!(f, "{}\t{}", stem(*i), why.replace('\n', " ")).map_err(|e| e.to_string())?;
        }
    }
    Ok(summary)
}

/// Build `set.json` from the game files in `dir`.
///
/// # Errors
/// A set whose games are not all at one rules version, or an unreadable file.
pub fn finalize_set(dir: &Path, name: &str, source: &str, note: &str) -> Result<SetMeta, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            n.starts_with('g') && (n.ends_with(Tier::Full.ext()) || n.ends_with(Tier::Slim.ext()))
        })
        .collect();
    files.sort();
    let mut games = Vec::new();
    let mut tier = None;
    let mut versions = std::collections::BTreeSet::new();
    for p in &files {
        let (g, sha) = GameFile::read(p)?;
        if tier.is_some_and(|t| t != g.tier) {
            return Err(format!("{}: mixed tiers in one set", dir.display()));
        }
        tier = Some(g.tier);
        versions.insert(g.header.rules_version);
        games.push(GameEntry {
            file: p.file_name().expect("name").to_string_lossy().into_owned(),
            steps: g.header.steps,
            decisions: g.decs.len(),
            decisions_hash: g.header.decisions_hash.clone(),
            bytes: std::fs::metadata(p).map_err(|e| e.to_string())?.len(),
            sha256: sha,
        });
    }
    let tier = tier.ok_or_else(|| format!("{}: no game files", dir.display()))?;
    if versions.len() != 1 {
        return Err(format!("{}: the games are at rules versions {versions:?}; a set has exactly one", dir.display()));
    }
    let smoke = SetMeta::pick_smoke(&games, 4);
    let old = SetMeta::read(dir).ok();
    let meta = SetMeta {
        name: name.to_owned(),
        tier,
        rules_version: *versions.iter().next().expect("one"),
        frozen: old.as_ref().is_some_and(|o| o.frozen),
        source: source.to_owned(),
        note: note.to_owned(),
        ladder: false,
        games,
        smoke,
    };
    meta.write(dir)?;
    Ok(meta)
}

// ---- ladder --------------------------------------------------------------------------------------------

/// One short slim game at every rules version `0..=current`: 3 players on the small learn-to-play map, 400 steps.
#[must_use]
pub fn ladder_jobs(current: u32) -> Vec<RecordJob> {
    let pool = ti4_engine::seating::LOBBY_FACTIONS;
    (0..=current)
        .map(|v| RecordJob {
            index: v as usize,
            spec: GameSpec {
                setup: "ladder".to_owned(),
                seed: 1000 + u64::from(v),
                players: 3,
                map: "3pLearnToPlayPoK".to_owned(),
                factions: (0..3).map(|s| pool[(v as usize * 3 + s) % pool.len()].to_owned()).collect(),
                speaker: "seat".to_owned(),
                card_set: None,
                rotation: None,
                starting_tech: true,
                thunders_edge: false,
                rules_version: Some(v),
            },
            source: Some("ladder".to_owned()),
            game: Some(v as usize),
        })
        .collect()
}

pub const LADDER_STEPS: usize = 400;

// ---- import of the X2 corpus ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct GameNo {
    g: usize,
}

/// What the importer needs of one X2 source, already read (so it can be tested without the repo's configs).
pub struct ImportInput {
    pub source: String,
    /// `None` for `fixtures` (static: nothing to play).
    pub plan: Option<Plan>,
    /// The manifest entry of the source: `{"games": [{game, steps, finished, decisions_hash}], "sha256": ..}`.
    pub manifest: Value,
    pub lines: Vec<String>,
}

/// Split one X2 source per game, lines byte-identical. Returns `(set name, relative dir, games)` per rules version.
///
/// # Errors
/// A source whose lines do not hash to the manifest, or a game with no plan entry.
pub fn import_games(input: &ImportInput) -> Result<Vec<(String, String, Vec<(String, GameFile)>)>, String> {
    // The proof of byte identity: the lines in file order hash to the manifest's sha256.
    let mut h = Sha256::new();
    for l in &input.lines {
        h.update(l.as_bytes());
        h.update(b"\n");
    }
    if input.manifest["sha256"].as_str() != Some(&hex_of(&h.finalize())) {
        return Err(format!("{}: the lines do not hash to the manifest's sha256: not importing a corpus that fails its own check", input.source));
    }
    let mut by_game: BTreeMap<usize, Vec<&String>> = BTreeMap::new();
    for l in &input.lines {
        let g: GameNo = serde_json::from_str(l).map_err(|e| format!("{}: {e}", input.source))?;
        by_game.entry(g.g).or_default().push(l);
    }
    let man_games: BTreeMap<usize, &Value> =
        input.manifest["games"].as_array().map(|a| a.iter().filter_map(|g| Some((g["game"].as_u64()? as usize, g))).collect()).unwrap_or_default();
    let mut by_version: BTreeMap<Option<u32>, Vec<(String, GameFile)>> = BTreeMap::new();
    for (game_no, lines) in by_game {
        let decs = lines.iter().map(|l| dec_of_full_line(l)).collect::<Result<Vec<_>, _>>().map_err(|e| format!("{} game {game_no}: {e}", input.source))?;
        let mut header: Header;
        let version;
        match &input.plan {
            None => {
                header = blank_header(None);
                header.decisions_hash = String::new();
                header.finished = true;
                header.source = Some(input.source.clone());
                header.game = Some(game_no);
                version = None;
            }
            Some(plan) => {
                let job = plan.jobs.iter().find(|j| j.game_no == game_no).ok_or_else(|| format!("{} game {game_no}: not in the source's plan", input.source))?;
                let m = man_games.get(&game_no).ok_or_else(|| format!("{} game {game_no}: not in the manifest", input.source))?;
                let mut spec = job.spec.clone();
                let effective = match spec.rules_version {
                    Some(v) => v,
                    None => spec.build()?.state.rules_version,
                };
                spec.rules_version = Some(effective);
                header = blank_header(Some(spec));
                header.rules_version = Some(effective);
                header.bot = plan.bot.clone();
                header.limits = Limits { max_rounds: plan.max_rounds, max_steps: job.max_steps };
                header.steps = m["steps"].as_u64().ok_or("manifest: steps")? as usize;
                header.decisions_hash = m["decisions_hash"].as_str().ok_or("manifest: decisions_hash")?.to_owned();
                header.finished = m["finished"].as_bool().ok_or("manifest: finished")?;
                header.source = Some(input.source.clone());
                header.game = Some(game_no);
                version = Some(effective);
            }
        }
        header.decisions = decs.len();
        by_version.entry(version).or_default().push((format!("g{game_no:03}"), GameFile { tier: Tier::Full, header, decs, checkpoints: Vec::new() }));
    }
    let split = by_version.len() > 1;
    Ok(by_version
        .into_iter()
        .map(|(v, games)| {
            let suffix = if split { format!("-v{}", v.map_or("static".to_owned(), |v| v.to_string())) } else { String::new() };
            if input.source == "golden20" {
                let n = format!("v{}-golden20{suffix}", v.unwrap_or(0));
                (n.clone(), n, games)
            } else {
                let n = format!("legacy-{}{suffix}", input.source);
                (n.clone(), format!("legacy-sources/{}{suffix}", input.source), games)
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden::corpus::{Job, sha_hex};
    use crate::golden::format::tests::scratch;
    use serde_json::json;

    fn spec(seed: u64) -> GameSpec {
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
            rules_version: Some(23),
        }
    }

    fn line(g: usize, step: usize, k: usize, chosen: &str) -> String {
        format!(r#"{{"s":"golden20","g":{g},"step":{step},"k":{k},"state":"00aa00aa00aa00aa","choice":{{"player":"a","prompt":"p","options":[],"context":{{"subtype":"t"}}}},"chosen":"{chosen}","v1":"1111111111111111","v2":"2222222222222222"}}"#)
    }

    fn input() -> ImportInput {
        let lines = vec![line(0, 0, 0, "a"), line(0, 1, 0, "b"), line(1, 0, 0, "c")];
        let mut h = Sha256::new();
        for l in &lines {
            h.update(l.as_bytes());
            h.update(b"\n");
        }
        ImportInput {
            source: "golden20".into(),
            plan: Some(Plan {
                jobs: vec![Job { game_no: 0, spec: spec(1), max_steps: 400_000 }, Job { game_no: 1, spec: spec(2), max_steps: 400_000 }],
                max_rounds: 50,
                bot: BTreeMap::new(),
                note: String::new(),
            }),
            manifest: json!({"sha256": hex_of(&h.finalize()), "games": [
                {"game": 0, "steps": 5, "finished": true, "decisions_hash": "h0"},
                {"game": 1, "steps": 3, "finished": false, "decisions_hash": "h1"}]}),
            lines,
        }
    }

    #[test]
    fn import_splits_per_game_keeps_lines_and_is_idempotent() {
        let sets = import_games(&input()).unwrap();
        assert_eq!(sets.len(), 1);
        let (name, dir, games) = &sets[0];
        assert_eq!((name.as_str(), dir.as_str()), ("v23-golden20", "v23-golden20"));
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].1.header.steps, 5);
        assert_eq!(games[0].1.header.rules_version, Some(23));
        assert_eq!(games[0].1.decs.len(), 2);
        let joined: Vec<&str> = games.iter().flat_map(|(_, g)| g.decs.iter().map(|d| d.raw.as_deref().unwrap())).collect();
        assert_eq!(joined, input().lines.iter().map(String::as_str).collect::<Vec<_>>(), "the lines are byte-identical and in order");
        // written twice: the same bytes
        let root = scratch("import_idem");
        save_games(&root.join("a"), games).unwrap();
        save_games(&root.join("b"), &import_games(&input()).unwrap()[0].2).unwrap();
        for f in ["g000.jsonl.zst", "g001.jsonl.zst"] {
            assert_eq!(std::fs::read(root.join("a").join(f)).unwrap(), std::fs::read(root.join("b").join(f)).unwrap());
        }
        let meta = finalize_set(&root.join("a"), "v23-golden20", "x2", "").unwrap();
        assert_eq!((meta.rules_version, meta.games.len(), meta.decisions()), (Some(23), 2, 3));
        let _ = sha_hex(b"", 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn import_refuses_lines_that_do_not_hash_to_the_manifest() {
        let mut i = input();
        i.lines[1] = line(0, 1, 0, "tampered");
        assert!(import_games(&i).unwrap_err().contains("sha256"));
    }

    #[test]
    fn a_source_with_two_rules_versions_splits_into_two_sets() {
        let mut i = input();
        i.plan.as_mut().unwrap().jobs[1].spec.rules_version = Some(24);
        let sets = import_games(&i).unwrap();
        let names: Vec<&str> = sets.iter().map(|s| s.0.as_str()).collect();
        assert_eq!(names, ["v23-golden20-v23", "v24-golden20-v24"]);
    }

    #[test]
    fn the_ladder_has_one_game_per_version() {
        let jobs = ladder_jobs(35);
        assert_eq!(jobs.len(), 36);
        assert!(jobs.iter().enumerate().all(|(i, j)| j.spec.rules_version == Some(i as u32) && j.index == i));
        let mut seeds: Vec<u64> = jobs.iter().map(|j| j.spec.seed).collect();
        seeds.dedup();
        assert_eq!(seeds.len(), 36);
    }

    #[test]
    fn finalize_refuses_mixed_versions() {
        let root = scratch("finalize_mixed");
        let mk = |v: u32| {
            let mut h = blank_header(Some(spec(1)));
            h.rules_version = Some(v);
            GameFile { tier: Tier::Slim, header: h, decs: vec![], checkpoints: vec![] }
        };
        save_games(&root, &[("g000".into(), mk(1)), ("g001".into(), mk(2))]).unwrap();
        assert!(finalize_set(&root, "x", "t", "").unwrap_err().contains("exactly one"));
        let _ = std::fs::remove_dir_all(root);
    }
}
