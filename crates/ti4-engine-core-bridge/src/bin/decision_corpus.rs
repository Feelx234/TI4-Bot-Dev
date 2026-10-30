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

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{cell::RefCell, rc::Rc};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use ti4_engine::choice::{Choice, ChoiceOption, Decider, DecisionLog, IllegalChoice, SeatObservation, SeededRandom};
use ti4_engine::fingerprint::{CanonicalHashVersion, decision_hash};
use ti4_selfplay::spec::{GameSpec, Matrix, SEATS};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const SOURCES: [&str; 10] = [
    "golden20", "bench", "bench-v1", "bench-v2", "bench-v3", "bench-v4", "bench-v5", "templates", "te", "fixtures",
];
/// The bench games recorded in full: seeds 21, 22 of the 4-player setup and seeds 21, 22 of the 6-player
/// setup (indices into the expanded config; the first ten of each setup are `golden20`).
const BENCH_PICKS: [usize; 4] = [20, 21, 120, 121];
/// Steps played from a template's opening.
const OPENING_STEPS: usize = 60;
/// Steps played at a Thunder's Edge table.
const TE_STEPS: usize = 5000;

fn sha_hex(bytes: &[u8], digits: usize) -> String {
    let d = Sha256::digest(bytes);
    let mut s = hex_of(&d);
    s.truncate(digits);
    s
}

fn hex_of(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

// ---- playing -------------------------------------------------------------------------------------------

type Captured = Rc<RefCell<Vec<(Choice, String)>>>;

struct Bot {
    seed: u64,
    decided: usize,
    random: SeededRandom,
    captured: Captured,
}

impl Bot {
    fn decide(&mut self, choice: &Choice, seen: Option<&SeatObservation<'_>>) -> Result<ChoiceOption, IllegalChoice> {
        let index = self.decided;
        let option = match seen {
            Some(seen) if !choice.options.is_empty() => {
                let seat = SEATS.iter().position(|s| *s == choice.player.as_str()).unwrap_or(0);
                // The thread's tuning was installed by `run_source` (`selfplay run --bot bot_version=N`).
                let (picked, _, _) = ti4_rule_bot::choose_explained(choice, Some(seen), self.seed, seat, index);
                choice.options[picked].clone()
            }
            _ => self.random.choose(choice)?,
        };
        self.decided += 1;
        self.captured.borrow_mut().push((choice.clone(), option.id.clone()));
        Ok(option)
    }
}

struct Shared(Rc<RefCell<Bot>>);

impl Decider for Shared {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().decide(choice, None)
    }
    fn choose_seeing(&mut self, choice: &Choice, seen: &SeatObservation<'_>) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().decide(choice, Some(seen))
    }
}

/// The entry line of one decision.
fn line(source: &str, game: usize, step: usize, k: usize, state: &str, choice: &Choice, chosen: &str) -> String {
    let mut log = DecisionLog::default();
    log.record(choice, &ChoiceOption::new(chosen, ""));
    let record = &log.records[0];
    let v1 = decision_hash(CanonicalHashVersion::V1, record).digest;
    let v2 = decision_hash(CanonicalHashVersion::V2, record).digest;
    let entry = json!({
        "s": source, "g": game, "step": step, "k": k, "state": state,
        "choice": serde_json::to_value(choice).expect("choice"),
        "chosen": chosen, "v1": &v1[..16], "v2": &v2[..16],
    });
    serde_json::to_string(&entry).expect("line")
}

struct Played {
    lines: Vec<String>,
    game_hash: String,
    steps: usize,
    finished: bool,
}

/// Play one game exactly as `selfplay run` (the rule bot at every seat), recording every decision.
fn play(source: &str, game_no: usize, spec: &GameSpec, max_rounds: u32, max_steps: usize) -> Result<Played, String> {
    let mut game = spec.build()?;
    let captured: Captured = Rc::default();
    game.table.set_default(Box::new(Shared(Rc::new(RefCell::new(Bot {
        seed: spec.seed,
        decided: 0,
        random: SeededRandom::new(spec.seed),
        captured: captured.clone(),
    })))));
    let mut lines = Vec::new();
    let mut steps = 0usize;
    let mut log = 0usize;
    while !game.state.finished && game.state.round <= max_rounds && steps < max_steps {
        let before = game.state.clone();
        let result = game.step();
        if let Some(error) = result.error {
            return Err(format!("{source} game {game_no} seed {}: step {steps}: {error}", spec.seed));
        }
        let asked = std::mem::take(&mut *captured.borrow_mut());
        if !asked.is_empty() {
            let state = sha_hex(serde_json::to_string(&before).expect("state").as_bytes(), 16);
            for (k, (choice, chosen)) in asked.iter().enumerate() {
                lines.push(line(source, game_no, steps, k, &state, choice, chosen));
            }
            log += asked.len();
        }
        steps += 1;
    }
    if log != game.table.log.records.len() {
        return Err(format!(
            "{source} game {game_no}: {log} decisions captured, {} logged",
            game.table.log.records.len()
        ));
    }
    Ok(Played {
        lines,
        game_hash: ti4_wasm::host_session::decisions_hash(&game.table.log.records),
        steps,
        finished: game.state.finished,
    })
}

struct Job {
    game_no: usize,
    spec: GameSpec,
    max_steps: usize,
}

struct Plan {
    jobs: Vec<Job>,
    max_rounds: u32,
    bot: BTreeMap<String, String>,
    note: String,
}

fn config_plan(config: &str, bot_version: Option<&str>, picks: Option<&[usize]>) -> Result<Plan, String> {
    let (matrix, specs) = ti4_engine_core_bridge::workload::specs(config)?;
    let Matrix { limits, mut bot, .. } = matrix;
    if let Some(v) = bot_version {
        bot.insert("bot_version".to_owned(), v.to_owned());
    }
    let jobs = specs
        .into_iter()
        .enumerate()
        .filter(|(i, _)| picks.is_none_or(|p| p.contains(i)))
        .map(|(game_no, spec)| Job { game_no, spec, max_steps: limits.max_steps })
        .collect();
    let note = format!("{config} bot {bot:?}{}", picks.map_or(String::new(), |p| format!(" games {p:?}")));
    Ok(Plan { jobs, max_rounds: limits.max_rounds, bot, note })
}

fn opening_spec(alias: &str, players: usize, thunders_edge: bool, turn: usize) -> GameSpec {
    let pool = ti4_engine::seating::LOBBY_FACTIONS;
    GameSpec {
        setup: "templates".to_owned(),
        seed: 1,
        players,
        map: alias.to_owned(),
        factions: (0..players).map(|s| pool[(turn + s) % pool.len()].to_owned()).collect(),
        speaker: "seat".to_owned(),
        card_set: None,
        rotation: None,
        starting_tech: true,
        thunders_edge,
        rules_version: None,
    }
}

fn plan_for(source: &str) -> Result<Option<Plan>, String> {
    let rb = "tools/rulebot";
    Ok(Some(match source {
        "golden20" => config_plan("tools/engine_profile/profile-20.json", None, None)?,
        "bench" => config_plan(&format!("{rb}/golden.json"), None, Some(&BENCH_PICKS))?,
        "bench-v1" => config_plan(&format!("{rb}/golden-v2.json"), Some("1"), Some(&BENCH_PICKS))?,
        "bench-v2" => config_plan(&format!("{rb}/golden-v2.json"), Some("2"), Some(&BENCH_PICKS))?,
        "bench-v3" => config_plan(&format!("{rb}/golden-v2.json"), Some("3"), Some(&BENCH_PICKS))?,
        "bench-v4" => config_plan(&format!("{rb}/golden-v4.json"), Some("4"), Some(&BENCH_PICKS))?,
        "bench-v5" => config_plan(&format!("{rb}/golden-v5.json"), Some("5"), Some(&BENCH_PICKS))?,
        "templates" => {
            let loader = ti4_host::maps::template_loader::TemplateLoader::load().map_err(|e| format!("{e:?}"))?;
            let mut jobs = Vec::new();
            let mut turn = 0usize;
            for summary in loader.list_summaries() {
                for te in [false, true] {
                    let spec = opening_spec(&summary.alias, summary.player_count, te, turn);
                    turn += summary.player_count;
                    // Templates the host refuses to seat (e.g. a one-player map) have no opening.
                    if spec.build().is_ok() {
                        jobs.push(Job { game_no: jobs.len(), spec, max_steps: OPENING_STEPS });
                    }
                }
            }
            Plan { jobs, max_rounds: 50, bot: BTreeMap::new(), note: format!("every template opening, {OPENING_STEPS} steps") }
        }
        "te" => {
            let pool = ti4_engine::seating::LOBBY_FACTIONS;
            let jobs = (0..pool.len() / 6)
                .map(|i| Job {
                    game_no: i,
                    spec: GameSpec {
                        seed: 7 + i as u64,
                        ..opening_spec("6pStandard", 6, true, i * 6)
                    },
                    max_steps: TE_STEPS,
                })
                .collect();
            Plan { jobs, max_rounds: 50, bot: BTreeMap::new(), note: format!("Thunder's Edge 6p tables, every lobby faction, {TE_STEPS} steps") }
        }
        _ => return Ok(None),
    }))
}

// ---- fixtures ------------------------------------------------------------------------------------------

fn find_choices(v: &Value, out: &mut Vec<Value>) {
    match v {
        Value::Object(m) => {
            let is_choice = m.get("prompt").is_some_and(Value::is_string)
                && m.get("options").and_then(Value::as_array).is_some_and(|o| o.iter().all(|x| x.get("id").is_some()));
            if is_choice {
                out.push(v.clone());
            }
            m.values().for_each(|x| find_choices(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| find_choices(x, out)),
        _ => {}
    }
}

fn fixture_lines() -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for dir in ["fixtures/protocol", "crates/ti4-server/tests/fixtures", "crates/ti4-host/tests/fixtures", "crates/ti4-wasm/tests/it/fixtures"] {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        for e in rd.flatten() {
            if e.path().extension().is_some_and(|x| x == "json") {
                files.push(e.path());
            }
        }
    }
    files.sort();
    let mut lines = Vec::new();
    for (g, path) in files.iter().enumerate() {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let Ok(value) = serde_json::from_str::<Value>(&text) else { continue };
        let mut found = Vec::new();
        find_choices(&value, &mut found);
        for (k, choice_json) in found.into_iter().enumerate() {
            let choice: Choice = serde_json::from_value(choice_json.clone()).map_err(|e| format!("{}: {e}", path.display()))?;
            // A fixture has no state; its Choice is recorded as the engine's types read and write it.
            let chosen = choice.options.first().map_or(String::new(), |o| o.id.clone());
            let name = path.display().to_string();
            let mut l = line("fixtures", g, 0, k, &sha_hex(name.as_bytes(), 16), &choice, &chosen);
            // The file the Choice came from, for the reader.
            l.insert_str(1, &format!("\"file\":{},", serde_json::to_string(&name).expect("name")));
            lines.push(l);
        }
    }
    Ok(lines)
}

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
                    let r = play(source, job.game_no, &job.spec, plan.max_rounds, job.max_steps);
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

/// Parse a recorded line: `(source, game, state, Choice, chosen, v1, v2)`.
fn check_line(l: &str) -> Result<(), String> {
    let v: Value = serde_json::from_str(l).map_err(|e| e.to_string())?;
    let choice_json = v.get("choice").ok_or("no choice")?;
    let choice: Choice = serde_json::from_value(choice_json.clone()).map_err(|e| e.to_string())?;
    if serde_json::to_value(&choice).map_err(|e| e.to_string())? != *choice_json {
        return Err("Choice JSON does not round-trip through ti4_engine::choice::Choice".to_owned());
    }
    let chosen = v["chosen"].as_str().ok_or("no chosen")?;
    if choice.free_form.is_none() && !choice.options.iter().any(|o| o.id == chosen) && v["s"] != "fixtures" {
        return Err(format!("chosen {chosen} not among the offered"));
    }
    let mut log = DecisionLog::default();
    log.record(&choice, &ChoiceOption::new(chosen, ""));
    for (key, version) in [("v1", CanonicalHashVersion::V1), ("v2", CanonicalHashVersion::V2)] {
        let h = decision_hash(version, &log.records[0]).digest;
        if v[key].as_str() != Some(&h[..16]) {
            return Err(format!("{key} differs: recorded {}, computed {}", v[key], &h[..16]));
        }
    }
    Ok(())
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
