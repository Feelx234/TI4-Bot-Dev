//! The oracle (package D2-6b(a), docs/todo/D2-remaining.md 3.2): the golden games played by two engines, compared after every step.
//! Subject B (the core mask) leads, answered by the rule bot; subject A (the legacy behaviour, `AreaMask::STRING`) is fed B's
//! chosen option ids. See `ti4_engine_core_bridge::oracle`.
//!
//! ```text
//! oracle CONFIG GAMES [--core MASK] [--allow FILE]                          the first GAMES games of CONFIG, one process
//! oracle CONFIG GAMES [--core MASK] [--allow FILE] --shard i/N --out FILE   only the games with index % N == i;
//!                                                                           writes FILE (shard file) and FILE.hits (allow-list hits)
//! oracle CONFIG GAMES [--core MASK] [--allow FILE] --game I | --seed S      one game (a repro; no stale check)
//! oracle --merge [--allow FILE] [--rules-version N] FILE...                 merge the shard files of one run (needs FILE.hits)
//! oracle CONFIG GAMES --native ARMS [--gaps-lent] [--closing] [--keep-going] [...]                                   the NATIVE oracle: a legacy game leads, a second legacy game
//!                                                                           follows and shadow-runs the named native arms (`tokens`, `all`):
//!                                                                           questions, events and positions of the live arm against the
//!                                                                           legacy step (`ti4_engine_core_bridge::native_subject`)
//! ```
//! `--core MASK` is the area spec of the leading subject (as `TI4_AREAS`, default `all=core`). `--allow` defaults to
//! `tools/engine_core/allow_list.json`. The report is the `DualReport` text of `dual_run` plus the allow-list summary and the number
//! of skipped scenarios; exit 1 on an unallowed difference, a changed decision hash or (full run: not a shard, `--game`, `--seed`) a
//! stale allow-list entry; exit 2 on bad arguments or files that do not make one complete run.

use ti4_engine_legacy::dual::{AreaMask, Backend};
use ti4_engine_core_bridge::allow_list::{AllowList, DEFAULT_PATH, Tally};
use ti4_engine_core_bridge::dual_shard::{self, GameRecord, Shard, ShardFile};
use ti4_engine_core_bridge::native_subject::{self, NativeSubject};
use ti4_engine_core_bridge::oracle::{self, CoreMask, Judge, LegacyMask, Limits, MaskSubject};
use ti4_engine_core_bridge::workload::{golden_game_count, install_bot_tuning, specs};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn usage() -> ! {
    eprintln!("usage: oracle CONFIG GAMES [--core MASK] [--allow FILE] [--shard i/N --out FILE | --game I | --seed S] | oracle --merge [--allow FILE] [--rules-version N] FILE...");
    std::process::exit(2);
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}

fn take_flag(args: &mut Vec<String>, name: &str) -> Option<String> {
    let at = args.iter().position(|a| a == name)?;
    if at + 1 >= args.len() {
        usage();
    }
    let value = args.remove(at + 1);
    args.remove(at);
    Some(value)
}

fn main() {
    ti4_engine_legacy::core_sync::unmapped::set_strict_for_gate();
    ti4_engine_legacy::core_sync::unsupported::set_disabled_policy_for_gate();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let allow_path = take_flag(&mut args, "--allow").unwrap_or_else(|| DEFAULT_PATH.to_owned());
    let list = AllowList::load(&allow_path).unwrap_or_else(|e| fail(&e));
    if args.first().is_some_and(|a| a == "--merge") {
        let rules_version = take_flag(&mut args, "--rules-version").map_or(ti4_model::state::RULES_VERSION, |v| v.parse().unwrap_or_else(|_| usage()));
        merge(&args[1..], &list, rules_version);
    }
    let core = take_flag(&mut args, "--core");
    let native = take_flag(&mut args, "--native").map(|text| ti4_engine_legacy::live::arms::parse(&text).unwrap_or_else(|e| fail(&e)));
    if let Some(arms) = native {
        native_subject::set_native_arms(arms);
    }
    let keep_going = if let Some(at) = args.iter().position(|a| a == "--keep-going") {
        args.remove(at);
        native_subject::set_keep_going(true);
        true
    } else {
        false
    };
    if let Some(at) = args.iter().position(|a| a == "--closing") {
        args.remove(at);
        native_subject::set_shadow_closing(true);
    }
    if let Some(at) = args.iter().position(|a| a == "--gaps-lent") {
        args.remove(at);
        native_subject::set_gaps_are_lent(true);
    }
    let shard = take_flag(&mut args, "--shard").map(|t| Shard::parse(&t).unwrap_or_else(|e| fail(&e)));
    let out = take_flag(&mut args, "--out");
    let game = take_flag(&mut args, "--game").map(|g| g.parse::<usize>().unwrap_or_else(|_| fail("--game wants a game index")));
    let seed = take_flag(&mut args, "--seed").map(|g| g.parse::<u64>().unwrap_or_else(|_| fail("--seed wants a seed number")));
    if [game.is_some(), seed.is_some(), shard.is_some()].into_iter().filter(|b| *b).count() > 1 {
        fail("--game, --seed and --shard do not go together");
    }
    if shard.is_some() != out.is_some() {
        fail("--shard and --out go together");
    }
    let (Some(config), Some(games)) = (args.first(), args.get(1).and_then(|g| g.parse::<usize>().ok())) else { usage() };
    let core_spec = core.unwrap_or_else(|| "all=core".to_owned());
    let mask = AreaMask::parse(&core_spec, AreaMask::STRING).unwrap_or_else(|e| fail(&e));
    oracle::set_core_mask(mask);

    let (matrix, all) = specs(config).unwrap_or_else(|e| fail(&e));
    install_bot_tuning(&matrix).unwrap_or_else(|e| fail(&e));
    let total = golden_game_count(config, games).unwrap_or_else(|e| fail(&e));
    let mut only = game;
    if let Some(seed) = seed {
        let hits: Vec<usize> = all.iter().take(games).enumerate().filter(|(_, g)| g.seed == seed).map(|(i, _)| i).collect();
        match hits.as_slice() {
            [one] => only = Some(*one),
            [] => fail(&format!("--seed {seed}: no game of the first {games} has that seed")),
            many => fail(&format!("--seed {seed}: games {many:?} share it; use --game I")),
        }
    }
    if let Some(o) = only
        && o >= total
    {
        fail(&format!("--game {o}: the run has {total} games (0..{total})"));
    }
    let shard_of = shard.unwrap_or(Shard { index: 0, of: 1 });
    let limits = Limits { max_rounds: matrix.limits.max_rounds, max_steps: matrix.limits.max_steps };
    let mut tally = Tally::new(&list);
    let mut records: Vec<GameRecord> = Vec::new();
    for (index, spec) in all.iter().take(games).enumerate() {
        if !shard_of.includes(index) || only.is_some_and(|o| o != index) {
            continue;
        }
        let mut judge = Judge { list: &list, tally: &mut tally, scenario: format!("seed-{}", spec.seed) };
        let played = if native.is_some() {
            oracle::play_scenario::<NativeSubject, MaskSubject<LegacyMask>>(spec, limits, &mut judge)
        } else {
            oracle::play_scenario::<MaskSubject<LegacyMask>, MaskSubject<CoreMask>>(spec, limits, &mut judge)
        }
        .unwrap_or_else(|e| fail(&format!("game {index} (seed {}): {e}", spec.seed)));
        if let Ok(p) = &played {
            eprintln!("game {index}: {} steps{}", p.steps, p.stopped.as_deref().map(|s| format!(", stopped: {s}")).unwrap_or_default());
        }
        records.push(oracle::record_of(index, played));
    }
    let file = ShardFile {
        config: config.clone(),
        config_hash: dual_shard::file_hash(config).unwrap_or_else(|e| fail(&e)),
        games: total,
        spec: format!("oracle A=string B={core_spec}"),
        areas_on_dual: core_areas(mask),
        shard: shard_of,
        records,
    };
    if let Some(out) = &out {
        file.save(out).unwrap_or_else(|e| fail(&e));
        std::fs::write(format!("{out}.hits"), tally.to_json()).unwrap_or_else(|e| fail(&format!("{out}.hits: {e}")));
        println!("shard {}/{}: wrote {out}", shard_of.index, shard_of.of);
    }
    // A shard or a repro of one game is a partial run: no stale check, no merge.
    if native.is_some() {
        // What the native oracle ran and what it left to the legacy engine: a clean run that ran nothing proves nothing.
        println!("native arms ran in shadow: {:?}", native_subject::ran_counts());
        println!("steps left to the legacy engine (lent): {:?}", native_subject::lent_counts());
        if keep_going {
            let kept = native_subject::kept_differences();
            println!("---- native differences kept (--keep-going): {} kinds ----", kept.len());
            for (kind, (n, first)) in kept {
                println!("{n:>7}  {kind}  e.g. {first}");
            }
        }
    }
    if shard.is_none() && only.is_none() {
        finish_full(&[file], &list, &tally, ti4_model::state::RULES_VERSION);
    }
    print_partial(&file, &list, &tally);
}

/// The areas the leading subject runs on `Core`.
fn core_areas(mask: AreaMask) -> Vec<String> {
    ti4_engine_legacy::dual::Area::ALL.into_iter().filter(|a| mask.backend(*a) == Backend::Core).map(|a| a.name().to_owned()).collect()
}

/// A shard or one game: what it played, its unallowed differences, the skips; exit 1 on a difference or a changed hash.
fn print_partial(file: &ShardFile, list: &AllowList, tally: &Tally) -> ! {
    let mut report = ti4_engine_legacy::dual::DualReport::default();
    let mut ok = true;
    for g in &file.records {
        report.add(&g.report);
        ok &= g.report.total_differences() == 0 && g.dual_hash == g.string_hash;
    }
    let skipped = file.records.iter().filter(|g| g.skipped.is_some()).count();
    println!("oracle: {} of {} games played here, {skipped} skipped", file.records.len() - skipped, file.games);
    print!("{report}");
    print!("{}", tally.render(list));
    exit_with(oracle::conclude(ok, list, tally, ti4_model::state::RULES_VERSION, false))
}

fn exit_with(verdict: Result<(), String>) -> ! {
    match verdict {
        Ok(()) => std::process::exit(0),
        Err(problems) => {
            eprint!("{problems}");
            std::process::exit(1);
        }
    }
}

/// Merge the files of a whole run, print what a one-process run prints, and exit with the verdict (stale check included).
fn finish_full(files: &[ShardFile], list: &AllowList, tally: &Tally, rules_version: u32) -> ! {
    match dual_shard::merge(files) {
        Ok(merged) => {
            print!("{}", merged.render());
            print!("{}", tally.render(list));
            exit_with(oracle::conclude(merged.ok(), list, tally, rules_version, true))
        }
        Err(e) => fail(&format!("MERGE REFUSED: {e}")),
    }
}

fn merge(paths: &[String], list: &AllowList, rules_version: u32) -> ! {
    if paths.is_empty() {
        usage();
    }
    let files: Vec<ShardFile> = paths.iter().map(|p| ShardFile::load(p).unwrap_or_else(|e| fail(&format!("MERGE REFUSED: {e}")))).collect();
    let mut tally = Tally::new(list);
    for p in paths {
        let hits = format!("{p}.hits");
        let text = std::fs::read_to_string(&hits).unwrap_or_else(|e| fail(&format!("MERGE REFUSED: {hits}: {e}")));
        tally.merge(&Tally::from_json(&text).unwrap_or_else(|e| fail(&format!("MERGE REFUSED: {hits}: {e}"))));
    }
    finish_full(&files, list, &tally, rules_version)
}
