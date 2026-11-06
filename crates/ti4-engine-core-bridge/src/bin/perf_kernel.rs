//! Kernel-only perf bench (docs/PERF_KERNEL.md): whole games driven DIRECTLY through the typed core's driver, with a seeded random pick over the
//! TYPED options. Nothing of the thin layer's per-step work runs here: no wrapper `Game`, no mirror (string `GameState`), no `TableAsker`, no
//! `Observed`, no journal, no question rendering (`render_asking`: option ids, labels, display facts), no event text (`pushed_since`), no decider.
//!
//! What still runs from outside the core, because the core cannot be driven without it (it is timed apart as "adapter" and shows as its own
//! rows in the perf report): `live::env::env_with_plays` (builds the `Env`: the ability registry with a text id per ability, the code table;
//! the ids are memoized in the `Render`), `Cx::texts` (the combat texts the core wants as `Code`s) and `Render::sync_codes` (one code table
//! for the render and the position). Setup is NOT timed: the opening position comes from the string setup (`NativeGame::from_spec`, the core has
//! no setup of its own) and is loaded by the codec.
//!
//! ```text
//! perf_kernel game  <players> <seed> [faction]        one game, JSON line
//! perf_kernel suite [reps]                            the fixed suite (3p..6p, several seeds and faction mixes), JSON line per game + summary
//! perf_kernel loop  <seconds>                         the suite again and again for <seconds> of loop time (for `perf record`)
//! perf_kernel mem   <players> <seed> [faction]        memory: live heap / RSS at step marks, peak, heap per state field at the end
//! perf_kernel micro <players> <seed> [faction] [at]   the per-step pieces on the live core after `at` steps (default 600), each in a loop
//! ```
//! Environment: `PERF_KERNEL_NOSTATE=1` skips the final state hash. `PERF_KERNEL_RUNS=1` (with `game`) counts the window runs (the stage probe's calls). `PERF_KERNEL_CHECK=1` runs the consistency checks of the perf changes after every step
//! (board indexes against a scan of the cells, the deferred play facts against the eager ones, the kept `Env` against one built from scratch; see `checks`), `cargo test --bin perf_kernel` plays a set of games with them on.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use ti4_engine::live::{self, arms};
use ti4_engine::render::Render;
use ti4_engine_core::choice::{Ask as CAsk, Choice, Opt, Pick};
use ti4_engine_core::driver::live::{ask_arm_fn, is_open, live_arm, AskFn, Env, Live, LiveFn};
use ti4_engine_core::driver::session::{live_ask_arm, step_arm, Arm, AskArm, Facts};
use ti4_engine_core::driver::turn::{self, TurnFlowWin};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::{CoreState, WindowKind};
use ti4_engine_core_bridge::native_play::{lobby_spec, NativeGame};
use ti4_model::content_types::SourceSet;

// With `--features perf-count` the counting allocator (allocation counts, bytes, live and peak); without, plain mimalloc, so a `perf record` profile
// has no counter atomics in it (the numbers of the counts are then zero).
#[cfg(feature = "perf-count")]
#[global_allocator]
static GLOBAL: ti4_perf_alloc::Counting = ti4_perf_alloc::Counting;
#[cfg(not(feature = "perf-count"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn counts() -> (u64, u64) {
    #[cfg(feature = "perf-count")]
    return ti4_perf_alloc::counts();
    #[cfg(not(feature = "perf-count"))]
    (0, 0)
}

mod alloc_stats {
    pub fn live() -> u64 {
        #[cfg(feature = "perf-count")]
        return ti4_perf_alloc::live();
        #[cfg(not(feature = "perf-count"))]
        0
    }
    pub fn peak() -> u64 {
        #[cfg(feature = "perf-count")]
        return ti4_perf_alloc::peak();
        #[cfg(not(feature = "perf-count"))]
        0
    }
    pub fn reset_peak() {
        #[cfg(feature = "perf-count")]
        ti4_perf_alloc::reset_peak();
    }
}

/// splitmix64.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// FNV-1a over bytes (the state hash), `u64`.
fn fnv(bytes: &[u8], mut h: u64) -> u64 {
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/statm").ok().and_then(|s| s.split_whitespace().nth(1).and_then(|x| x.parse::<u64>().ok())).map_or(0, |pages| pages * 4)
}

#[derive(Default, Clone, Copy)]
struct Phase {
    ns: u128,
    allocs: u64,
    bytes: u64,
}

impl Phase {
    fn add(&mut self, t: Instant, a0: (u64, u64)) {
        let a1 = counts();
        self.ns += t.elapsed().as_nanos();
        self.allocs += a1.0 - a0.0;
        self.bytes += a1.1 - a0.1;
    }
}

#[derive(Default)]
struct Phases {
    /// `env_with_plays` (adapter).
    env: Phase,
    /// `Cx::texts` + `sync_codes` (adapter).
    texts: Phase,
    /// `Env::clone` per live call.
    env_clone: Phase,
    /// `ask`: the question the arm's open window waits on.
    ask: Phase,
    /// `advance`: the window runs (the rules).
    advance: Phase,
    /// Facts, the arm choice, `asks`, after_arm, the glue tests.
    arm: Phase,
    /// The pick and the chain hash (the bench's own work).
    pick: Phase,
}

struct Game {
    core: CoreState,
    render: Render,
    sources: SourceSet,
    kernel: &'static Kernel,
    seed: u64,
    decided: usize,
    steps: usize,
    chain: u64,
    after_glue: bool,
    ph: Phases,
    /// (step, live heap bytes, rss kB) at the memory marks.
    marks: Vec<(usize, u64, u64)>,
    sample: bool,
    /// Run `checks::after_step` after every step (the board indexes against the scans they replace).
    check: bool,
}

/// What the perf changes must never change, checked against the definitions they replaced (the cell scans), on the live position of a random game.
mod checks {
    use ti4_engine_core::generated::{Planet, System, UnitBaseType};
    use ti4_engine_core::query::{board, fleet, trade};
    use ti4_engine_core::state::CoreState;
    use ti4_engine_core::tables::Scope;
    use ti4_engine_core::{Seat, SysSet};

    /// The scans the board queries used to be (`query::board` before the indexes).
    pub fn after_step(core: &CoreState, step: usize) {
        let b = core.board();
        assert!(b.indexes_agree_with_a_scan(), "step {step}: a board index differs from the scan of the cells");
        // The set queries are cheap and run at every step; the per-system scans (systems x planets) every fourth.
        let seats: Vec<Seat> = core.players().iter().map(|p| p.seat).collect();
        for &seat in &seats {
            let mut with_units = SysSet::EMPTY;
            let mut with_token = SysSet::EMPTY;
            let mut other = SysSet::EMPTY;
            for (system, cell) in b.systems() {
                if cell.units.as_slice().iter().any(|u| u.owner() == seat) {
                    with_units.insert(system.index());
                }
                if cell.cmd.contains(seat) {
                    with_token.insert(system.index());
                }
                if cell.units.as_slice().iter().any(|u| u.owner() != seat) {
                    other.insert(system.index());
                }
            }
            assert_eq!(board::systems_with_units_of(core, seat), with_units, "step {step}: systems_with_units_of {seat:?}");
            assert_eq!(board::systems_with_token(core, seat), with_token, "step {step}: systems_with_token {seat:?}");
            assert_eq!(board::systems_with_units_other_than(core, seat), other, "step {step}: systems_with_units_other_than {seat:?}");
            let mut controlled: Vec<(System, Planet)> = b.planets().filter(|(_, c)| c.control == Some(seat)).filter_map(|(p, c)| c.placed_in.or(p.data().system).map(|s| (s, p))).collect();
            controlled.sort_unstable();
            assert_eq!(board::controlled_planets(core, seat).as_slice(), controlled.as_slice(), "step {step}: controlled_planets {seat:?}");
            assert_eq!(board::controlled_count(core, seat), b.planets().filter(|(_, c)| c.control == Some(seat)).count(), "step {step}: controlled_count {seat:?}");
            // `systems_with_units_anywhere` and `presence` against the loops over systems and planets.
            let mut anywhere = SysSet::EMPTY;
            let mut present = with_units;
            for (system, _) in b.systems() {
                let space = b.sys(system).units.as_slice().iter().any(|u| u.owner() == seat);
                let planets = board::planets_of(core, system).any(|p| b.planet(p).units.as_slice().iter().any(|u| u.owner() == seat));
                if space || planets {
                    anywhere.insert(system.index());
                }
            }
            for (planet, cell) in b.planets() {
                if cell.control == Some(seat) || cell.units.as_slice().iter().any(|u| u.owner() == seat) {
                    if let Some(system) = cell.placed_in.or(planet.data().system) {
                        present.insert(system.index());
                    }
                }
            }
            assert_eq!(board::systems_with_units_anywhere(core, seat), anywhere, "step {step}: systems_with_units_anywhere {seat:?}");
            assert_eq!(trade::presence(core, seat), present, "step {step}: presence {seat:?}");
            for base in UnitBaseType::ALL {
                let mut total = 0i64;
                for (_, cell) in b.systems() {
                    total += cell.units.as_slice().iter().filter(|p| p.owner() == seat && p.kind().data().base_type == base).count() as i64;
                }
                for (_, cell) in b.planets() {
                    total += cell.units.as_slice().iter().filter(|p| p.owner() == seat && p.kind().data().base_type == base).count() as i64;
                }
                for captor in core.players().iter() {
                    total += captor.misc.captured_units.as_slice().iter().filter(|(owner, unit)| *owner == seat && unit.data().base_type == base).count() as i64;
                }
                assert_eq!(fleet::held_on_board(core, Scope(u32::MAX), seat, base), total, "step {step}: held_on_board {seat:?} {base:?}");
            }
        }
        assert_eq!(ti4_engine::ties::combat_live::space_cannon_kinds(core), ti4_engine::ties::combat_live::space_cannon_kinds_by_systems(core), "step {step}: space_cannon_kinds");
        if step % 4 == 0 {
            let placed: Vec<(Planet, System)> = b.planets().filter_map(|(p, c)| c.placed_in.map(|s| (p, s))).collect();
            assert_eq!(board::placed_planets(core), placed, "step {step}: placed_planets");
            for system in System::ALL {
                let scan: Vec<Planet> = system
                    .data()
                    .planets
                    .iter()
                    .copied()
                    .filter(|p| b.planet(*p).placed_in.is_none())
                    .chain(b.planets().filter(|(_, c)| c.placed_in == Some(system)).map(|(p, _)| p))
                    .collect();
                assert_eq!(board::planets_of(core, system).collect::<Vec<_>>(), scan, "step {step}: planets_of {system:?}");
            }
        }
    }
}

#[derive(Debug)]
enum End {
    Finished,
    Defect(#[allow(dead_code)] String),
    StepLimit,
}

/// Weight of an option: opening a transaction weighs a tenth (the shipped bot's rule), everything else 1 (ten and a hundred, to stay in integers).
fn weight(o: &Opt) -> u64 {
    if matches!(o, Opt::Turn(ti4_engine_core::choice::turn::Opt::Open { .. })) { 10 } else { 100 }
}

impl Game {
    fn new(players: usize, seed: u64, faction: &str) -> Result<Self, String> {
        let spec = lobby_spec(faction, players, seed);
        let g = NativeGame::from_spec(&spec)?;
        let (core, cx, render, _base, kernel, sources) = g.probe();
        let mut fresh = Render::new(cx.owners.ids().to_vec()).with_codes(render.codes_snapshot());
        fresh.factions.clone_from(&render.factions);
        Ok(Self { core: core.clone(), render: fresh, sources, kernel, seed, decided: 0, steps: 0, chain: 0xcbf2_9ce4_8422_2325, after_glue: false, ph: Phases::default(), marks: Vec::new(), sample: false, check: std::env::var_os("PERF_KERNEL_CHECK").is_some() })
    }

    /// The seeded weighted random pick over the typed options. `None` for a question with no option.
    fn pick(&mut self, choice: &Choice) -> Option<Pick> {
        let t = Instant::now();
        let a0 = counts();
        let opts = choice.opts.as_slice();
        // The last card of the draft is not a decision (the table's `LoneCard`).
        if opts.len() == 1 && choice.ask == CAsk::Strategy(ti4_engine_core::choice::strategy::Ask::Draft) {
            self.ph.pick.add(t, a0);
            return Some(Pick::Opt(opts[0]));
        }
        if opts.is_empty() {
            return None;
        }
        let total: u64 = opts.iter().map(weight).sum();
        let mut roll = mix(self.seed ^ mix(self.decided as u64 ^ 0xA5A5_5A5A_0000_0000) ^ mix(u64::from(choice.actor.0) ^ 0x5EA7_0000_0000_0000)) % total;
        let mut at = opts.len() - 1;
        for (i, o) in opts.iter().enumerate() {
            let w = weight(o);
            if roll < w {
                at = i;
                break;
            }
            roll -= w;
        }
        self.decided += 1;
        let mut h = DefaultHasher::new();
        (self.chain, choice.actor.0, opts.len(), opts[at]).hash(&mut h);
        self.chain = h.finish();
        self.ph.pick.add(t, a0);
        Some(Pick::Opt(opts[at]))
    }

    /// `live::run_live` without rendering, the table and the rollback clone: the arm's pending question (or the window run to its first), the picks,
    /// the nested questions, up to the arm's next stage question or its end. Returns whether the window finished.
    fn run_live(&mut self, advance: LiveFn, ask: AskFn, env: &Env<'_>, lcx: &live::Cx<'_>) -> Result<bool, String> {
        let t = Instant::now();
        let a0 = counts();
        let mut prepared = env.clone();
        self.ph.env_clone.add(t, a0);
        let t = Instant::now();
        let a0 = counts();
        prepared.table.facts.combat = (lcx.texts)(&mut self.core, lcx.render);
        lcx.render.sync_codes(&mut self.core.texts.codes);
        self.ph.texts.add(t, a0);
        let env = &prepared;
        let t = Instant::now();
        let a0 = counts();
        let first = ask(&self.core, env).map_err(|e| format!("ask: {e:?}"))?;
        self.ph.ask.add(t, a0);
        let mut asking = match first {
            Some(a) => a,
            None => {
                let t = Instant::now();
                let a0 = counts();
                let (live, out) = advance(&mut self.core, env, None).map_err(|e| format!("advance: {e:?}"))?;
                self.ph.advance.add(t, a0);
                let t = Instant::now();
                let a0 = counts();
                lcx.render.sync_codes(&mut self.core.texts.codes);
                self.ph.texts.add(t, a0);
                match live {
                    Live::Done if out.held => return Ok(false),
                    Live::Done => return Ok(true),
                    Live::Asks(a) => *a,
                }
            }
        };
        loop {
            let Some(pick) = self.pick(&asking.choice) else { return Err("a question with no option".to_owned()) };
            let t = Instant::now();
            let a0 = counts();
            let (live, out) = advance(&mut self.core, env, Some(pick)).map_err(|e| format!("advance({pick:?}): {e:?}"))?;
            self.ph.advance.add(t, a0);
            let t = Instant::now();
            let a0 = counts();
            lcx.render.sync_codes(&mut self.core.texts.codes);
            self.ph.texts.add(t, a0);
            match live {
                Live::Done if out.held => return Ok(false),
                Live::Done => return Ok(true),
                Live::Asks(next) if next.nested.is_some() || self.core.pending().nested.is_some() => asking = *next,
                Live::Asks(_) => return Ok(false),
            }
        }
    }

    fn asks(&self) -> bool {
        if is_open::<TurnFlowWin>(&self.core) {
            return true;
        }
        let facts = Facts::of_core(&self.core, false);
        match live_ask_arm(&facts) {
            AskArm::None | AskArm::Nothing => false,
            AskArm::Draft => turn::draft::question(&self.core).is_some(),
            AskArm::StartingTech | AskArm::LeaderFollowers | AskArm::EventScoring | AskArm::Scoring | AskArm::Tokens | AskArm::Voting | AskArm::Diplomacy | AskArm::Aftermath | AskArm::AgendaTalks | AskArm::Tactical => true,
            AskArm::Secondary | AskArm::Closing | AskArm::Trade | AskArm::ActionMenu => self.core.meta().active.is_some() && !ti4_engine_core::driver::turn_flow::all_passed(&self.core),
        }
    }

    /// `NativeGame::step_inner` over the typed driver. `Ok(false)`: the game is over.
    fn step(&mut self) -> Result<bool, String> {
        let t = Instant::now();
        let a0 = counts();
        if self.check {
            live::env::verify_cache(true);
        }
        let env = live::env::env_with_plays(&self.render, &self.core, self.kernel, self.sources);
        self.ph.env.add(t, a0);
        // The deferred play facts: armed on the position the step starts from, read after the step has changed the position, they equal the facts
        // the provider computes at once at the start (and the roster shortcut equals the provider's roster).
        let armed = self.check.then(|| {
            let mut table = env.table.clone();
            table.facts.arm_plays(&self.core, self.kernel);
            assert!(table.facts.plays.is_pending(), "the live env defers the play facts");
            let provider = env.table.facts.plays_provider.clone().expect("the live env has a provider");
            (table, format!("{:?}", (provider.0)(&self.core, self.kernel)), format!("{:?}", env.table.facts.provider_roster(&self.core, self.kernel)), format!("{:?}", (provider.0)(&self.core, self.kernel).and_then(|p| p.roster)))
        });
        let render = std::mem::take(&mut self.render);
        let r = self.step_with(&env, &render);
        self.render = render;
        if let Some((table, eager, roster, eager_roster)) = armed {
            assert_eq!(format!("{:?}", table.facts.plays.as_ref()), eager, "step {}: deferred play facts differ from the eager ones", self.steps);
            assert_eq!(roster, eager_roster, "step {}: provider_roster differs from the provider's roster", self.steps);
        }
        if self.check {
            checks::after_step(&self.core, self.steps);
        }
        r
    }

    fn step_with(&mut self, env: &Env<'_>, render: &Render) -> Result<bool, String> {
        let lcx = live::Cx::new(render, env);
        // 1. The reconcile hooks.
        for _ in 0..8 {
            if self.after_glue {
                break;
            }
            let left_open = is_open::<TurnFlowWin>(&self.core);
            let fin = self.run_live(turn::advance_reconcile, turn::ask_job, env, &lcx)?;
            if fin && !left_open {
                break;
            }
        }
        // 2. The arm.
        let t = Instant::now();
        let a0 = counts();
        let facts = Facts::of_core(&self.core, false);
        let asks = self.asks();
        let arm = step_arm(&facts, asks);
        if arm == Arm::Finished {
            return Ok(false);
        }
        let (Some(advance), Some(ask)) = (live_arm(arm), ask_arm_fn(arm)) else { return Err(format!("arm {} has no live function", arms::name(arm))) };
        self.ph.arm.add(t, a0);
        // 3. The native step.
        let asked_before = self.decided;
        let finished = self.run_live(advance, ask, env, &lcx)?;
        let t = Instant::now();
        let a0 = counts();
        if !env.table.facts.staging {
            // (The thin layer's run_live drops the staged events of a table without a faction module.)
            self.core.pending_mut().staged.clear();
        }
        // 4. The hand-overs between the arms.
        if finished && arm == Arm::Tactical && self.core.pending().window(WindowKind::Tactical).is_none() && self.core.pending().window(WindowKind::Aftermath).is_none() && turn::open_job_of(&self.core).is_none() {
            turn::open_finish(&mut self.core);
        }
        self.steps += 1;
        let asked_none = self.decided == asked_before;
        let handed_off = arm == Arm::Tactical && asked_none && self.core.pending().window(WindowKind::Aftermath).is_some();
        let parked_for_scoring = self.core.pending().window(WindowKind::EventScoring).is_some();
        let removal_next = !finished
            && arm == Arm::Aftermath
            && ti4_engine_core::driver::windows::chain::ask_aftermath(&self.core, env)
                .ok()
                .flatten()
                .is_some_and(|asking| matches!(asking.choice.ask, ti4_engine_core::choice::Ask::Tactical(ti4_engine_core::choice::tactical::Ask::Remove { .. })));
        let glued = matches!(arm, Arm::TurnPrelude | Arm::StrategyAnnounce);
        self.after_glue = !parked_for_scoring && (removal_next || (finished && (glued || arm == Arm::Aftermath || handed_off || (self.after_glue && arm == Arm::Tactical && asked_none))));
        self.ph.arm.add(t, a0);
        Ok(true)
    }

    fn play(&mut self, max_steps: usize) -> End {
        loop {
            if self.steps >= max_steps {
                return End::StepLimit;
            }
            if self.sample && [0, 250, 500, 750, 1000, 1500, 2000].contains(&self.steps) {
                self.marks.push((self.steps, alloc_stats::live(), rss_kb()));
            }
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.step())) {
                Ok(Ok(true)) => {}
                Ok(Ok(false)) => return End::Finished,
                Ok(Err(e)) => return End::Defect(e),
                Err(p) => return End::Defect(format!("PANIC: {}", p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default())),
            }
        }
    }

    fn state_hash(&self) -> u64 {
        if std::env::var_os("PERF_KERNEL_NOSTATE").is_some() {
            return 0;
        }
        fnv(format!("{:?}", self.core).as_bytes(), 0xcbf2_9ce4_8422_2325)
    }
}

struct Result1 {
    players: usize,
    seed: u64,
    faction: String,
    end: String,
    steps: usize,
    decisions: usize,
    ms: f64,
    allocs: u64,
    bytes: u64,
    peak: u64,
    chain: u64,
    state: u64,
    ph: Phases,
    finished: bool,
}

fn play_one(players: usize, seed: u64, faction: &str) -> Result<Result1, String> {
    let mut g = Game::new(players, seed, faction)?;
    alloc_stats::reset_peak();
    let live0 = alloc_stats::live();
    let a0 = counts();
    let t = Instant::now();
    let end = g.play(20_000);
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let a1 = counts();
    let peak = alloc_stats::peak().saturating_sub(live0);
    let finished = matches!(end, End::Finished);
    Ok(Result1 { players, seed, faction: faction.to_owned(), end: format!("{end:?}"), steps: g.steps, decisions: g.decided, ms, allocs: a1.0 - a0.0, bytes: a1.1 - a0.1, peak, chain: g.chain, state: g.state_hash(), ph: std::mem::take(&mut g.ph), finished })
}

fn json(r: &Result1) -> String {
    let p = |n: &str, x: &Phase| format!("\"{n}\":{{\"ms\":{:.1},\"allocs\":{},\"mb\":{:.1}}}", x.ns as f64 / 1e6, x.allocs, x.bytes as f64 / 1e6);
    format!(
        "{{\"players\":{},\"seed\":{},\"faction\":\"{}\",\"end\":{:?},\"steps\":{},\"decisions\":{},\"ms\":{:.1},\"steps_per_s\":{:.0},\"allocs\":{},\"mb\":{:.1},\"peak_live_mb\":{:.2},\"chain\":\"{:016x}\",\"state\":\"{:016x}\",{}}}",
        r.players, r.seed, r.faction, r.end, r.steps, r.decisions, r.ms, r.steps as f64 / (r.ms / 1e3), r.allocs, r.bytes as f64 / 1e6, r.peak as f64 / 1e6, r.chain, r.state,
        [p("env", &r.ph.env), p("texts", &r.ph.texts), p("env_clone", &r.ph.env_clone), p("ask", &r.ph.ask), p("advance", &r.ph.advance), p("arm", &r.ph.arm), p("pick", &r.ph.pick)].join(",")
    )
}

/// The fixed suite: (players, seed, seat-0 faction). Several faction mixes (the other seats rotate by seed), 3p to 6p.
const SUITE: &[(usize, u64, &str)] = &[
    (3, 1, "sol"), (3, 2, "hacan"), (3, 3, "yssaril"),
    (4, 1, "sol"), (4, 2, "letnev"), (4, 3, "jolnar"), (4, 4, "mahact"), (4, 5, "nekro"),
    (5, 1, "xxcha"), (5, 2, "l1z1x"), (5, 3, "ghost"),
    (6, 1, "arborec"), (6, 2, "muaat"), (6, 3, "naalu"),
];

/// Heap bytes freed by dropping the value (live-size difference): what the value owned alone.
fn heap_of<T>(name: &str, v: T, size: usize) {
    let l0 = alloc_stats::live();
    drop(v);
    let freed = l0.saturating_sub(alloc_stats::live());
    println!("  {name:<12} size_of {size:>6} B   heap freed on drop {freed:>9} B");
}

fn mem(players: usize, seed: u64, faction: &str) {
    println!("process start: live {} kB, rss {} kB", alloc_stats::live() / 1000, rss_kb());
    let mut g = Game::new(players, seed, faction).expect("builds");
    g.sample = true;
    println!("after setup:   live {} kB, rss {} kB", alloc_stats::live() / 1000, rss_kb());
    alloc_stats::reset_peak();
    let live0 = alloc_stats::live();
    let end = g.play(20_000);
    g.marks.push((g.steps, alloc_stats::live(), rss_kb()));
    println!("{players}p seed {seed} {faction}: {end:?}, {} steps, {} decisions, peak live during play {:.2} MB over the start", g.steps, g.decided, alloc_stats::peak().saturating_sub(live0) as f64 / 1e6);
    println!("step marks (step, live heap kB, rss kB):");
    for (s, l, r) in &g.marks {
        println!("  {s:>6} {:>9} {r:>9}", l / 1000);
    }
    println!("end position, one owner each (fields moved out of the state and dropped):");
    let core = std::mem::take(&mut g.core);
    let total0 = alloc_stats::live();
    let mut core = core;
    macro_rules! field {
        ($($f:ident),*) => {$( heap_of(stringify!($f), std::mem::take(&mut core.$f), std::mem::size_of_val(&core.$f)); )*};
    }
    println!("  ledger: {} events, {} dice, {} vp rows, {} relic rows; size_of Emit {} B, RollRec {} B, VpRec {} B", core.ledger.events.len(), core.ledger.dice.len(), core.ledger.vp.len(), core.ledger.relics.len(), std::mem::size_of::<ti4_engine_core::state::Emit>(), std::mem::size_of::<ti4_engine_core::state::RollRec>(), std::mem::size_of::<ti4_engine_core::state::VpRec>());
    heap_of("ledger.events", std::mem::take(&mut core.ledger.events), 0);
    heap_of("ledger.dice", std::mem::take(&mut core.ledger.dice), 0);
    heap_of("ledger.vp", std::mem::take(&mut core.ledger.vp), 0);
    heap_of("ledger.relics", std::mem::take(&mut core.ledger.relics), 0);
    field!(board, map, players, decks, cards, objectives, agenda, strategy, explore, trade, marks, transient, pending, rng, ledger, texts, meta, seats);
    println!("  state total live before the drops {} kB; left in the (emptied) state {} kB", total0 / 1000, alloc_stats::live() / 1000);
    println!("size_of CoreState {} B", std::mem::size_of::<CoreState>());
    let hwm = std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|l| l.starts_with("VmHWM")).map(str::to_owned)).unwrap_or_default();
    println!("{hwm} (peak resident set of the process, setup included)");
}

fn timeit<T>(label: &str, reps: u32, mut f: impl FnMut() -> T) {
    let a0 = counts();
    let t = Instant::now();
    for _ in 0..reps {
        std::hint::black_box(f());
    }
    let us = t.elapsed().as_secs_f64() * 1e6 / f64::from(reps);
    let a1 = counts();
    println!("  {label:<46} {us:>9.2} us   allocs {:>6}   bytes {:>9.1} KB", (a1.0 - a0.0) / u64::from(reps), (a1.1 - a0.1) as f64 / 1e3 / f64::from(reps));
}

fn micro(players: usize, seed: u64, faction: &str, at: usize) {
    let mut g = Game::new(players, seed, faction).expect("builds");
    let end = g.play(at);
    println!("{players}p seed {seed} {faction}: played {} steps ({end:?}); per-step pieces on this position (tight loops, hot cache):", g.steps);
    let env = live::env::env_with_plays(&g.render, &g.core, g.kernel, g.sources);
    println!("  ability specs {}, size_of Spec {} B, AbilityDef {} B, AbilityTable {} B, Env {} B, CoreState {} B", env.table.specs().len(), std::mem::size_of::<ti4_engine_core::kernel::ability::Spec>(), std::mem::size_of::<ti4_engine_core::kernel::AbilityDef>(), std::mem::size_of::<ti4_engine_core::kernel::stage::AbilityTable>(), std::mem::size_of::<Env<'_>>(), std::mem::size_of::<CoreState>());
    {
        use ti4_engine_core::kernel::ability::Applies;
        let specs = env.table.specs();
        let (mut every, mut faction, mut held, mut every_cond) = (0, 0, 0, 0);
        for sp in specs {
            match sp.applies {
                Applies::EverySeat => {
                    every += 1;
                    every_cond += usize::from(sp.condition.is_some());
                }
                Applies::Faction(_) => faction += 1,
                Applies::Held(_) => held += 1,
            }
        }
        println!("  specs by `applies`: every seat {every} (with a condition fn: {every_cond}), one faction {faction}, held-predicate {held}; registry rows (all seats) {}", env.registry.len());
    }
    timeit("CoreState::clone", 20_000, || g.core.clone());
    timeit("CoreState::clone + one ledger push (tail copy)", 20_000, || {
        let mut c = g.core.clone();
        c.ledger.vp.push(ti4_engine_core::state::VpRec { player: ti4_engine_core::Seat(0), delta: 0, reason: ti4_engine_core::state::LedgerReason::Custodians });
        c
    });
    timeit("env_with_plays (registry + table + codes)", 400, || live::env::env_with_plays(&g.render, &g.core, g.kernel, g.sources));
    timeit("Env::clone (AbilityTable + registry)", 4_000, || env.clone());
    timeit("AbilityTable::clone", 4_000, || env.table.clone());
    timeit("Facts::of_core", 200_000, || Facts::of_core(&g.core, false));
    timeit("asks()", 100_000, || g.asks());
    let lcx = live::Cx::new(&g.render, &env);
    timeit("combat_texts + sync_codes (Cx::texts)", 2_000, || {
        let t = (lcx.texts)(&mut g.core, lcx.render);
        lcx.render.sync_codes(&mut g.core.texts.codes);
        t
    });
    timeit("Render::sync_codes alone (equal tables)", 20_000, || g.render.sync_codes(&mut g.core.texts.codes));
    println!("  texts in the code table: {}", g.core.texts.codes.len());
    timeit("planets_of for all 231 systems (current)", 400, || System::ALL.iter().map(|s| ti4_engine_core::query::board::planets_of(&g.core, *s).count()).sum::<usize>());
    timeit("Board::planet x 159 (one pass)", 20_000, || g.core.board().planets().filter(|(_, c)| c.control.is_some()).count());
}

use ti4_engine_core::generated::System;

/// `PERF_KERNEL_RUNS=1`: the stage probe's calls (one per run of a window operation and one per stop of a driver: the difference of two builds is
/// the difference of their runs, docs/NESTQ1_RESUME.md).
static SEEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn count_seen(_: &'static str, _: &dyn std::fmt::Debug) {
    SEEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// The nested answers by path (`PERF_KERNEL_RUNS=1`): parked ones by card and ability, replayed ones by the window kind, each by the question's subtype.
fn print_park_detail() {
    for (key, n) in ti4_engine_core::driver::park_detail_counts() {
        let path = match key.parked {
            Some((card, primary)) => format!("parked {card:?}/{}", if primary { "primary" } else { "secondary" }),
            None => format!("replay {:?}", key.kind),
        };
        eprintln!("{n:6} {path} {:?}", key.subtype);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // stepmachine-1: `PERF_KERNEL_PARK=0` answers every nested question by the replay (the parked continuation switched off),
    // `PERF_KERNEL_VERIFY_PARK=1` also replays every parked answer and panics when the two differ.
    if std::env::var("PERF_KERNEL_PARK").is_ok_and(|v| v == "0") {
        ti4_engine_core::driver::set_parking(false);
    }
    if std::env::var_os("PERF_KERNEL_VERIFY_PARK").is_some() {
        ti4_engine_core::driver::live::verify_parked(true);
    }
    match args.get(1).map(String::as_str) {
        Some("game") => {
            let (p, s) = (args[2].parse().unwrap(), args[3].parse().unwrap());
            let f = args.get(4).map_or("sol", String::as_str);
            let runs = std::env::var_os("PERF_KERNEL_RUNS").is_some();
            if runs {
                ti4_engine_core::driver::probe::set(Some(count_seen));
                ti4_engine_core::driver::park_detail(true);
            }
            let r = play_one(p, s, f).expect("builds");
            println!("{}", json(&r));
            if runs {
                let (parks, resumed) = ti4_engine_core::driver::park_stats();
                eprintln!("{{\"probe_calls\":{},\"parks\":{parks},\"parked_answers\":{resumed}}}", SEEN.load(std::sync::atomic::Ordering::Relaxed));
                print_park_detail();
            }
        }
        Some("setup") => {
            let (p, s) = (args[2].parse().unwrap(), args[3].parse().unwrap());
            let g = Game::new(p, s, args.get(4).map_or("sol", String::as_str)).expect("builds");
            println!("{{\"setup\":true,\"cells\":{}}}", g.core.meta().round);
        }
        Some("micro") => {
            let (p, s) = (args[2].parse().unwrap(), args[3].parse().unwrap());
            micro(p, s, args.get(4).map_or("sol", String::as_str), args.get(5).and_then(|x| x.parse().ok()).unwrap_or(600));
        }
        Some("mem") => {
            let (p, s) = (args[2].parse().unwrap(), args[3].parse().unwrap());
            mem(p, s, args.get(4).map_or("sol", String::as_str));
        }
        Some("suite") => {
            let reps: usize = args.get(2).and_then(|x| x.parse().ok()).unwrap_or(1);
            if std::env::var_os("PERF_KERNEL_RUNS").is_some() {
                ti4_engine_core::driver::park_detail(true);
            }
            let (mut steps, mut ms, mut allocs, mut bytes, mut dec, mut bad) = (0usize, 0.0, 0u64, 0u64, 0usize, 0);
            let mut all = 0xcbf2_9ce4_8422_2325u64;
            for _ in 0..reps {
                for &(p, s, f) in SUITE {
                    let r = play_one(p, s, f).expect("builds");
                    println!("{}", json(&r));
                    steps += r.steps;
                    dec += r.decisions;
                    ms += r.ms;
                    allocs += r.allocs;
                    bytes += r.bytes;
                    if !r.finished {
                        bad += 1;
                    }
                    all = fnv(&r.chain.to_le_bytes(), all);
                    all = fnv(&r.state.to_le_bytes(), all);
                }
            }
            let (parks, resumed) = ti4_engine_core::driver::park_stats();
            eprintln!("{{\"parks\":{parks},\"parked_answers\":{resumed}}}");
            print_park_detail();
            println!("{{\"summary\":true,\"games\":{},\"unfinished\":{bad},\"steps\":{steps},\"decisions\":{dec},\"ms\":{ms:.1},\"steps_per_s\":{:.0},\"allocs\":{allocs},\"mb\":{:.1},\"suite_hash\":\"{all:016x}\"}}", SUITE.len() * reps, steps as f64 / (ms / 1e3), bytes as f64 / 1e6);
        }
        Some("loop") => {
            let secs: f64 = args[2].parse().unwrap();
            let start = Instant::now();
            let mut loop_ms = 0.0;
            let mut games = 0;
            while start.elapsed().as_secs_f64() < secs {
                for &(p, s, f) in SUITE {
                    let r = play_one(p, s, f).expect("builds");
                    loop_ms += r.ms;
                    games += 1;
                }
            }
            println!("{{\"loop\":true,\"games\":{games},\"loop_ms\":{loop_ms:.0}}}");
        }
        _ => eprintln!("usage: perf_kernel game <players> <seed> [faction] | suite [reps] | loop <seconds> | mem <players> <seed> [faction]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Random play of many games (3p to 6p, several factions) with the checks of `checks::after_step` after every step: the board indexes and the
    /// queries built on them agree with the scans of the cells at every position the games reach.
    #[test]
    fn the_perf_changes_agree_with_what_they_replaced_after_random_play_of_many_games() {
        let mut steps = 0;
        for &(players, seed, faction) in SUITE.iter().take(8).chain(&[(4, 7, "hacan"), (4, 8, "ghost"), (3, 9, "empyrean"), (5, 10, "saar"), (6, 11, "arborec")]) {
            let mut g = Game::new(players, seed, faction).expect("builds");
            g.check = true;
            let end = g.play(20_000);
            assert!(matches!(end, End::Finished), "{players}p seed {seed} {faction}: {end:?}");
            steps += g.steps;
        }
        assert!(steps > 5_000, "{steps} steps checked");
    }
}
