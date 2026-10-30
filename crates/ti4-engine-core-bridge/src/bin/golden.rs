//! `golden`: frozen per-`RULES_VERSION` replays of recorded games (docs/todo/D4-core-tests.md 4; tools/engine_core/golden/README.md).
//!
//! ```text
//! golden list
//! golden check  [SET...] [--games SUBSTR,..] [--mask SPEC] [--threads N] [--smoke] [--static]
//! golden diff   SET [--games SUBSTR,..] [--mask SPEC] [--threads N] [--full]
//! golden bless  SET --reason TEXT --expect SUBTYPE[,..] [--issue ID] [--bugfix] [--games SUBSTR,..]
//! golden record --set NAME --config FILE [--games A..B|i,j] [--rules-version N] [--tier full|slim] [--bot-version V]
//!               [--require-dual-clean MASK] [--shard I/N] [--workers N] [--note TEXT] [--new]
//! golden bless-new --set NAME --config FILE ...      record + freeze every older set + log (the set must be new, at RULES_VERSION)
//! golden ladder [--require-dual-clean MASK] [--workers N]
//! golden finalize NAME [--source S] [--note T]        build set.json from the game files (after sharded records)
//! golden import-corpus [--from DIR] [SOURCE...]       split the X2 decision corpus per game, lines byte-identical
//! golden index                                        rewrite sets.json, freeze the sets below RULES_VERSION
//! golden props                                        (properties over golden positions are D4-3's)
//! ```
//! `--dir DIR` (default `tools/engine_core/golden`) is the golden directory; run from the repo root. `--mask` is a
//! `TI4_AREAS` spec (`all=core`, `all=string`); without it the process default (`TI4_AREAS`) applies.

use std::path::{Path, PathBuf};

use ti4_engine::dual::{AreaMask, Backend};
use ti4_engine_core_bridge::golden::bless::{self, BlessArgs, GitEnv};
use ti4_engine_core_bridge::golden::corpus;
use ti4_engine_core_bridge::golden::format::{GameFile, Limits, Tier};
use ti4_engine_core_bridge::golden::record::{self, ImportInput, RecordJob, RecordOpts};
use ti4_engine_core_bridge::golden::replay::{self, Divergence, EngineReplayer, Replayer};
use ti4_engine_core_bridge::golden::sets::{self, SetMeta};
use ti4_model::state::RULES_VERSION;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

struct Args {
    pos: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

const SWITCHES: [&str; 6] = ["--smoke", "--static", "--full", "--bugfix", "--new", "--help"];

impl Args {
    fn parse() -> Self {
        let mut pos = Vec::new();
        let mut flags = Vec::new();
        let mut it = std::env::args().skip(1).peekable();
        while let Some(a) = it.next() {
            if a.starts_with("--") {
                if SWITCHES.contains(&a.as_str()) {
                    flags.push((a, None));
                } else {
                    let v = it.next().unwrap_or_else(|| fail(&format!("{a} needs a value")));
                    flags.push((a, Some(v)));
                }
            } else {
                pos.push(a);
            }
        }
        Self { pos, flags }
    }
    fn get(&self, name: &str) -> Option<&str> {
        self.flags.iter().rev().find(|(n, _)| n == name).and_then(|(_, v)| v.as_deref())
    }
    fn has(&self, name: &str) -> bool {
        self.flags.iter().any(|(n, _)| n == name)
    }
    fn list(&self, name: &str) -> Vec<String> {
        self.get(name).map(|v| v.split(',').map(str::to_owned).filter(|s| !s.is_empty()).collect()).unwrap_or_default()
    }
    fn num(&self, name: &str, default: usize) -> usize {
        self.get(name).map_or(default, |v| v.parse().unwrap_or_else(|_| fail(&format!("{name}: not a number"))))
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("golden: {msg}");
    std::process::exit(2)
}

fn mask_of(spec: Option<&str>) -> Option<AreaMask> {
    spec.map(|s| AreaMask::parse(s, AreaMask::STRING).unwrap_or_else(|e| fail(&format!("--mask: {e}"))))
}

fn main() {
    let args = Args::parse();
    let Some(cmd) = args.pos.first().cloned() else { usage() };
    let root = PathBuf::from(args.get("--dir").unwrap_or("tools/engine_core/golden"));
    let result = match cmd.as_str() {
        "list" => cmd_list(&root),
        "index" => reindex(&root).map(|n| println!("sets.json: {n} sets")),
        "check" => cmd_check(&root, &args),
        "diff" => cmd_diff(&root, &args),
        "bless" => cmd_bless(&root, &args),
        "record" => cmd_record(&root, &args, false),
        "bless-new" => cmd_record(&root, &args, true),
        "ladder" => cmd_ladder(&root, &args),
        "finalize" => cmd_finalize(&root, &args),
        "import-corpus" => cmd_import(&root, &args),
        "props" => {
            println!("golden props: the properties over golden positions (P7-P10) are package D4-3 (tests/wrapper_props.rs of ti4-engine-core-tests); nothing to run here.");
            Ok(())
        }
        _ => usage(),
    };
    if let Err(e) = result {
        eprintln!("golden {cmd}: {e}");
        std::process::exit(1);
    }
}

fn usage() -> ! {
    eprintln!("usage: golden list|index|check|diff|bless|record|bless-new|ladder|finalize|import-corpus|props (see the header of crates/ti4-engine-core-bridge/src/bin/golden.rs)");
    std::process::exit(2)
}

fn pure_mean(dir: &Path, meta: &SetMeta) -> Option<u32> {
    let mut vals = Vec::new();
    for g in &meta.games {
        if let Ok((f, _)) = GameFile::read(&dir.join(&g.file))
            && let Some(p) = f.header.pure_milli
        {
            vals.push(p);
        }
    }
    (!vals.is_empty()).then(|| vals.iter().sum::<u32>() / vals.len() as u32)
}

fn reindex(root: &Path) -> Result<usize, String> {
    sets::reindex(root, RULES_VERSION, &pure_mean).map(|v| v.len())
}

fn cmd_list(root: &Path) -> Result<(), String> {
    println!("RULES_VERSION {RULES_VERSION}; golden dir {}", root.display());
    for e in sets::read_index(root)? {
        println!(
            "{:<22} {:<5} rules {:<5} {:<7} {:>4} games {:>7} decisions  pure {}  {}  ({})",
            e.name,
            format!("{:?}", e.tier).to_lowercase(),
            e.rules_version.map_or("-".to_owned(), |v| v.to_string()),
            if e.frozen { "frozen" } else { "open" },
            e.games,
            e.decisions,
            e.pure_share_at_record.map_or("n/a".to_owned(), |p| format!("{}.{}%", p / 10, p % 10)),
            e.dir,
            e.source
        );
    }
    Ok(())
}

fn replayer_of(a: &Args) -> EngineReplayer {
    let mask = mask_of(a.get("--mask"));
    if let Some(m) = mask {
        let on: Vec<&str> = ti4_engine::dual::Area::ALL.into_iter().filter(|x| m.backend(*x) != Backend::String).map(ti4_engine::dual::Area::name).collect();
        eprintln!("mask: {} area(s) off the string path ({})", on.len(), a.get("--mask").unwrap_or(""));
    }
    EngineReplayer { mask, require_clean: None }
}

struct StaticOnly;
impl Replayer for StaticOnly {
    fn check(&self, g: &GameFile) -> Result<Option<Divergence>, String> {
        Ok(replay::static_check(g))
    }
    fn record(&self, _: &replay::RecordReq) -> Result<replay::Recording, String> {
        Err("--static does not record".to_owned())
    }
}

fn cmd_check(root: &Path, a: &Args) -> Result<(), String> {
    let wanted: Vec<String> = a.pos[1..].to_vec();
    let index = sets::read_index(root)?;
    let chosen: Vec<_> = index.iter().filter(|e| wanted.is_empty() || wanted.contains(&e.name)).collect();
    if chosen.is_empty() {
        return Err(format!("no set matches {wanted:?} (golden list)"));
    }
    let engine = replayer_of(a);
    let static_only = StaticOnly;
    let replayer: &dyn Replayer = if a.has("--static") { &static_only } else { &engine };
    let only = a.list("--games");
    let threads = a.num("--threads", 4);
    let started = std::time::Instant::now();
    let (mut games, mut failed, mut decisions) = (0usize, 0usize, 0usize);
    for e in chosen {
        let dir = root.join(&e.dir);
        let meta = SetMeta::read(&dir)?;
        if let Some(v) = meta.rules_version
            && v > RULES_VERSION
        {
            return Err(format!("{}: recorded at rules version {v}, the build knows up to {RULES_VERSION}", meta.name));
        }
        if meta.rules_version.is_none() && !a.has("--static") {
            // a static set (the wire fixtures' Choice objects): its lines are checked by the static path in `replay_game`
        }
        println!("{}  {:?}  rules {:?}  {}  {} games  {} decisions", meta.name, meta.tier, meta.rules_version, if meta.is_frozen(RULES_VERSION) { "frozen" } else { "open" }, meta.games.len(), meta.decisions());
        let set_start = std::time::Instant::now();
        let results = replay::check_set(&dir, &meta, replayer, &only, a.has("--smoke"), threads)?;
        let mut set_failed = 0;
        for r in &results {
            games += 1;
            decisions += r.decisions;
            match &r.outcome {
                Ok(None) => println!("  ok   {}  {} decisions  {:.1}s", r.file, r.decisions, r.millis as f64 / 1000.0),
                Ok(Some(d)) => {
                    set_failed += 1;
                    println!("  FAIL {}: {d}", r.file);
                }
                Err(e) => {
                    set_failed += 1;
                    println!("  FAIL {}: {e}", r.file);
                }
            }
        }
        failed += set_failed;
        println!(
            "  set {}: {} games, {set_failed} failed, pure share at record {} ({:.1}s)",
            meta.name,
            results.len(),
            e.pure_share_at_record.map_or("n/a (measured post-D2)".to_owned(), |p| format!("{}.{}%", p / 10, p % 10)),
            set_start.elapsed().as_secs_f64()
        );
    }
    println!("golden check: {games} games, {decisions} decisions, {failed} failed ({:.1}s)", started.elapsed().as_secs_f64());
    if failed > 0 { Err(format!("{failed} game(s) do not reproduce")) } else { Ok(()) }
}

fn cmd_diff(root: &Path, a: &Args) -> Result<(), String> {
    let set = a.pos.get(1).ok_or("golden diff SET")?;
    let engine = replayer_of(a);
    let (dir, _, diffs) = bless::diff_set(root, set, &engine, &a.list("--games"), a.num("--threads", 4))?;
    if diffs.is_empty() {
        println!("{set}: every game reproduces");
        return Ok(());
    }
    println!("{:<14} {:>6} {:<20} {:<28} {:>9}", "game", "step", "kind", "area (subtype)", "affected");
    for (file, d) in &diffs {
        let (g, _) = GameFile::read(&dir.join(file))?;
        let affected = g.decs.iter().filter(|x| x.step >= d.step).count();
        println!("{file:<14} {:>6} {:<20} {:<28} {affected:>9}", d.step, d.kind.name(), d.subtype);
        println!("    {d}");
        if a.has("--full") {
            let req = replay::RecordReq { spec: g.header.spec.clone().ok_or("no spec")?, bot: g.header.bot.clone(), limits: g.header.limits.clone(), tier: g.tier, source: g.header.source.clone(), game: g.header.game };
            if let replay::Recording::Game(new) = engine.record(&req)? {
                println!("    changed decisions by subtype after a bot-driven re-record: {:?}", bless::changed_decisions(&g, &new));
            }
        }
    }
    Err(format!("{} game(s) differ", diffs.len()))
}

fn cmd_bless(root: &Path, a: &Args) -> Result<(), String> {
    let set = a.pos.get(1).ok_or("golden bless SET --reason ... --expect ...")?;
    let args = BlessArgs {
        reason: a.get("--reason").unwrap_or("").to_owned(),
        expect: a.list("--expect"),
        issue: a.get("--issue").map(str::to_owned),
        bugfix: a.has("--bugfix"),
        only: a.list("--games"),
    };
    let env = GitEnv { golden_dir: root.canonicalize().map_err(|e| e.to_string())? };
    // Re-recording uses the process default mask (the build's), like the game it protects.
    let engine = EngineReplayer::default();
    let r = bless::bless(root, set, &args, RULES_VERSION, &engine, &env, a.num("--threads", 4))?;
    if r.changed_files.is_empty() {
        println!("{set}: nothing differs, nothing blessed");
        return Ok(());
    }
    reindex(root)?;
    print!("blessed {set}: {} game(s)\n{}", r.changed_files.len(), r.block);
    Ok(())
}

fn parse_games(spec: Option<&str>, total: usize) -> Vec<usize> {
    match spec {
        None => (0..total).collect(),
        Some(s) if s.contains("..") => {
            let (a, b) = s.split_once("..").expect("..");
            let a: usize = a.parse().unwrap_or(0);
            let b: usize = if b.is_empty() { total } else { b.parse().unwrap_or_else(|_| fail("--games A..B")) };
            (a..b.min(total)).collect()
        }
        Some(s) => s.split(',').filter_map(|x| x.parse().ok()).filter(|i| *i < total).collect(),
    }
}

fn require_clean(a: &Args) -> Option<AreaMask> {
    a.get("--require-dual-clean").map(|m| {
        // `all` is the short name of the whole-engine dual mask
        let spec = if m == "all" { "all=dual" } else { m };
        AreaMask::parse(spec, AreaMask::STRING).unwrap_or_else(|e| fail(&format!("--require-dual-clean: {e}")))
    })
}

fn cmd_record(root: &Path, a: &Args, new: bool) -> Result<(), String> {
    let name = a.get("--set").ok_or("--set NAME")?;
    let config = a.get("--config").ok_or("--config FILE")?;
    let tier = Tier::parse(a.get("--tier").unwrap_or("slim"))?;
    let (matrix, specs) = ti4_engine_core_bridge::workload::specs(config)?;
    let mut bot = matrix.bot.clone();
    if let Some(v) = a.get("--bot-version") {
        bot.insert("bot_version".to_owned(), v.to_owned());
    }
    let pin = a.get("--rules-version").map(|v| v.parse::<u32>().unwrap_or_else(|_| fail("--rules-version N")));
    let shard = a.get("--shard").map(|s| {
        let (i, n) = s.split_once('/').unwrap_or_else(|| fail("--shard I/N"));
        (i.parse::<usize>().unwrap_or_else(|_| fail("--shard I/N")), n.parse::<usize>().unwrap_or_else(|_| fail("--shard I/N")))
    });
    let dir = root.join(name);
    if new || a.has("--new") {
        if dir.join("set.json").exists() {
            return Err(format!("{name} exists: a NEW set must be new (use `golden bless` to change an open set)"));
        }
        if pin.is_some_and(|v| v != RULES_VERSION) {
            return Err(format!("bless-new records at RULES_VERSION {RULES_VERSION}, not {pin:?}"));
        }
    }
    let jobs: Vec<RecordJob> = parse_games(a.get("--games"), specs.len())
        .into_iter()
        .map(|i| {
            let mut spec = specs[i].clone();
            if pin.is_some() {
                spec.rules_version = pin;
            }
            RecordJob { index: i, spec, source: Some(name.to_owned()), game: Some(i) }
        })
        .collect();
    let opts = RecordOpts {
        dir: dir.clone(),
        tier,
        bot,
        limits: Limits { max_rounds: matrix.limits.max_rounds, max_steps: matrix.limits.max_steps },
        workers: a.num("--workers", 2),
        shard,
    };
    let engine = EngineReplayer { mask: None, require_clean: require_clean(a) };
    let summary = record::record_games(&opts, &jobs, &engine)?;
    println!("recorded {} game(s), excluded {} (dual differences, T13: not frozen)", summary.recorded.len(), summary.excluded.len());
    for (i, why) in &summary.excluded {
        println!("  excluded {}: {why}", record::stem(*i));
    }
    if shard.is_some() {
        println!("sharded record: run `golden finalize {name}` once all shards are done");
        return Ok(());
    }
    finish_set(root, &dir, name, config, a.get("--note").unwrap_or(""), new || a.has("--new"))
}

fn finish_set(root: &Path, dir: &Path, name: &str, source: &str, note: &str, log: bool) -> Result<(), String> {
    let meta = record::finalize_set(dir, name, source, note)?;
    reindex(root)?;
    println!("set {name}: {} games, {} decisions, rules version {:?}, tier {:?}", meta.games.len(), meta.decisions(), meta.rules_version, meta.tier);
    if log {
        let env = GitEnv { golden_dir: root.canonicalize().map_err(|e| e.to_string())? };
        bless::log_new_set(root, &env, name, meta.tier, meta.rules_version, meta.games.len(), "new set")?;
    }
    Ok(())
}

fn cmd_finalize(root: &Path, a: &Args) -> Result<(), String> {
    let name = a.pos.get(1).ok_or("golden finalize NAME")?;
    finish_set(root, &root.join(name), name, a.get("--source").unwrap_or("record"), a.get("--note").unwrap_or(""), false)
}

fn cmd_ladder(root: &Path, a: &Args) -> Result<(), String> {
    let dir = root.join("ladder");
    let opts = RecordOpts {
        dir: dir.clone(),
        tier: Tier::Slim,
        bot: Default::default(),
        limits: Limits { max_rounds: 50, max_steps: record::LADDER_STEPS },
        workers: a.num("--workers", 2),
        shard: None,
    };
    let engine = EngineReplayer { mask: None, require_clean: require_clean(a) };
    let summary = record::record_games(&opts, &record::ladder_jobs(RULES_VERSION), &engine)?;
    println!("ladder: recorded {} game(s), excluded {}", summary.recorded.len(), summary.excluded.len());
    for (i, why) in &summary.excluded {
        println!("  excluded {}: {why}", record::stem(*i));
    }
    // the ladder is a set of games at DIFFERENT rules versions: one set.json per version would be pointless, so the
    // set carries rules_version = the newest and each game header its own (checked by every_version_has_a_set)
    finish_ladder(root, &dir)
}

fn finish_ladder(root: &Path, dir: &Path) -> Result<(), String> {
    // finalize_set demands one version per set; the ladder is the one exception and is assembled here
    let mut files: Vec<_> = std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().ends_with(Tier::Slim.ext())).collect();
    files.sort();
    let mut games = Vec::new();
    for p in &files {
        let (g, sha) = GameFile::read(p)?;
        games.push(sets::GameEntry {
            file: p.file_name().expect("name").to_string_lossy().into_owned(),
            steps: g.header.steps,
            decisions: g.decs.len(),
            decisions_hash: g.header.decisions_hash.clone(),
            bytes: std::fs::metadata(p).map_err(|e| e.to_string())?.len(),
            sha256: sha,
        });
    }
    let smoke = SetMeta::pick_smoke(&games, 4);
    SetMeta {
        name: "ladder".to_owned(),
        tier: Tier::Slim,
        rules_version: None,
        frozen: false,
        source: "golden ladder".to_owned(),
        note: "one short game per rules version 0..=RULES_VERSION; each game header pins its own version".to_owned(),
        games,
        smoke,
    }
    .write(dir)?;
    println!("ladder: {} game(s)", files.len());
    reindex(root)?;
    Ok(())
}

fn cmd_import(root: &Path, a: &Args) -> Result<(), String> {
    let from = PathBuf::from(a.get("--from").unwrap_or("tools/engine_core/decision_corpus"));
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(from.join("manifest.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let wanted: Vec<String> = if a.pos.len() > 1 { a.pos[1..].to_vec() } else { corpus::SOURCES.iter().map(|s| (*s).to_owned()).collect() };
    for source in wanted {
        let entry = manifest["sources"][source.as_str()].clone();
        if entry.is_null() {
            return Err(format!("{source}: not in the manifest"));
        }
        let path = from.join(format!("{source}.jsonl.zst"));
        let f = std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let dec = zstd::Decoder::new(f).map_err(|e| e.to_string())?;
        let lines: Vec<String> = std::io::BufRead::lines(std::io::BufReader::new(dec)).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        let input = ImportInput { plan: corpus::plan_for(&source)?, source: source.clone(), manifest: entry, lines };
        for (name, rel, games) in record::import_games(&input)? {
            let dir = root.join(&rel);
            let _ = std::fs::remove_dir_all(&dir);
            bless::save_games(&dir, &games)?;
            let meta = record::finalize_set(&dir, &name, &format!("x2:{source}"), "imported from tools/engine_core/decision_corpus, lines byte-identical (rules version = the effective one at import for specs that did not pin it)")?;
            println!("{name}: {} games, {} decisions -> {rel}", meta.games.len(), meta.decisions());
        }
    }
    reindex(root)?;
    Ok(())
}
