//! Perf A/B (docs/PERF_AB.md): ONE bot game per process on the legacy string engine or on the core (the `Game` wrapper), timed.
//!
//! ```text
//! perf_ab <legacy|core> <smart|random|record:FILE|replay:FILE> <players> <seed> [faction]
//! ```
//! `smart`: the shipped rule bot reading the view (`choose_explained` with `Some(seen)`; where an engine offers no view the bot picks at random).
//! `random`: the rule bot's seeded random pick only, never reading a view (what `native_play` plays). `record:F`: as smart, writes the chosen
//! option ids to F. `replay:F`: answers from F by index (no bot work, no view), to isolate the engine's own cost.
//! Prints one JSON line: wall time of the step loop, time spent inside the decider, setup time, steps, decisions, peak RSS, a chain hash.

use std::time::Instant;

use sha2::{Digest, Sha256};
use ti4_content::ContentStore;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation};
use ti4_engine::native::Game;
use ti4_engine::wire::choice::Table;
use ti4_engine_core_bridge::legacy_adapter::ThinDecider;
use ti4_engine_core_bridge::native_play::lobby_spec;
use ti4_selfplay::spec::SEATS;
use std::cell::RefCell;
use std::rc::Rc;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Clone)]
enum Mode {
    Smart,
    Random,
    Record(String),
    Replay(Vec<String>),
}

#[derive(Default)]
struct Stats {
    ns: u128,
    calls: usize,
    seeing: usize,
    chosen: Vec<String>,
    full: Vec<String>,
    chain: String,
    bad: Option<String>,
}

struct Bot {
    seed: u64,
    mode: Mode,
    stats: Rc<RefCell<Stats>>,
}

impl Bot {
    fn decide(&mut self, choice: &Choice, seen: Option<&SeatObservation<'_>>) -> Result<ChoiceOption, IllegalChoice> {
        let t = Instant::now();
        let mut s = self.stats.borrow_mut();
        let index = s.calls;
        s.calls += 1;
        let option = if let Mode::Replay(ids) = &self.mode {
            let want = ids.get(index).cloned().unwrap_or_default();
            match choice.options.iter().find(|o| o.id == want) {
                Some(o) => o.clone(),
                None => {
                    s.bad.get_or_insert_with(|| format!("replay: decision {index} wants {want:?}, not offered"));
                    choice.options[0].clone()
                }
            }
        } else {
            let seat = SEATS.iter().position(|x| *x == choice.player.as_str()).unwrap_or(0);
            let view = if matches!(self.mode, Mode::Random) { None } else { seen.filter(|_| !choice.options.is_empty()) };
            if view.is_some() {
                s.seeing += 1;
            }
            let (picked, _, _) = ti4_rule_bot::choose_explained(choice, view, self.seed, seat, index);
            choice.options[picked.min(choice.options.len() - 1)].clone()
        };
        let mut h = Sha256::new();
        h.update(s.chain.as_bytes());
        h.update(format!("{}\u{1}{}\u{1}{}", choice.player, choice.prompt, option.id).as_bytes());
        s.chain = h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect();
        if matches!(self.mode, Mode::Record(_)) {
            s.chosen.push(option.id.clone());
            let offered: Vec<&str> = choice.options.iter().map(|o| o.id.as_str()).collect();
            s.full.push(format!("{index} {} | {} | {} of {} | {}", choice.player, choice.prompt, option.id, choice.options.len(), offered.join(",").chars().take(200).collect::<String>()));
        }
        s.ns += t.elapsed().as_nanos();
        Ok(option)
    }
}

impl Decider for Bot {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.decide(choice, None)
    }
    fn choose_seeing(&mut self, choice: &Choice, seen: &SeatObservation<'_>) -> Result<ChoiceOption, IllegalChoice> {
        self.decide(choice, Some(seen))
    }
    fn wants_view(&self) -> bool {
        matches!(self.mode, Mode::Smart | Mode::Record(_))
    }
}

fn hwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmHWM:")).and_then(|l| l.split_whitespace().nth(1).and_then(|n| n.parse().ok())))
        .unwrap_or(0)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 4 {
        eprintln!("usage: perf_ab <legacy|core> <smart|random|record:FILE|replay:FILE> <players> <seed> [faction]");
        std::process::exit(2);
    }
    let engine = a[0].clone();
    let mode = match a[1].split_once(':') {
        None if a[1] == "smart" => Mode::Smart,
        None if a[1] == "random" => Mode::Random,
        Some(("record", f)) => Mode::Record(f.to_owned()),
        Some(("replay", f)) => Mode::Replay(std::fs::read_to_string(f).expect("replay file").lines().map(str::to_owned).collect()),
        _ => {
            eprintln!("bad mode");
            std::process::exit(2);
        }
    };
    let players: usize = a[2].parse().unwrap();
    let seed: u64 = a[3].parse().unwrap();
    let faction = a.get(4).map_or("sol", String::as_str);

    let setup = Instant::now();
    let spec = lobby_spec(faction, players, seed);
    let stats = Rc::new(RefCell::new(Stats::default()));
    let bot = Bot { seed, mode: mode.clone(), stats: stats.clone() };
    let (mut steps, mut errors) = (0usize, 0usize);
    let (setup_ns, loop_ns, rounds, finished, records);
    if engine == "legacy" {
        let mut game = ti4_engine_core_bridge::legacy_game::legacy_game(&spec).expect("a lobby game builds");
        game.table.set_default(Box::new(ThinDecider::new(Box::new(bot))));
        setup_ns = setup.elapsed().as_nanos();
        let t = Instant::now();
        ti4_engine_legacy::dual::with_mask(ti4_engine_legacy::dual::AreaMask::STRING, || {
            while !game.state.finished && steps < 100_000 {
                if game.step().error.is_some() {
                    errors += 1;
                    break;
                }
                steps += 1;
            }
        });
        loop_ns = t.elapsed().as_nanos();
        rounds = game.state.round;
        finished = game.state.finished;
        records = game.table.log.records.len();
    } else {
        let legacy = spec.build().expect("a lobby game builds");
        let galaxy = legacy.galaxy().cloned().expect("a lobby game has a map");
        let table = Table::with_default(Box::new(bot));
        let mut game = Game::try_with_table(legacy.state.clone(), ContentStore::embedded(), table).expect("playable").with_galaxy(galaxy);
        drop(legacy);
        setup_ns = setup.elapsed().as_nanos();
        let t = Instant::now();
        while !game.state.finished && steps < 100_000 {
            let r = game.step();
            if let Some(e) = r.error {
                errors += 1;
                eprintln!("step error at step {steps}: {e}");
                break;
            }
            steps += 1;
        }
        loop_ns = t.elapsed().as_nanos();
        rounds = game.state.round;
        finished = game.state.finished;
        records = game.table.log.records.len();
    }
    let s = stats.borrow();
    if let Mode::Record(f) = &mode {
        std::fs::write(f, s.chosen.join("\n") + "\n").expect("write record");
        std::fs::write(format!("{f}.full"), s.full.join("\n") + "\n").expect("write record");
    }
    println!(
        "{{\"engine\":\"{engine}\",\"mode\":\"{}\",\"players\":{players},\"seed\":{seed},\"finished\":{finished},\"errors\":{errors},\"rounds\":{rounds},\"steps\":{steps},\"decisions\":{},\"journal\":{records},\"seeing\":{},\"loop_ms\":{:.1},\"decider_ms\":{:.1},\"setup_ms\":{:.1},\"hwm_kb\":{},\"chain\":\"{}\",\"bad\":{:?}}}",
        a[1].split(':').next().unwrap(),
        s.calls,
        s.seeing,
        loop_ns as f64 / 1e6,
        s.ns as f64 / 1e6,
        setup_ns as f64 / 1e6,
        hwm_kb(),
        s.chain,
        s.bad
    );
}
