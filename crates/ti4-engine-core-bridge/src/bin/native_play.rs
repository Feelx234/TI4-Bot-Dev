//! The native playthrough (lane M7): whole games played by the CORE ALONE, every gap counted, exit 1 on any gap.
//!
//! The rule of the project: one can play an ENTIRE game in the new engine without any bail on the agreed content. This bin plays lobby games
//! (`card_set te`, `starting_tech`, no Thunder's Edge content, every lobby-seatable faction, 3 to 6 seats, seeded) by calling ONLY the core:
//! `live_reconcile` + `step_arm` + `live_step` per step, answered by a deterministic seeded bot. No string `Game` steps the game (the legacy game is
//! used to create the opening position, and only in `--shadow` / `--survey` as a bridge-only cross-check). A step that hits an unported arm, a source
//! of questions no lane has made native, a bail of a live window or a close the core does not own stops the game with a counted gap; the island
//! coverage matrix says where. A game that finishes with zero gaps prints its final scores and a state fingerprint and exits 0.
//!
//! ```text
//! native_play [--quick] [--factions all|f1,f2,..] [--players 4,6] [--seeds N] [--first-seed S] [--maxsteps M] [--maxrounds R]
//!             [--twice] [--shadow] [--survey] [--top N] [--list]
//! ```
//!
//! * `--quick`: the cheap check of an island lane: sol, 4 players, seed 1 (what the flags below then override).
//! * `--twice`: play every game a second time and compare the fingerprints (determinism). A mismatch is a failure.
//! * `--shadow`: run the legacy game in parallel with the same answers and compare every step (questions, plain events, position); the first
//!   difference is reported and fails the run.
//! * `--survey`: the legacy game leads a WHOLE game (the shipped rule bot answers) and every step is classified by the arm and the source of
//!   questions the native loop would need: the volume of each gap, not only the first one. Needs no native play.
//!
//! Build it with the `gate` profile (`cargo build --profile gate -p ti4-engine-core-bridge --bin native_play`): a whole game needs optimisation.
//! Exit 0 only when every game finished natively with zero gaps, no defect and no difference; exit 1 otherwise; exit 2 on a usage error.

use std::time::Instant;

use ti4_engine::seating::LOBBY_FACTIONS;
use ti4_engine_core_bridge::native_play::{lobby_spec, play_game, shadow_play, survey, End, Matrix, NativeGame};
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
    twice: bool,
    shadow: bool,
    survey: bool,
    top: usize,
    list: bool,
}

fn usage() -> ! {
    eprintln!("usage: native_play [--quick] [--factions all|f1,f2] [--players 3,4,5,6] [--seeds N] [--first-seed S] [--maxsteps M] [--maxrounds R] [--twice] [--shadow] [--survey] [--top N] [--list]");
    std::process::exit(2);
}

fn parse() -> Options {
    let mut o = Options {
        factions: vec!["sol".to_owned()],
        players: vec![4],
        seeds: 1,
        first_seed: 1,
        max_steps: 100_000,
        max_rounds: 50,
        twice: false,
        shadow: false,
        survey: false,
        top: 5,
        list: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| usage());
        match a.as_str() {
            "--quick" => {}
            "--factions" => {
                let v = value();
                o.factions = if v == "all" { LOBBY_FACTIONS.iter().map(|f| (*f).to_owned()).collect() } else { v.split(',').map(str::to_owned).collect() };
            }
            "--players" => o.players = value().split(',').map(|p| p.parse().unwrap_or_else(|_| usage())).collect(),
            "--seeds" => o.seeds = value().parse().unwrap_or_else(|_| usage()),
            "--first-seed" => o.first_seed = value().parse().unwrap_or_else(|_| usage()),
            "--maxsteps" => o.max_steps = value().parse().unwrap_or_else(|_| usage()),
            "--maxrounds" => o.max_rounds = value().parse().unwrap_or_else(|_| usage()),
            "--twice" => o.twice = true,
            "--shadow" => o.shadow = true,
            "--survey" => o.survey = true,
            "--top" => o.top = value().parse().unwrap_or_else(|_| usage()),
            "--list" => o.list = true,
            _ => usage(),
        }
    }
    for f in &o.factions {
        if !LOBBY_FACTIONS.contains(&f.as_str()) {
            eprintln!("{f:?} is not a lobby faction");
            std::process::exit(2);
        }
    }
    if o.players.iter().any(|p| !(3..=6).contains(p)) {
        eprintln!("players must be 3 to 6");
        std::process::exit(2);
    }
    o
}

fn describe(end: &End) -> String {
    match end {
        End::Finished => "finished".to_owned(),
        End::Gap(g) => format!("GAP {}", g.key()),
        End::Defect(d) => format!("DEFECT {}: {}", d.arm, d.what),
        End::StepLimit => "STEP LIMIT".to_owned(),
        End::RoundLimit => "ROUND LIMIT".to_owned(),
    }
}

fn scores(s: &[(String, i16)]) -> String {
    s.iter().map(|(f, vp)| format!("{f} {vp}")).collect::<Vec<_>>().join(", ")
}

fn main() {
    let o = parse();
    let mut specs: Vec<GameSpec> = Vec::new();
    for faction in &o.factions {
        for &players in &o.players {
            for k in 0..o.seeds {
                specs.push(lobby_spec(faction, players, o.first_seed + k));
            }
        }
    }
    if o.list {
        for s in &specs {
            println!("{}", s.command_line());
        }
        return;
    }
    let mode = if o.survey { "survey (legacy leads)" } else if o.shadow { "native + shadow" } else { "native" };
    println!("native_play: {} games | {mode} | card set te | starting_tech | max {} steps, {} rounds", specs.len(), o.max_steps, o.max_rounds);
    let mut matrix = Matrix::default();
    let mut bad = false;
    let mut finished = 0usize;
    let mut survey_total: std::collections::BTreeMap<(String, bool), u64> = std::collections::BTreeMap::new();
    for spec in &specs {
        let started = Instant::now();
        let head = format!("game {} players {} {} seed {}", spec.factions[0], spec.players, spec.map, spec.seed);
        if o.survey {
            match survey(spec, o.max_steps, o.max_rounds) {
                Ok(s) => {
                    println!("{head} | legacy rounds {} steps {} {} | {:.0}s", s.rounds, s.steps, if s.finished { "finished" } else { "NOT finished" }, started.elapsed().as_secs_f64());
                    for (k, n) in s.steps_by_arm {
                        *survey_total.entry(k).or_default() += n;
                    }
                }
                Err(e) => {
                    println!("{head} | could not build: {e}");
                    bad = true;
                }
            }
            continue;
        }
        let result = if o.shadow {
            shadow_play(spec, o.max_steps, o.max_rounds).map(|s| (s.played, Some((s.compared, s.first_difference))))
        } else {
            NativeGame::from_spec(spec).map(|mut g| (play_game(&mut g, o.max_steps, o.max_rounds), None))
        };
        let (p, shadow) = match result {
            Ok(done) => done,
            Err(e) => {
                println!("{head} | could not build: {e}");
                bad = true;
                continue;
            }
        };
        let mut line = format!("{head} | rounds {} steps {} | {} | fingerprint {}", p.rounds, p.steps, describe(&p.end), p.fingerprint);
        if let Some((compared, difference)) = &shadow {
            line.push_str(&format!(" | shadow compared {compared}"));
            if let Some(d) = difference {
                line.push_str(&format!(" | FIRST DIFFERENCE {d}"));
                bad = true;
            }
        }
        line.push_str(&format!(" | {:.0}s", started.elapsed().as_secs_f64()));
        println!("{line}");
        if p.end == End::Finished {
            finished += 1;
            println!("    final scores: {}", scores(&p.scores));
        } else {
            bad = true;
        }
        if o.twice {
            match NativeGame::from_spec(spec).map(|mut g| play_game(&mut g, o.max_steps, o.max_rounds)) {
                Ok(second) if second.fingerprint == p.fingerprint && second.steps == p.steps => println!("    determinism: the second run gives the same fingerprint {}", second.fingerprint),
                Ok(second) => {
                    println!("    DETERMINISM FAILED: first {} ({} steps), second {} ({} steps)", p.fingerprint, p.steps, second.fingerprint, second.steps);
                    bad = true;
                }
                Err(e) => {
                    println!("    DETERMINISM: the second run could not build: {e}");
                    bad = true;
                }
            }
        }
        matrix.merge(&p.matrix);
    }
    if o.survey {
        println!("---- survey: steps of the whole legacy game by the arm the native loop needs (native = has a live function) ----");
        let mut rows: Vec<_> = survey_total.iter().collect();
        rows.sort_by_key(|((_, native), n)| (*native, std::cmp::Reverse(**n)));
        for ((arm, native), n) in rows {
            println!("{n:>9}  {:<7} {arm}", if *native { "native" } else { "GAP" });
        }
        let gap: u64 = survey_total.iter().filter(|((_, n), _)| !*n).map(|(_, n)| *n).sum();
        let all: u64 = survey_total.values().sum();
        println!("native_play survey: {gap} of {all} steps need an arm or a question source the core does not run yet");
        std::process::exit(i32::from(bad || gap > 0));
    }
    println!("---- island coverage matrix (native steps, gaps by cause) ----");
    for line in matrix.render(o.top) {
        println!("{line}");
    }
    println!("native_play: {} games | {} finished with zero gaps | {} gaps | {}", specs.len(), finished, matrix.total_gaps(), if bad { "FAILED (a whole game must play natively without a gap)" } else { "ZERO GAP" });
    std::process::exit(i32::from(bad));
}
