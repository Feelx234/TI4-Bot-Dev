//! Perf layers (docs/PERF_WHY.md): the SAME random-pick game timed at three layers, with allocation counts, and micro-timings of the per-step
//! pieces on a mid-game position.
//!
//! ```text
//! perf_why layers <players> <seed> [reps]   kernel (NativeGame loop) | wrapper (Game) | legacy (string Game), time + allocations
//! perf_why micro  <players> <seed> [at]     the per-step pieces on the LIVE core after `at` kernel steps (default 700), each timed in a loop
//! ```

use std::time::Instant;

use ti4_content::ContentStore;
use ti4_engine::core_sync::{Codec, Ctx, Slice, SliceSet, Sources, materialize};
use ti4_engine::native::Game;
use ti4_engine::wire::choice::Table;
use ti4_engine_core_bridge::legacy_adapter::ThinDecider;
use ti4_engine_core_bridge::native_play::{NativeGame, Step, lobby_spec};
use ti4_selfplay::spec::SEATS;

#[global_allocator]
static GLOBAL: ti4_perf_alloc::Counting = ti4_perf_alloc::Counting;

fn counts() -> (u64, u64) {
    ti4_perf_alloc::counts()
}

struct Pick {
    seed: u64,
    calls: usize,
}

impl ti4_engine::choice::Decider for Pick {
    fn choose(&mut self, choice: &ti4_engine::choice::Choice) -> Result<ti4_engine::choice::ChoiceOption, ti4_engine::choice::IllegalChoice> {
        let seat = SEATS.iter().position(|x| *x == choice.player.as_str()).unwrap_or(0);
        let index = self.calls;
        self.calls += 1;
        let (picked, _, _) = ti4_rule_bot::choose_explained(choice, None, self.seed, seat, index);
        Ok(choice.options[picked.min(choice.options.len() - 1)].clone())
    }
}

fn report(label: &str, ms: f64, steps: usize, a0: (u64, u64), a1: (u64, u64)) {
    let (n, b) = (a1.0 - a0.0, a1.1 - a0.1);
    println!("{label:<26} {ms:>9.1} ms  steps {steps:>5}  {:>7.1} us/step  allocs {n:>10} ({:>7.0}/step)  bytes {:>8.1} MB ({:>7.1} KB/step)", ms * 1000.0 / steps as f64, n as f64 / steps as f64, b as f64 / 1e6, b as f64 / 1e3 / steps as f64);
}

/// The kernel loop; `mirror`: also materialize the string state after every step (and drop the previous one), as the wrapper does;
/// `per_question`: also materialize once per question asked in the step, as the wrapper's asker does.
fn run_kernel(spec: &ti4_selfplay::spec::GameSpec, mirror: bool, per_question: bool) -> (f64, usize, (u64, u64), (u64, u64)) {
    let mut g = NativeGame::from_spec(spec).expect("builds");
    let a0 = counts();
    let t = Instant::now();
    let mut steps = 0;
    let mut kept = None;
    loop {
        match g.step() {
            Step::Played(p) => {
                steps += 1;
                if mirror || per_question {
                    let (core, cx, _, base, ..) = g.probe();
                    if per_question {
                        for _ in 0..p.asked.len() {
                            std::hint::black_box(materialize::materialize(core, cx, base).unwrap());
                        }
                    }
                    if mirror {
                        kept = Some(materialize::materialize(core, cx, base).unwrap());
                    }
                }
            }
            Step::Finished => break,
            other => panic!("kernel game stopped: {:?}", matches!(other, Step::Gap(_))),
        }
    }
    drop(kept);
    (t.elapsed().as_secs_f64() * 1e3, steps, a0, counts())
}

fn run_wrapper(spec: &ti4_selfplay::spec::GameSpec, until: usize) -> (f64, usize, (u64, u64), (u64, u64), Game<'static>) {
    let legacy = spec.build().expect("builds");
    let galaxy = legacy.galaxy().cloned().expect("map");
    let table = Table::with_default(Box::new(Pick { seed: spec.seed, calls: 0 }));
    let mut game = Game::try_with_table(legacy.state.clone(), ContentStore::embedded(), table).expect("playable").with_galaxy(galaxy);
    drop(legacy);
    let a0 = counts();
    let t = Instant::now();
    let mut steps = 0;
    while !game.state.finished && steps < until {
        let r = game.step();
        assert!(r.error.is_none(), "{:?}", r.error);
        steps += 1;
    }
    (t.elapsed().as_secs_f64() * 1e3, steps, a0, counts(), game)
}

fn run_legacy(spec: &ti4_selfplay::spec::GameSpec) -> (f64, usize, (u64, u64), (u64, u64)) {
    let mut game = ti4_engine_core_bridge::legacy_game::legacy_game(spec).expect("builds");
    game.table.set_default(Box::new(ThinDecider::new(Box::new(Pick { seed: spec.seed, calls: 0 }))));
    let a0 = counts();
    let t = Instant::now();
    let mut steps = 0;
    ti4_engine_legacy::dual::with_mask(ti4_engine_legacy::dual::AreaMask::STRING, || {
        while !game.state.finished && steps < 100_000 {
            assert!(game.step().error.is_none());
            steps += 1;
        }
    });
    (t.elapsed().as_secs_f64() * 1e3, steps, a0, counts())
}

fn time<T>(label: &str, reps: u32, mut f: impl FnMut() -> T) {
    let a0 = counts();
    let t = Instant::now();
    for _ in 0..reps {
        std::hint::black_box(f());
    }
    let us = t.elapsed().as_secs_f64() * 1e6 / f64::from(reps);
    let a1 = counts();
    println!("  {label:<34} {us:>9.1} us   allocs {:>7}   bytes {:>9.1} KB", (a1.0 - a0.0) / u64::from(reps), (a1.1 - a0.1) as f64 / 1e3 / f64::from(reps));
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 3 {
        eprintln!("usage: perf_why <layers|micro> <players> <seed> [reps|at]");
        std::process::exit(2);
    }
    let players: usize = a[1].parse().unwrap();
    let seed: u64 = a[2].parse().unwrap();
    let spec = lobby_spec("sol", players, seed);
    match a[0].as_str() {
        "layers" => {
            let reps: usize = a.get(3).map_or(3, |r| r.parse().unwrap());
            println!("{players}p seed {seed}, random-pick bot, gate build, mimalloc (counting)");
            for (label, m, q) in [("kernel (NativeGame)", false, false), ("kernel + mirror/step", true, false), ("kernel + mirror + 1/question", true, true)] {
                for _ in 0..reps {
                    let (ms, s, a0, a1) = run_kernel(&spec, m, q);
                    report(label, ms, s, a0, a1);
                }
            }
            for _ in 0..reps {
                let (ms, s, a0, a1, _) = run_wrapper(&spec, 100_000);
                report("wrapper (Game)", ms, s, a0, a1);
            }
            for _ in 0..reps {
                let (ms, s, a0, a1) = run_legacy(&spec);
                report("legacy (string Game)", ms, s, a0, a1);
            }
        }
        "micro" => {
            let at: usize = a.get(3).map_or(700, |r| r.parse().unwrap());
            let mut g = NativeGame::from_spec(&spec).expect("builds");
            for _ in 0..at {
                assert!(matches!(g.step(), Step::Played(_)));
            }
            let (core, cx, render, base, kernel, sources) = g.probe();
            println!("{players}p seed {seed}: the LIVE core after {at} kernel steps (round {})", g.round());
            let mirror = materialize::materialize(core, cx, base).expect("mirror");
            println!("  mirror GameState json {} KB; opening position json {} KB; ledger events {}", serde_json::to_string(&mirror).map_or(0, |s| s.len()) / 1024, serde_json::to_string(base).map_or(0, |s| s.len()) / 1024, core.ledger.events.len());
            println!("per-step pieces");
            time("GameState clone (base)", 300, || base.clone());
            time("CoreState clone", 3000, || core.clone());
            time("materialize (clone + store ALL)", 200, || materialize::materialize(core, cx, base).unwrap());
            time("materialize + drop of the old mirror", 200, || {
                let m = materialize::materialize(core, cx, base).unwrap();
                drop(m);
            });
            println!("  store per slice into a clone of the base state");
            for slice in Slice::ALL {
                if slice == Slice::Map {
                    continue;
                }
                let mut gs = base.clone();
                time(&format!("store {slice:?}"), 300, || Codec::store_slice(cx, core, &mut gs, slice).unwrap());
            }
            let mut cx2 = Ctx::of(&mirror).unwrap();
            time("Codec::load all (from the mirror)", 100, || {
                cx2 = Ctx::of(&mirror).unwrap();
                Codec::load(&mut cx2, &mirror, Sources { galaxy: None, rng: None }, SliceSet(SliceSet::ALL.0 & !(1 << Slice::Map as u32))).unwrap()
            });
            time("env_with_plays (registry etc.)", 200, || ti4_engine::live::env::env_with_plays(render, core, kernel, sources));
            time("roster_for x2", 200, || {
                ti4_engine::ties::technology_core::roster_for(ContentStore::embedded(), ti4_model::content_types::FULL);
                ti4_engine::ties::technology_core::roster_for(ContentStore::embedded(), sources)
            });
            let content = ContentStore::embedded();
            time("Observed::new (cheap)", 3000, || ti4_engine::observed::Observed::new(&mirror, content, sources, None));
        }
        _ => std::process::exit(2),
    }
}
