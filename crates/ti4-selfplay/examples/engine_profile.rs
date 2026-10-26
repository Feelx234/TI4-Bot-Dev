//! Timing counters for the engine's workloads besides plain self-play (docs/ENGINE_OPTIMIZATION.md):
//!
//! ```text
//! engine_profile game   CONFIG.json [N]   bot games of the config (first N), and per game:
//!                                         engine vs bot time, the views a server would build at
//!                                         every question (one per seat and a spectator) and their
//!                                         JSON, then the server's replay of the decisions
//!                                         (`ti4_host::replay::replay_session`) and a bare replay
//! engine_profile solo   SEED STEPS PLAYERS   `ti4_wasm::ti4_run_seeded` natively (random seats; the
//!                                         same call as `node crates/ti4-wasm/js/run.mjs`)
//! engine_profile resume SAVE.json [N]     the browser host's resume of an offline save (a feedback
//!                                         report or a save), natively; tools/engine_profile/
//!                                         wasm_resume.mjs times the same in WebAssembly
//! ```
//!
//! Every time is CPU time of this thread (`CLOCK_THREAD_CPUTIME_ID` via /proc is not portable, so
//! it is wall time of a single thread: run it on a quiet core, or several times). The games are
//! played exactly as `selfplay run` plays them: the printed hash equals the bench's for the spec.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use ti4_engine::choice::{
    Choice, ChoiceOption, Decider, IllegalChoice, Scripted, SeatObservation, SeededRandom,
};
use ti4_engine::game::Game;
use ti4_host::protocol::status::ViewerRole;
use ti4_model::id::PlayerId;
use ti4_selfplay::spec::{Matrix, SEATS};

struct Bot {
    seed: u64,
    decided: usize,
    random: SeededRandom,
    bot: Rc<RefCell<Duration>>,
}

impl Bot {
    fn decide(
        &mut self,
        choice: &Choice,
        seen: Option<&SeatObservation<'_>>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        let started = Instant::now();
        let index = self.decided;
        let option = match seen {
            Some(seen) if !choice.options.is_empty() => {
                let seat = SEATS
                    .iter()
                    .position(|s| *s == choice.player.as_str())
                    .unwrap_or(0);
                ti4_rule_bot::tuning::set(ti4_rule_bot::tuning::current());
                let (picked, _, _) =
                    ti4_rule_bot::choose_explained(choice, Some(seen), self.seed, seat, index);
                choice.options[picked].clone()
            }
            _ => self.random.choose(choice)?,
        };
        self.decided += 1;
        *self.bot.borrow_mut() += started.elapsed();
        Ok(option)
    }
}

struct Shared(Rc<RefCell<Bot>>);

impl Decider for Shared {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().decide(choice, None)
    }
    fn choose_seeing(
        &mut self,
        choice: &Choice,
        seen: &SeatObservation<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().decide(choice, Some(seen))
    }
}

#[derive(Default)]
struct Times {
    steps: Duration,
    bot: Duration,
    options: Duration,
    views: Duration,
    json: Duration,
    json_bytes: usize,
    boundaries: usize,
    replay_server: Duration,
    replay_bare: Duration,
    decisions: usize,
    rounds: u32,
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn game(config: &str, limit: Option<usize>) -> Result<(), String> {
    let text = std::fs::read_to_string(config).map_err(|e| format!("{config}: {e}"))?;
    let matrix: Matrix = serde_json::from_str(&text).map_err(|e| format!("{config}: {e}"))?;
    let (specs, _) = ti4_selfplay::driver::expand(&matrix)?;
    let content = ti4_content::ContentStore::embedded();
    let mut total = Times::default();
    println!(
        "seed players decisions rounds | steps_ms bot_ms engine_ms | options_ms views_ms json_ms json_kb/boundary | replay_server_ms replay_bare_ms | hash"
    );
    for spec in specs.iter().take(limit.unwrap_or(usize::MAX)) {
        let mut game = spec.build()?;
        let initial = game.state.clone();
        let galaxy = game.galaxy().cloned();
        let bot_time = Rc::new(RefCell::new(Duration::ZERO));
        game.table
            .set_default(Box::new(Shared(Rc::new(RefCell::new(Bot {
                seed: spec.seed,
                decided: 0,
                random: SeededRandom::new(spec.seed),
                bot: bot_time.clone(),
            })))));
        let tiles = galaxy
            .as_ref()
            .map_or_else(Vec::new, |g| ti4_host::map::build_board_tiles(content, g));
        let layout = ti4_host::map::GalaxyLayout::from_galaxy(
            galaxy.as_ref().ok_or("the spec builds no map")?,
            ti4_model::content_types::POK,
        );
        let mut viewers: Vec<ViewerRole> = game
            .state
            .players
            .iter()
            .map(|p| ViewerRole::Player(p.id.clone()))
            .collect();
        viewers.push(ViewerRole::Spectator);
        let mut t = Times::default();
        let mut version = 0u64;
        let mut steps = 0usize;
        while !game.state.finished
            && game.state.round <= matrix.limits.max_rounds
            && steps < matrix.limits.max_steps
        {
            // The server's work at a boundary where a seat is asked: the question and one update
            // per subscriber (here every seat and a spectator), serialized.
            let started = Instant::now();
            let pending = game.legal_options();
            t.options += started.elapsed();
            if let Some(choice) = &pending {
                t.boundaries += 1;
                version += 1;
                let started = Instant::now();
                let updates: Vec<_> = viewers
                    .iter()
                    .map(|viewer| {
                        ti4_host::projection::project_state_update_with_map(
                            "profile",
                            version,
                            &game.state,
                            viewer,
                            Some((choice, "nonce")),
                            &tiles,
                            &layout,
                            game.galaxy(),
                            None,
                        )
                    })
                    .collect();
                t.views += started.elapsed();
                let started = Instant::now();
                for update in &updates {
                    t.json_bytes += serde_json::to_string(update)
                        .map_err(|e| e.to_string())?
                        .len();
                }
                t.json += started.elapsed();
            }
            let started = Instant::now();
            let result = game.step();
            t.steps += started.elapsed();
            steps += 1;
            if let Some(error) = result.error {
                return Err(format!("seed {}: {error}", spec.seed));
            }
        }
        t.bot = *bot_time.borrow();
        let records = game.table.log.records.clone();
        let hash = ti4_wasm::host_session::decisions_hash(&records);
        t.decisions = records.len();
        t.rounds = game.state.round;
        let started = Instant::now();
        let report = ti4_host::replay::replay_session(&initial, galaxy.as_ref(), &records)
            .map_err(|e| format!("replay: {e:?}"))?;
        t.replay_server = started.elapsed();
        let started = Instant::now();
        let mut again = Game::with_table(
            initial.clone(),
            content,
            ti4_engine::choice::Table::with_default(Box::new(Scripted::new(
                records.iter().map(|r| r.chosen.clone()),
            ))),
        );
        if let Some(g) = &galaxy {
            again = again.with_galaxy(g.clone());
        }
        while again.table.log.records.len() < records.len() {
            if again.step().error.is_some() {
                break;
            }
        }
        t.replay_bare = started.elapsed();
        let same = report.hashes_match && again.table.log.records.len() == records.len();
        println!(
            "{} {} {} {} | {:.0} {:.0} {:.0} | {:.0} {:.0} {:.0} {:.1} | {:.0} {:.0} | {}{}",
            spec.seed,
            spec.players,
            t.decisions,
            t.rounds,
            ms(t.steps),
            ms(t.bot),
            ms(t.steps.saturating_sub(t.bot)),
            ms(t.options),
            ms(t.views),
            ms(t.json),
            t.json_bytes as f64 / 1024.0 / t.boundaries.max(1) as f64,
            ms(t.replay_server),
            ms(t.replay_bare),
            hash,
            if same { "" } else { " REPLAY-DIFFERS" },
        );
        total.steps += t.steps;
        total.bot += t.bot;
        total.options += t.options;
        total.views += t.views;
        total.json += t.json;
        total.json_bytes += t.json_bytes;
        total.boundaries += t.boundaries;
        total.replay_server += t.replay_server;
        total.replay_bare += t.replay_bare;
        total.decisions += t.decisions;
    }
    let all = total.steps + total.options + total.views + total.json;
    let pct = |d: Duration| 100.0 * d.as_secs_f64() / all.as_secs_f64().max(1e-9);
    println!(
        "TOTAL decisions {} boundaries {} | bot {:.0} ms ({:.1}%), engine steps {:.0} ms ({:.1}%), \
         extra option generation {:.0} ms ({:.1}%), views {:.0} ms ({:.1}%), json {:.0} ms ({:.1}%, {:.1} KiB per boundary) \
         | replay: server {:.0} ms ({:.3} ms/decision), bare {:.0} ms ({:.3} ms/decision)",
        total.decisions,
        total.boundaries,
        ms(total.bot),
        pct(total.bot),
        ms(total.steps.saturating_sub(total.bot)),
        pct(total.steps.saturating_sub(total.bot)),
        ms(total.options),
        pct(total.options),
        ms(total.views),
        pct(total.views),
        ms(total.json),
        pct(total.json),
        total.json_bytes as f64 / 1024.0 / total.boundaries.max(1) as f64,
        ms(total.replay_server),
        ms(total.replay_server) / total.decisions.max(1) as f64,
        ms(total.replay_bare),
        ms(total.replay_bare) / total.decisions.max(1) as f64,
    );
    Ok(())
}

fn solo(seed: u32, players: u32, steps: u32) -> Result<(), String> {
    let started = Instant::now();
    let text = ti4_wasm::response(ti4_wasm::ti4_run_seeded(seed, players, steps))?;
    let elapsed = started.elapsed();
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    println!(
        "solo seed {seed} players {players} steps {steps}: decisions {} round {} in {:.0} ms ({:.3} ms/decision)",
        value["decisions"],
        value["round"],
        ms(elapsed),
        ms(elapsed) / value["decisions"].as_f64().unwrap_or(1.0),
    );
    Ok(())
}

/// The browser host's resume, natively: what `node tools/engine_profile/wasm_resume.mjs` times
/// in WebAssembly (the same save, the same host code).
fn resume(path: &str, repeats: usize) -> Result<(), String> {
    let file: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| format!("{path}: {e}"))?)
            .map_err(|e| format!("{path}: {e}"))?;
    let save = [
        file.pointer("/context/local/save"),
        file.pointer("/local/save"),
        file.get("save"),
        Some(&file),
    ]
    .into_iter()
    .flatten()
    .find(|v| v.get("game").is_some() && v.get("decisions").is_some())
    .cloned()
    .ok_or("no save found")?;
    ti4_wasm::set_native_host(|text: &str| {
        if text.contains("\"kind\":\"progress\"") {
            ti4_wasm::host_api::CONTINUE
        } else {
            -1
        }
    });
    let request = serde_json::json!({ "resume": save }).to_string();
    for i in 0..repeats {
        let len = ti4_wasm::host_api::put_input(&request);
        let started = Instant::now();
        let status = ti4_wasm::ti4_host_start(u32::try_from(len).map_err(|e| e.to_string())?);
        println!(
            "resume {}: status {status} in {:.0} ms",
            i + 1,
            ms(started.elapsed())
        );
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let _ = PlayerId::new("a");
    match args.first().map(String::as_str) {
        Some("game") => game(
            args.get(1).ok_or("game CONFIG.json [N]")?,
            args.get(2).and_then(|n| n.parse().ok()),
        ),
        Some("solo") => {
            let n = |i: usize, d: u32| args.get(i).and_then(|v| v.parse().ok()).unwrap_or(d);
            solo(n(1, 3), n(3, 8), n(2, 2000))
        }
        Some("resume") => resume(
            args.get(1).ok_or("resume SAVE.json [REPEATS]")?,
            args.get(2).and_then(|n| n.parse().ok()).unwrap_or(3),
        ),
        _ => Err(
            "usage: engine_profile game CONFIG.json [N] | solo SEED STEPS PLAYERS (as js/run.mjs) | resume SAVE.json [N]"
                .to_owned(),
        ),
    }
}
