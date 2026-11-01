//! The zero-bail playthrough (package KF5, `docs/D2_LF_RESUME.md`): whole games, every counted bail a failure.
//!
//! The rule of the project: ONE MUST BE ABLE TO PLAY AN ENTIRE GAME IN THE NEW ENGINE WITHOUT ANY BAIL on the agreed content
//! (everything except Thunder's Edge content other than its strategy card set, and the dropped factions and features). This bin is
//! that sentence as a command: for each lobby faction it plays real games (the shipped rule bot, as the golden games), with
//! `starting_tech: true` and the `te` strategy card set as the server sets them, with every area on `Dual` (or the areas you name),
//! `TI4_BAIL_IS_FAILURE` on, and reports
//!
//! * per game: faction at seat a, players, map, seed, rounds, steps, finished, the decision hash, the bails of THIS game;
//! * per cause (the `zero_bail.py` shape, `<n> bail|gate <label> -- <why>`), biggest first, over all games;
//! * the unmapped positions (`core_sync::unmapped`) and the lent steps (`product::take_lent`);
//! * the bails BY ISLAND (`ti4_engine_core_bridge::islands`): the same causes sorted into the lanes that own them (M1 trade ... F menu),
//!   so a lane reads its own rows and sees at once whether its island is green;
//! * the COVERAGE of the live path (`live::coverage`): how many live steps each native arm ran. A zero bail count that never ran a
//!   native arm proves nothing (trap T-Z1).
//!
//! ```text
//! playthrough [--factions all|f1,f2,..] [--players 4,6] [--seeds N] [--first-seed S] [--maxsteps M] [--maxrounds R]
//!             [--areas SPEC] [--card-set te] [--no-starting-tech] [--native ARMS] [--quick] [--top N] [--list]
//! ```
//!
//! Exit 0 only when no game bailed, none is unfinished (round or step limit), none panicked and no position was unmapped. Exit 1
//! otherwise (the list IS the work list of the lanes still porting), exit 2 on a usage error. `--list` prints the games and exits.
//! A game is `GameSpec { seat a: the faction under test, the others drawn from the lobby factions by the seed }`.
//!
//! # The cheap check of an island lane (`--quick`)
//!
//! `--quick` is the one-game check a lane runs after wiring its island: ONE faction (`--factions`, default `sol`), ONE player count
//! (`--players`, default 4), ONE seed (`--seeds`/`--first-seed`), and a step cap (`--maxsteps`, default [`QUICK_STEPS`]) that is not a
//! failure by itself: a game that reaches the cap without a bail is reported `capped` and the verdict says so, because the check
//! is "does my island bail", not "does the game finish" (the finish is the Z-gate's, which waits for 8 consecutive green seeds). The
//! per-island table at the end is the result to read. Build it with the `gate` profile (`tools/engine_core/playthrough_quick.sh`
//! does, and runs it): a debug build needs more than 17 CPU minutes for 2500 steps under `all=dual`.

use std::panic::AssertUnwindSafe;

use ti4_engine_legacy::dual::{self, AreaMask, DualReport};
use ti4_engine_legacy::seating::LOBBY_FACTIONS;
use ti4_engine_core_bridge::workload::golden_decider;
use ti4_selfplay::spec::GameSpec;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

struct Options {
    factions: Vec<String>,
    players: Vec<usize>,
    seeds: u64,
    first_seed: u64,
    max_steps: usize,
    max_rounds: u32,
    areas: String,
    card_set: String,
    starting_tech: bool,
    native: Option<String>,
    list: bool,
    /// The one-game check of a lane: a step cap is not a failure.
    quick: bool,
    /// Causes printed per island.
    top: usize,
}

/// The step cap of `--quick` when `--maxsteps` is not given.
const QUICK_STEPS: usize = 2_500;

fn usage() -> ! {
    eprintln!(
        "usage: playthrough [--factions all|f1,f2] [--players 4,6] [--seeds N] [--first-seed S] [--maxsteps M] [--maxrounds R] [--areas SPEC] [--card-set te] [--no-starting-tech] [--native ARMS] [--quick] [--top N] [--list]"
    );
    std::process::exit(2);
}

fn parse() -> Options {
    let mut o = Options {
        factions: LOBBY_FACTIONS.iter().map(|f| (*f).to_owned()).collect(),
        players: vec![4],
        seeds: 1,
        first_seed: 1,
        max_steps: 400_000,
        max_rounds: 50,
        areas: "all=dual".to_owned(),
        card_set: "te".to_owned(),
        starting_tech: true,
        native: None,
        list: false,
        quick: false,
        top: 3,
    };
    let (mut factions_given, mut steps_given) = (false, false);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| usage());
        match a.as_str() {
            "--factions" => {
                let v = value();
                factions_given = true;
                if v != "all" {
                    o.factions = v.split(',').map(str::to_owned).collect();
                }
            }
            "--players" => o.players = value().split(',').map(|p| p.parse().unwrap_or_else(|_| usage())).collect(),
            "--seeds" => o.seeds = value().parse().unwrap_or_else(|_| usage()),
            "--first-seed" => o.first_seed = value().parse().unwrap_or_else(|_| usage()),
            "--maxsteps" => {
                o.max_steps = value().parse().unwrap_or_else(|_| usage());
                steps_given = true;
            }
            "--maxrounds" => o.max_rounds = value().parse().unwrap_or_else(|_| usage()),
            "--areas" => o.areas = value(),
            "--card-set" => o.card_set = value(),
            "--no-starting-tech" => o.starting_tech = false,
            "--native" => o.native = Some(value()),
            "--list" => o.list = true,
            "--quick" => o.quick = true,
            "--top" => o.top = value().parse().unwrap_or_else(|_| usage()),
            _ => usage(),
        }
    }
    if o.quick {
        // The cheap check: one faction (sol unless named), the player count and seeds as given (4 players, 1 seed by default).
        if !factions_given {
            o.factions = vec!["sol".to_owned()];
        }
        if !steps_given {
            o.max_steps = QUICK_STEPS;
        }
    }
    for f in &o.factions {
        if !LOBBY_FACTIONS.contains(&f.as_str()) {
            eprintln!("{f:?} is not a lobby faction");
            std::process::exit(2);
        }
    }
    o
}

/// The maps a table of `players` plays (the golden configs' names).
fn maps_for(players: usize) -> &'static [&'static str] {
    match players {
        4 => &["4pOuterRim", "4pHyperlanes"],
        6 => &["6pStandard", "6pOuterRim"],
        _ => &["random"],
    }
}

/// Seat a plays `faction`; the others are the next lobby factions in a rotation fixed by the seed, never the same faction twice.
fn table(faction: &str, players: usize, seed: u64) -> Vec<String> {
    let mut out = vec![faction.to_owned()];
    let mut at = usize::try_from(seed).unwrap_or(0) % LOBBY_FACTIONS.len();
    while out.len() < players {
        at = (at + 1) % LOBBY_FACTIONS.len();
        let candidate = LOBBY_FACTIONS[at];
        if !out.iter().any(|f| f == candidate) {
            out.push(candidate.to_owned());
        }
    }
    out
}

fn spec_of(o: &Options, faction: &str, players: usize, seed: u64) -> GameSpec {
    let maps = maps_for(players);
    GameSpec {
        setup: format!("playthrough-{faction}"),
        seed,
        players,
        map: maps[usize::try_from(seed).unwrap_or(0) % maps.len()].to_owned(),
        factions: table(faction, players, seed),
        speaker: "random".to_owned(),
        card_set: Some(o.card_set.clone()),
        rotation: None,
        starting_tech: o.starting_tech,
        thunders_edge: false,
        rules_version: None,
    }
}

/// How one game ended.
struct Played {
    spec: GameSpec,
    rounds: u32,
    steps: usize,
    finished: bool,
    why_not: Option<String>,
    hash: String,
    bails: u64,
    /// `--quick`: the game stopped at the step cap (not a failure by itself).
    capped: bool,
    seconds: f64,
}

fn play(o: &Options, spec: &GameSpec) -> Played {
    let started = std::time::Instant::now();
    let _ = dual::take_report();
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| -> Result<(u32, usize, bool, Option<String>, String), String> {
        let mut game = spec.build()?;
        game.table.set_default(golden_decider(spec.seed));
        let mut steps = 0usize;
        while !game.state.finished && game.state.round <= o.max_rounds && steps < o.max_steps {
            let result = game.step();
            steps += 1;
            if let Some(error) = result.error {
                return Ok((game.state.round, steps, false, Some(format!("step error: {error}")), String::new()));
            }
        }
        let hash = ti4_wasm::host_session::decisions_hash(&game.table.log.records);
        let why = if game.state.finished {
            None
        } else if steps >= o.max_steps {
            Some(format!("step limit ({steps})"))
        } else {
            Some(format!("round limit ({})", game.state.round))
        };
        Ok((game.state.round, steps, game.state.finished, why, hash))
    }));
    let report = dual::take_report();
    let bails = report.windows.iter().filter(|(k, _)| k.contains("[bail:") || k.contains("[gate:")).map(|(_, w)| w.bailed).sum::<u64>();
    GAME_REPORTS.with(|r| r.borrow_mut().add(&report));
    let (rounds, steps, finished, why_not, hash) = match outcome {
        Ok(Ok(done)) => done,
        Ok(Err(e)) => (0, 0, false, Some(format!("could not build: {e}")), String::new()),
        Err(p) => {
            let text = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default();
            (0, 0, false, Some(format!("PANIC: {text}")), String::new())
        }
    };
    let capped = o.quick && !finished && why_not.as_deref().is_some_and(|w| w.starts_with("step limit"));
    Played { spec: spec.clone(), rounds, steps, finished, why_not, hash, bails, capped, seconds: started.elapsed().as_secs_f64() }
}

thread_local! {
    static GAME_REPORTS: std::cell::RefCell<DualReport> = std::cell::RefCell::new(DualReport::default());
}

fn main() {
    ti4_engine::core_sync::unmapped::set_strict_for_gate();
    ti4_engine::core_sync::unsupported::set_disabled_policy_for_gate();
    let o = parse();
    let mask = AreaMask::parse(&o.areas, AreaMask::STRING).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let mut specs = Vec::new();
    for faction in &o.factions {
        for &players in &o.players {
            for k in 0..o.seeds {
                specs.push(spec_of(&o, faction, players, o.first_seed + k));
            }
        }
    }
    if o.list {
        for s in &specs {
            println!("{}", s.command_line());
        }
        return;
    }
    // A bail is a failure; a difference does not stop the game (the list is the report).
    dual::set_bail_is_failure(true);
    dual::set_fail_fast(false);
    dual::set_default_mask(mask);
    let native = match &o.native {
        Some(text) => ti4_engine::live::arms::parse(text).unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(2)
        }),
        None => ti4_engine::live::arms::from_env(),
    };
    println!("playthrough: {} games | areas {} | card set {} | starting_tech {} | native arms {:?}", specs.len(), o.areas, o.card_set, o.starting_tech, ti4_engine::live::arms::names(native));
    let mut played = Vec::new();
    for spec in &specs {
        let p = play(&o, spec);
        println!(
            "game {} players {} {} seed {} | rounds {} steps {} | {} | bails {} | hash {} | {:.0}s",
            p.spec.factions[0],
            p.spec.players,
            p.spec.map,
            p.spec.seed,
            p.rounds,
            p.steps,
            if p.capped { "capped (quick)" } else { p.why_not.as_deref().unwrap_or("finished") },
            p.bails,
            p.hash,
            p.seconds
        );
        played.push(p);
    }
    let report = GAME_REPORTS.with(|r| std::mem::take(&mut *r.borrow_mut()));
    let lent = ti4_engine::product::take_lent();
    let unfinished = played.iter().filter(|p| !p.finished && !p.capped).count();
    let capped = played.iter().filter(|p| p.capped).count();
    let bailing = played.iter().filter(|p| p.bails > 0).count();
    println!("---- bails by cause (all games) ----");
    let mut causes: Vec<(String, u64)> =
        report.windows.iter().filter(|(k, _)| k.contains("[bail:") || k.contains("[gate:")).map(|(k, w)| (k.clone(), w.bailed)).collect();
    causes.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (cause, n) in &causes {
        println!("{n:9}  {cause}");
    }
    println!("---- bails by island (the lanes' work lists; green = none counted) ----");
    let rows = ti4_engine_core_bridge::islands::by_island(causes.iter().map(|(k, n)| (k.as_str(), *n)));
    for line in ti4_engine_core_bridge::islands::render(&rows, o.top) {
        println!("{line}");
    }
    println!("---- unmapped positions ----");
    for (reason, n) in &report.unmapped {
        println!("{n:9}  {reason}");
    }
    println!("---- lent steps ----");
    for (label, n) in &lent {
        println!("{n:9}  {label}");
    }
    println!("---- coverage of the live path (steps run natively, by arm) ----");
    for (arm, n) in ti4_engine::live::coverage::take() {
        println!("{n:9}  {arm}");
    }
    // Under TI4_BAIL_IS_FAILURE every counted bail is ALSO a difference (`bail::<label>`): those are the tables above. What is listed here
    // are the differences that are not bails (a real disagreement of the two sides).
    let (bail_diffs, others): (Vec<_>, Vec<_>) = report.differences.iter().partition(|d| d.label.starts_with("bail::"));
    println!("---- differences other than bails ({} bail differences are the tables above) ----", bail_diffs.len());
    for d in others {
        println!("{d}");
    }
    let bad = bailing > 0 || unfinished > 0 || !report.unmapped.is_empty() || !lent.is_empty() || report.total_differences() > 0;
    println!(
        "playthrough: {} games | {} unfinished | {} capped | {} with bails | {} causes | {} unmapped | {} lent | {}",
        played.len(),
        unfinished,
        capped,
        bailing,
        causes.len(),
        report.unmapped.values().sum::<u64>(),
        lent.values().sum::<u64>(),
        if bad {
            "FAILED (a whole game must play without a bail)"
        } else if capped > 0 {
            "NO BAIL SO FAR (quick: the step cap stopped the game, the finish is the Z-gate's)"
        } else {
            "ZERO BAIL"
        }
    );
    std::process::exit(i32::from(bad));
}
