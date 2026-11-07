//! Selfplay on the core driver (`docs/MCTS_BOT_PLAN.md` 4.4): every seat a `SeatAgent`, the game's streams answering the dice.
//!
//! ```text
//! search_selfplay [--setup mcts|quick|lobby:FACTION:PLAYERS] [--games N] [--seed S] [--agents rule,rule,blind,random]
//!                 [--max-rounds R] [--out SUMMARY.json] [--games-out GAMES.jsonl]
//! ```
//!
//! Agents: `rule` (the rule bot with its typed view), `blind` (the rule bot's random pick, the quick game's bot), `random` (a uniform
//! legal answer from the search stream, seeded by the game seed and the seat). The policy-net and MCTS agents plug in through the same
//! trait (`ti4_engine_search::agent`). A game is reproducible from (setup, seed, agents).

use std::io::Write as _;
use std::time::Instant;

use ti4_engine_search::agent::{RandomAgent, RuleBotAgent, SeatAgent};
use ti4_engine_search::chance::SearchRng;
use ti4_engine_search::driver::{self, Limits};

#[global_allocator]
static GLOBAL: ti4_perf_alloc::Counting = ti4_perf_alloc::Counting;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn spec(setup: &str, seed: u64) -> Result<ti4_selfplay::spec::GameSpec, String> {
    match setup {
        "mcts" => Ok(ti4_engine_search::mcts_spec(seed)),
        "quick" => Ok(ti4_engine_search::quick_spec()),
        other => {
            let parts: Vec<&str> = other.split(':').collect();
            match parts.as_slice() {
                ["lobby", faction, players] => Ok(ti4_engine_search::lobby_spec(faction, players.parse().map_err(|_| "players")?, seed)),
                _ => Err(format!("setup {other}: use mcts, quick or lobby:FACTION:PLAYERS")),
            }
        }
    }
}

fn agent(name: &str, seed: u64, seat: usize) -> Result<Box<dyn SeatAgent>, String> {
    match name {
        "rule" => Ok(Box::new(RuleBotAgent::seeing())),
        "blind" => Ok(Box::new(RuleBotAgent::blind())),
        "random" => Ok(Box::new(RandomAgent(SearchRng::at(seed, &[seat as u64, 0x7a4d])))),
        other => Err(format!("agent {other}: use rule, blind or random")),
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let setup = arg(&args, "--setup").unwrap_or_else(|| "mcts".to_owned());
    let games: u64 = arg(&args, "--games").map_or(Ok(1), |s| s.parse()).map_err(|_| "--games")?;
    let first: u64 = arg(&args, "--seed").map_or(Ok(1), |s| s.parse()).map_err(|_| "--seed")?;
    let names: Vec<String> = arg(&args, "--agents").unwrap_or_else(|| "rule".to_owned()).split(',').map(str::to_owned).collect();
    let max_rounds: u32 = arg(&args, "--max-rounds").map_or(Ok(30), |s| s.parse()).map_err(|_| "--max-rounds")?;
    let mut games_out = arg(&args, "--games-out").map(|p| std::fs::File::create(p).map_err(|e| e.to_string())).transpose()?;
    let (mut wins, mut finished, mut errors) = (Vec::<u64>::new(), 0u64, 0u64);
    let t_all = Instant::now();
    for seed in first..first + games {
        let spec = spec(&setup, seed)?;
        let mut pos = ti4_engine_search::start(&spec)?;
        let seats = pos.table().seats.len();
        wins.resize(seats.max(wins.len()), 0);
        let mut agents: Vec<Box<dyn SeatAgent>> = (0..seats).map(|s| agent(names.get(s).or(names.last()).map_or("rule", String::as_str), seed, s)).collect::<Result<_, _>>()?;
        let t = Instant::now();
        let ended = driver::play(&mut pos, &mut agents, Limits { max_rounds, ..Limits::default() }, false, |_, _| {});
        let wall = t.elapsed().as_secs_f64() * 1000.0;
        if let Some(w) = ended.outcome.winner {
            wins[w] += 1;
        }
        finished += u64::from(ended.outcome.finished);
        errors += u64::from(ended.error.is_some());
        let line = serde_json::json!({
            "seed": seed, "setup": setup, "factions": pos.table().factions, "agents": names,
            "finished": ended.outcome.finished, "round": ended.outcome.round, "vp": ended.outcome.vp, "winner": ended.outcome.winner,
            "steps": ended.steps, "decisions": ended.decisions, "error": ended.error, "wall_ms": wall,
        });
        println!("{line}");
        if let Some(f) = games_out.as_mut() {
            writeln!(f, "{line}").map_err(|e| e.to_string())?;
        }
    }
    let summary = serde_json::json!({ "setup": setup, "games": games, "first_seed": first, "agents": names, "finished": finished, "errors": errors, "wins_by_seat": wins, "wall_s": t_all.elapsed().as_secs_f64() });
    eprintln!("{summary}");
    if let Some(p) = arg(&args, "--out") {
        std::fs::write(p, summary.to_string()).map_err(|e| e.to_string())?;
    }
    Ok(())
}
