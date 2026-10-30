//! The shadow harness of the typed rule bot (docs/todo/D3-bots.md D3-0; library part `shadow`).
//!
//! ```text
//! bot_shadow --config FILE [--games 0,5,11 | --games 0..5] [--shards N --shard K] [--bot key=value]...
//!            [--max-steps N] [--mode self-check|source] [--mutant none|pick|smart|rng]
//!            [--min-coverage subtype=0.99]... [--json FILE] [--strict]
//! bot_shadow --spec JSON  [same flags]       one game given as a `GameSpec` (the `spec` of a mismatch report)
//! ```
//! Plays the games as `selfplay run` does, answers every decision with the string bot and compares the typed bot's
//! `(picked, smart, rng position, next word)` at every call that has a position. Prints the coverage by subtype.
//! Exit 1 on a mismatch (with `--strict`; a mismatch is always printed), exit 2 when a `--min-coverage` is missed
//! or the arguments or the run fail.

use std::collections::BTreeMap;

use ti4_engine_core_bridge::shadow::{self, Config, Mode, Mutant, NoTyped};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn fail(message: &str) -> ! {
    eprintln!("bot_shadow: {message}");
    std::process::exit(2);
}

fn games(text: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for part in text.split(',').filter(|p| !p.is_empty()) {
        if let Some((a, b)) = part.split_once("..") {
            let (a, b): (usize, usize) = (
                a.parse().unwrap_or_else(|_| fail("--games: bad range")),
                b.parse().unwrap_or_else(|_| fail("--games: bad range")),
            );
            out.extend(a..b);
        } else {
            out.push(part.parse().unwrap_or_else(|_| fail("--games: bad number")));
        }
    }
    out
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut config: Option<String> = None;
    let mut spec: Option<String> = None;
    let mut only: Option<Vec<usize>> = None;
    let (mut shards, mut shard) = (1usize, 0usize);
    let mut cfg = Config::default();
    let mut min_coverage: Vec<(String, f64)> = Vec::new();
    let mut json: Option<String> = None;
    let mut strict = false;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().unwrap_or_else(|| fail(&format!("{name} needs a value")));
        match arg.as_str() {
            "--config" => config = Some(value("--config")),
            "--spec" => spec = Some(value("--spec")),
            "--games" => only = Some(games(&value("--games"))),
            "--shards" => shards = value("--shards").parse().unwrap_or_else(|_| fail("--shards: bad number")),
            "--shard" => shard = value("--shard").parse().unwrap_or_else(|_| fail("--shard: bad number")),
            "--bot" => {
                let kv = value("--bot");
                let (k, v) = kv.split_once('=').unwrap_or_else(|| fail("--bot wants key=value"));
                cfg.bot.insert(k.to_owned(), v.to_owned());
            }
            "--max-steps" => cfg.max_steps = Some(value("--max-steps").parse().unwrap_or_else(|_| fail("--max-steps: bad number"))),
            "--mode" => {
                cfg.mode = match value("--mode").as_str() {
                    "self-check" => Mode::SelfCheck,
                    "source" => Mode::Source,
                    other => fail(&format!("unknown mode '{other}'")),
                }
            }
            "--mutant" => cfg.mutant = Mutant::parse(&value("--mutant")).unwrap_or_else(|e| fail(&e)),
            "--min-coverage" => {
                let kv = value("--min-coverage");
                let (k, v) = kv.split_once('=').unwrap_or_else(|| fail("--min-coverage wants subtype=ratio"));
                min_coverage.push((k.to_owned(), v.parse().unwrap_or_else(|_| fail("--min-coverage: bad ratio"))));
            }
            "--json" => json = Some(value("--json")),
            "--strict" => strict = true,
            other => fail(&format!("unknown argument '{other}'")),
        }
    }
    if shards == 0 || shard >= shards {
        fail("--shard must be below --shards");
    }
    // A single `--spec` game is wrapped into a one-setup config file in the temp dir.
    let path = match (config, spec) {
        (Some(c), None) => c,
        (None, Some(s)) => {
            let path = std::env::temp_dir().join(format!("bot_shadow_spec_{}.json", std::process::id()));
            std::fs::write(&path, shadow::config_of_spec(&s).unwrap_or_else(|e| fail(&e))).unwrap_or_else(|e| fail(&e.to_string()));
            path.to_string_lossy().into_owned()
        }
        _ => fail("give exactly one of --config and --spec"),
    };
    let include = |i: usize| i % shards == shard && only.as_ref().is_none_or(|o| o.contains(&i));
    let report = shadow::run(&path, &cfg, &include, || Box::new(NoTyped)).unwrap_or_else(|e| fail(&e));
    print!("{}", shadow::render(&report, cfg.mode));
    if !report.view_calls.is_empty() {
        print!("{}", shadow::render_view_calls(&report));
    }
    if let Some(file) = json {
        let text = serde_json::to_string_pretty(&shadow::to_json(&report)).unwrap_or_default();
        std::fs::write(&file, text).unwrap_or_else(|e| fail(&format!("{file}: {e}")));
    }
    let mut missed = BTreeMap::new();
    for (subtype, want) in &min_coverage {
        let have = report.coverage(subtype);
        if have < *want {
            missed.insert(subtype.clone(), (have, *want));
        }
    }
    for (subtype, (have, want)) in &missed {
        eprintln!("coverage of {subtype}: {have:.4} < {want}");
    }
    if strict && !report.mismatches.is_empty() {
        std::process::exit(1);
    }
    if !missed.is_empty() {
        std::process::exit(2);
    }
}
