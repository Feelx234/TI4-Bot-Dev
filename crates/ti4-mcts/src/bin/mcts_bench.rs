//! Micro-bench of the MCTS core (docs/L6_RESUME.md): simulations/s per evaluator.
//!
//! ```text
//! mcts_bench [--games N] [--every K] [--sims S] [--eval uniform,rule,rollout:40] [--puct C]
//! ```
//!
//! Rule-bot games of the MCTS setup (seeds 1..=N, the rule bot with its view at every seat); at every K-th tree question with 2+
//! candidates a fresh search of S simulations (no reuse, no playout cap, no halving of the kinds) by an `MctsAgent` with each evaluator.
//! Reports simulations/s, the time per simulation and its parts (evaluator, engine: fork + answer + segment), nodes per search and the
//! live heap a tree keeps per node.

use std::time::Instant;

use ti4_engine_search::advance::default_tree;
use ti4_engine_search::agent::{RuleBotAgent, SeatAgent};
use ti4_engine_search::{Node, mcts_spec, start};
use ti4_mcts::canon::candidates;
use ti4_mcts::play::EvalSpec;
use ti4_mcts::{Config, MctsAgent, NonRoot, Stats};

#[global_allocator]
static GLOBAL: ti4_perf_alloc::Counting = ti4_perf_alloc::Counting;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let games: u64 = arg(&args, "--games").map_or(Ok(2), |v| v.parse()).map_err(|e| format!("--games: {e}"))?;
    let every: usize = arg(&args, "--every").map_or(Ok(4), |v| v.parse()).map_err(|e| format!("--every: {e}"))?;
    let sims: usize = arg(&args, "--sims").map_or(Ok(32), |v| v.parse()).map_err(|e| format!("--sims: {e}"))?;
    let evals = arg(&args, "--eval").unwrap_or_else(|| "uniform,rule,rollout:40".to_owned());
    let non_root = match arg(&args, "--puct") {
        Some(c) => NonRoot::Puct { c: c.parse().map_err(|e| format!("--puct: {e}"))? },
        None => NonRoot::Gumbel,
    };
    println!("mcts_bench: {games} games, every {every}th tree question, {sims} simulations per search, non-root {non_root:?}");
    for spec in evals.split(',') {
        let eval = EvalSpec::parse(spec)?;
        let cfg = Config { sims_full: sims, sims_fast: sims, full_fraction: 1.0, half_kinds: false, non_root, ..Config::default() };
        let mut agent = MctsAgent::new(cfg, eval.build());
        let mut stats = Stats::default();
        let (mut searches, mut nodes, mut bytes, mut positions) = (0u64, 0u64, 0u64, 0u64);
        let t0 = Instant::now();
        for seed in 1..=games {
            let mut pos = start(&mcts_spec(seed))?;
            let mut bot = RuleBotAgent::seeing();
            let mut k = 0usize;
            while let Node::Decide(choice) = pos.node() {
                let choice = choice.clone();
                if default_tree(&pos, &choice) {
                    let cands = candidates(&pos, &choice, None);
                    if cands.len() >= 2 {
                        k += 1;
                        if k.is_multiple_of(every) {
                            agent.clear_tree();
                            let live0 = ti4_perf_alloc::live();
                            let r = agent.search(&pos, &choice, cands, None)?;
                            let live1 = ti4_perf_alloc::live();
                            let (n, p) = agent.kept_tree().unwrap_or((0, 0));
                            agent.clear_tree();
                            stats.add(&r.stats);
                            searches += 1;
                            nodes += n as u64;
                            positions += p as u64;
                            bytes += live1.saturating_sub(live0);
                        }
                    }
                }
                let d = bot.decide(&pos, &choice)?;
                pos.apply(d).map_err(|e| e.to_string())?;
            }
        }
        let wall = t0.elapsed().as_secs_f64();
        let per_sim_us = stats.wall_ns as f64 / 1e3 / stats.sims.max(1) as f64;
        println!(
            "  {:<16} {searches:>5} searches, {:>7} sims: {:>7.0} sims/s, {:>7.0} us/sim (evaluator {:>6.0} us, engine {:>6.0} us); \
             {:.1} nodes/search ({:.1} positions), depth max {}, chance nodes {}, terminals {}; live heap {:.1} KB/node; delegated {:.1}/sim; total {wall:.1} s",
            agent.eval.name(),
            stats.sims,
            stats.sims_per_s(),
            per_sim_us,
            stats.eval_ns as f64 / 1e3 / stats.sims.max(1) as f64,
            stats.engine_ns as f64 / 1e3 / stats.sims.max(1) as f64,
            nodes as f64 / searches.max(1) as f64,
            positions as f64 / searches.max(1) as f64,
            stats.max_depth,
            stats.chance_nodes,
            stats.terminals,
            bytes as f64 / 1024.0 / nodes.max(1) as f64,
            stats.delegated as f64 / stats.sims.max(1) as f64,
        );
    }
    Ok(())
}
