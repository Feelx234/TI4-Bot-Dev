//! The recorded decision corpus (docs/ENGINE_CORE_STATE.md 2.3, package X2; docs/DECISION_FAMILIES.md).
//!
//! ```text
//! decision_corpus record DIR [SOURCE...] [--workers N]   play the sources with the current string engine
//!                                                        and write DIR/<source>.jsonl.zst + manifest.json
//! decision_corpus verify DIR [SOURCE...] [--workers N]   play them again and compare with DIR line by line
//! decision_corpus check  DIR [SOURCE...]                 no engine run: the files against the manifest, the
//!                                                        recorded Choice JSON against its V1/V2 hashes
//! decision_corpus list                                   the sources
//! ```
//! Sources: `golden20` (tools/engine_profile/profile-20.json, all 20 games), `bench`, `bench-v1`..`bench-v5`
//! (the bench configurations of tools/rulebot/selfplay.sh, a few games each), `templates` (the first
//! decisions of every map template's opening, with and without Thunder's Edge), `te` (Thunder's Edge
//! tables seating every lobby faction), `fixtures` (the Choice objects inside the checked-in wire fixtures).
//!
//! One line per decision: `{"s":source,"g":game,"step":n,"k":ordinal in the step,"state":H,"choice":Choice,
//! "chosen":id,"v1":H,"v2":H}`. `state` is the first 16 hex digits of the SHA-256 of the serialised
//! `GameState` at the START of the step that asked (the engine reconciles derived facts and then asks, and
//! a decider cannot see the state, so a step's decisions share one state hash). `v1`/`v2` are the first
//! 16 hex digits of `fingerprint::decision_hash` of the `DecisionRecord` the engine logged.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use ti4_engine_core_bridge::golden::corpus::{Played, SOURCES, check_line, fixture_lines, hex_of, play, plan_for};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// The sources, the bot-driven `play`, the line format and `check_line` live in
// `ti4_engine_core_bridge::golden::corpus` (shared with the `golden` tool); this binary is the CLI of the X2 corpus.

// ---- running a source ----------------------------------------------------------------------------------

struct SourceRun {
    /// Per game, its lines (in game order).
    games: Vec<Vec<String>>,
    summary: Value,
}

fn run_source(source: &str, workers: usize) -> Result<SourceRun, String> {
    if source == "fixtures" {
        let lines = fixture_lines()?;
        let n = lines.len();
        return Ok(SourceRun { games: vec![lines], summary: json!({ "decisions": n, "note": "Choice objects inside checked-in wire fixtures" }) });
    }
    let plan = plan_for(source)?.ok_or_else(|| format!("unknown source {source}"))?;
    let tuning = ti4_selfplay::driver::tuning_from(&plan.bot)?;
    let next = AtomicUsize::new(0);
    let results: Vec<Mutex<Option<Result<Played, String>>>> = plan.jobs.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                ti4_rule_bot::tuning::set(tuning.clone());
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some(job) = plan.jobs.get(i) else { break };
                    let r = play(source, job.game_no, &job.spec, plan.max_rounds, job.max_steps, None);
                    *results[i].lock().expect("lock") = Some(r);
                }
            });
        }
    });
    let mut games = Vec::new();
    let mut per_game = Vec::new();
    for (job, cell) in plan.jobs.iter().zip(results) {
        let played = cell.into_inner().expect("lock").expect("ran")?;
        per_game.push(json!({
            "game": job.game_no, "map": job.spec.map, "seed": job.spec.seed, "players": job.spec.players,
            "thunders_edge": job.spec.thunders_edge, "steps": played.steps, "finished": played.finished,
            "decisions": played.lines.len(), "decisions_hash": played.game_hash,
        }));
        games.push(played.lines);
    }
    let n: usize = games.iter().map(Vec::len).sum();
    Ok(SourceRun { games, summary: json!({ "decisions": n, "note": plan.note, "games": per_game }) })
}

// ---- files ---------------------------------------------------------------------------------------------

fn file_of(dir: &Path, source: &str) -> PathBuf {
    dir.join(format!("{source}.jsonl.zst"))
}

fn write_source(dir: &Path, source: &str, run: &SourceRun) -> Result<Value, String> {
    let path = file_of(dir, source);
    let file = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = zstd::Encoder::new(file, 19).map_err(|e| e.to_string())?;
    enc.long_distance_matching(true).map_err(|e| e.to_string())?;
    enc.window_log(27).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut raw = 0usize;
    for l in run.games.iter().flatten() {
        enc.write_all(l.as_bytes()).map_err(|e| e.to_string())?;
        enc.write_all(b"\n").map_err(|e| e.to_string())?;
        hasher.update(l.as_bytes());
        hasher.update(b"\n");
        raw += l.len() + 1;
    }
    enc.finish().map_err(|e| e.to_string())?;
    let mut entry = run.summary.clone();
    entry["file"] = json!(path.file_name().expect("name").to_string_lossy());
    entry["raw_bytes"] = json!(raw);
    entry["zst_bytes"] = json!(std::fs::metadata(&path).map_err(|e| e.to_string())?.len());
    entry["sha256"] = json!(hex_of(&hasher.finalize()));
    Ok(entry)
}

fn read_lines(dir: &Path, source: &str) -> Result<Vec<String>, String> {
    let path = file_of(dir, source);
    let file = std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let dec = zstd::Decoder::new(file).map_err(|e| e.to_string())?;
    BufReader::new(dec).lines().map(|l| l.map_err(|e| e.to_string())).collect()
}

fn manifest(dir: &Path) -> Result<Value, String> {
    let p = dir.join("manifest.json");
    serde_json::from_str(&std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?).map_err(|e| e.to_string())
}

// ---- main ----------------------------------------------------------------------------------------------

fn main() {
    ti4_engine::core_sync::unmapped::set_strict_for_gate();
    ti4_engine::core_sync::unsupported::set_disabled_policy_for_gate();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut workers = 4usize;
    let mut positional = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--workers" {
            workers = it.next().and_then(|n| n.parse().ok()).expect("--workers N");
        } else {
            positional.push(a.clone());
        }
    }
    let Some(mode) = positional.first().cloned() else {
        eprintln!("usage: decision_corpus record|verify|check DIR [SOURCE...] [--workers N] | list");
        std::process::exit(2);
    };
    if mode == "list" {
        println!("{}", SOURCES.join("\n"));
        return;
    }
    let dir = PathBuf::from(positional.get(1).expect("DIR"));
    let sources: Vec<String> = if positional.len() > 2 { positional[2..].to_vec() } else { SOURCES.iter().map(|s| (*s).to_owned()).collect() };
    let mut failed = false;
    let mut man = if mode == "record" {
        std::fs::create_dir_all(&dir).expect("dir");
        manifest(&dir).unwrap_or_else(|_| json!({ "format": 1, "sources": {} }))
    } else {
        manifest(&dir).expect("manifest.json")
    };
    for source in &sources {
        let started = std::time::Instant::now();
        match mode.as_str() {
            "record" => match run_source(source, workers).and_then(|r| write_source(&dir, source, &r)) {
                Ok(entry) => {
                    println!("{source}: {} decisions, {} raw bytes -> {} zst ({:.1}s)", entry["decisions"], entry["raw_bytes"], entry["zst_bytes"], started.elapsed().as_secs_f64());
                    man["sources"][source] = entry;
                }
                Err(e) => {
                    println!("FAIL {source}: {e}");
                    failed = true;
                }
            },
            "verify" => {
                let result = (|| -> Result<(usize, usize), String> {
                    let stored = read_lines(&dir, source)?;
                    let run = run_source(source, workers)?;
                    let mut n = 0usize;
                    for l in run.games.iter().flatten() {
                        match stored.get(n) {
                            Some(s) if s == l => {}
                            Some(s) => return Err(format!("decision {n} differs\n  recorded: {}\n  now:      {}", &s[..s.len().min(600)], &l[..l.len().min(600)])),
                            None => return Err(format!("decision {n} is new")),
                        }
                        n += 1;
                    }
                    if n != stored.len() {
                        return Err(format!("{} decisions recorded, {n} now", stored.len()));
                    }
                    let games: Vec<&Value> = run.summary["games"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
                    let hashes: Vec<&Value> = games.iter().map(|g| &g["decisions_hash"]).collect();
                    let recorded: Vec<&Value> = man["sources"][source]["games"].as_array().map(|a| a.iter().map(|g| &g["decisions_hash"]).collect()).unwrap_or_default();
                    if hashes != recorded {
                        return Err("game decision hashes differ from the manifest".to_owned());
                    }
                    Ok((n, games.len()))
                })();
                match result {
                    Ok((n, g)) => println!("ok   {source}: {n} decisions of {g} games identical (state, Choice JSON, V1, V2) ({:.1}s)", started.elapsed().as_secs_f64()),
                    Err(e) => {
                        println!("FAIL {source}: {e}");
                        failed = true;
                    }
                }
            }
            "check" => {
                let result = (|| -> Result<usize, String> {
                    let lines = read_lines(&dir, source)?;
                    let mut hasher = Sha256::new();
                    for (i, l) in lines.iter().enumerate() {
                        hasher.update(l.as_bytes());
                        hasher.update(b"\n");
                        check_line(l).map_err(|e| format!("line {i}: {e}"))?;
                    }
                    if man["sources"][source]["sha256"].as_str() != Some(&hex_of(&hasher.finalize())) {
                        return Err("sha256 differs from the manifest".to_owned());
                    }
                    if man["sources"][source]["decisions"].as_u64() != Some(lines.len() as u64) {
                        return Err("decision count differs from the manifest".to_owned());
                    }
                    Ok(lines.len())
                })();
                match result {
                    Ok(n) => println!("ok   {source}: {n} decisions: sha256, Choice round trip, V1/V2 recomputed"),
                    Err(e) => {
                        println!("FAIL {source}: {e}");
                        failed = true;
                    }
                }
            }
            _ => {
                eprintln!("unknown mode {mode}");
                std::process::exit(2);
            }
        }
    }
    if mode == "record" {
        std::fs::write(dir.join("manifest.json"), serde_json::to_string_pretty(&man).expect("manifest") + "\n").expect("write manifest");
    }
    if failed {
        std::process::exit(1);
    }
}
