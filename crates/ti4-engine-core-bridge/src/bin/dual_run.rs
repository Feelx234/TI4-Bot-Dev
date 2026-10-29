//! The dual run in a game (docs/ENGINE_CORE_STATE.md, 3.3): golden games and the template openings
//! played with the chosen areas on `Dual`, every answer compared, then the games played again on
//! `String` and their decision hashes compared.
//!
//! ```text
//! dual_run CONFIG GAMES [AREAS]                           AREAS as TI4_AREAS (default: movement=dual)
//! dual_run CONFIG GAMES [AREAS] --shard i/N --out FILE   play only the games with index % N == i, write FILE
//! dual_run --merge FILE...                                merge the N shard files of one run
//! dual_run CONFIG GAMES [AREAS] --game I                  play only game I (a repro of one game; unsharded text)
//! dual_run --specs CONFIG GAMES                           one JSON line per game: index, seed, map, factions, ... and the
//!                                                         `selfplay replay` flags (the soak maps a difference to its game)
//! ```
//! Prints the [`DualReport`](ti4_engine::dual::DualReport); exit 1 on any difference or hash change.
//! The merge prints exactly what the unsharded run prints (after checking that the shards cover every game
//! exactly once, see `dual_shard`); it exits 2 when the files do not make one complete run.

use ti4_engine::dual::{self, Area, AreaMask, Backend, DualReport};
use ti4_engine_core_bridge::dual_shard::{self, GameRecord, Shard, ShardFile};
use ti4_engine_core_bridge::workload::{golden_game_count, golden_games, specs as config_specs};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn usage() -> ! {
    eprintln!("usage: dual_run CONFIG GAMES [AREAS] [--shard i/N --out FILE | --game I] | dual_run --merge FILE... | dual_run --specs CONFIG GAMES");
    std::process::exit(2);
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--merge") {
        merge(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "--specs") {
        specs(&args[1..]);
    }
    let mut take_flag = |name: &str| -> Option<String> {
        let at = args.iter().position(|a| a == name)?;
        if at + 1 >= args.len() {
            usage();
        }
        let value = args.remove(at + 1);
        args.remove(at);
        Some(value)
    };
    let shard = take_flag("--shard").map(|t| Shard::parse(&t).unwrap_or_else(|e| fail(&e)));
    let out = take_flag("--out");
    let only = take_flag("--game").map(|g| g.parse::<usize>().unwrap_or_else(|_| fail("--game wants a game index")));
    if only.is_some() && shard.is_some() {
        fail("--game and --shard do not go together");
    }
    if shard.is_some() != out.is_some() {
        fail("--shard and --out go together");
    }
    let (Some(config), Some(games)) = (args.first(), args.get(1).and_then(|g| g.parse::<usize>().ok())) else {
        usage()
    };
    let spec = args.get(2).map_or("movement=dual", String::as_str);
    let dual_mask = AreaMask::parse(spec, AreaMask::STRING).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let on_dual: Vec<_> = Area::ALL.into_iter().filter(|a| dual_mask.backend(*a) == Backend::Dual).map(Area::name).collect();
    if let (Some(shard), Some(out)) = (shard, out) {
        run_shard(config, games, spec, dual_mask, &on_dual, shard, &out);
    }
    let _ = dual::take_report();
    dual::set_default_mask(dual_mask);
    // Stride beyond any game length: only the games are played, positions are not kept.
    let include = |g: usize| only.is_none_or(|o| o == g);
    if let Some(o) = only {
        let total = golden_game_count(config, games).unwrap_or_else(|e| fail(&e));
        if o >= total {
            fail(&format!("--game {o}: the run has {total} games (0..{total})"));
        }
    }
    let (_, dual_hashes) = golden_games(config, games, usize::MAX, &include, &mut |_, _| {}).expect("dual games");
    let report = dual::take_report();

    dual::set_default_mask(AreaMask::all(Backend::String));
    let (_, string_hashes) = golden_games(config, games, usize::MAX, &include, &mut |_, _| {}).expect("string games");
    let mut bad = report.total_differences() > 0;
    for (i, (d, s)) in dual_hashes.iter().zip(&string_hashes).enumerate() {
        if d != s {
            println!("HASH CHANGED game {i}: dual {d} string {s}");
            bad = true;
        }
    }
    println!("games {} | areas on dual: {on_dual:?} | hashes equal: {}", dual_hashes.len(), dual_hashes == string_hashes);
    print!("{report}");
    std::process::exit(i32::from(bad));
}

/// `--specs CONFIG GAMES`: the games of the run, one JSON object per line.
fn specs(args: &[String]) -> ! {
    let (Some(config), Some(games)) = (args.first(), args.get(1).and_then(|g| g.parse::<usize>().ok())) else {
        usage()
    };
    let (matrix, all) = config_specs(config).unwrap_or_else(|e| fail(&e));
    for (index, spec) in all.iter().take(games).enumerate() {
        let mut line = serde_json::to_value(spec).expect("a game spec is JSON");
        let object = line.as_object_mut().expect("a game spec is an object");
        object.insert("index".to_owned(), index.into());
        object.insert("selfplay_flags".to_owned(), spec.command_line().into());
        object.insert("bot".to_owned(), serde_json::to_value(&matrix.bot).expect("a map is JSON"));
        println!("{line}");
    }
    std::process::exit(0);
}

/// Play the games of one shard and write its file.
fn run_shard(config: &str, games: usize, spec: &str, mask: AreaMask, on_dual: &[&str], shard: Shard, out: &str) -> ! {
    let total = golden_game_count(config, games).unwrap_or_else(|e| fail(&e));
    let config_hash = dual_shard::file_hash(config).unwrap_or_else(|e| fail(&e));
    let include = |game: usize| shard.includes(game);
    let mut records: Vec<GameRecord> = Vec::new();

    let _ = dual::take_report();
    dual::set_default_mask(mask);
    golden_games(config, games, usize::MAX, &include, &mut |index, hash| {
        // One report per game: the merge folds them in game order.
        records.push(GameRecord { index, dual_hash: hash.to_owned(), string_hash: String::new(), report: dual::take_report() });
    })
    .expect("dual games");

    dual::set_default_mask(AreaMask::all(Backend::String));
    let mut next = 0;
    golden_games(config, games, usize::MAX, &include, &mut |index, hash| {
        assert_eq!(records[next].index, index, "string games in the order of the dual games");
        records[next].string_hash = hash.to_owned();
        next += 1;
    })
    .expect("string games");
    let _ = dual::take_report();

    let file = ShardFile {
        config: config.to_owned(),
        config_hash,
        games: total,
        spec: spec.to_owned(),
        areas_on_dual: on_dual.iter().map(|a| (*a).to_owned()).collect(),
        shard,
        records,
    };
    file.save(out).unwrap_or_else(|e| fail(&e));
    // The shard's own view: the same text as an unsharded run of its games.
    let mut report = DualReport::default();
    let mut bad = false;
    for g in &file.records {
        report.add(&g.report);
        if g.dual_hash != g.string_hash {
            println!("HASH CHANGED game {}: dual {} string {}", g.index, g.dual_hash, g.string_hash);
            bad = true;
        }
    }
    bad |= report.total_differences() > 0;
    println!("shard {}/{}: {} of {} games, areas on dual: {on_dual:?}, wrote {out}", shard.index, shard.of, file.records.len(), total);
    print!("{report}");
    std::process::exit(i32::from(bad));
}

/// Merge shard files and print what the unsharded run prints.
fn merge(paths: &[String]) -> ! {
    if paths.is_empty() {
        usage();
    }
    let files: Vec<ShardFile> = paths.iter().map(|p| ShardFile::load(p).unwrap_or_else(|e| fail(&format!("MERGE REFUSED: {e}")))).collect();
    match dual_shard::merge(&files) {
        Ok(merged) => {
            print!("{}", merged.render());
            std::process::exit(i32::from(!merged.ok()));
        }
        Err(e) => fail(&format!("MERGE REFUSED: {e}")),
    }
}
