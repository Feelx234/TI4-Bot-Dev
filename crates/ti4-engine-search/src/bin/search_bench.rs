//! Micro-bench of the search API (docs/C6_RESUME.md): positions/s, forks/s, bytes per fork, the ledger (lane C7).
//!
//! ```text
//! search_bench [--seed S] [--sims K] [--games N]
//! ```
//!
//! 1. `game`: the quick game through the API with the rule bot's random pick (the product's quick game): steps/s, answers/s.
//! 2. `fork`: at every question of that game: the time of `Position::fork` (median of 16), the bytes it allocates, the bytes the first
//!    answer after the fork allocates against the same answer without a fork, with and without the C7 ledger trim; the ledger's size.
//! 3. `sims`: games of the MCTS setup (rule bot with its view at every seat): at every tree decision `K` simulations, each a
//!    `fork_search` + `advance_to_tree_decision` with the rule bot delegate (one segment, as M0 timed it): simulations/s, answers/s,
//!    and the live heap of `K` kept forks.

use std::time::{Duration, Instant};

use ti4_engine_search::advance::{Halt, Rules, advance_to_tree_decision, default_tree};
use ti4_engine_search::agent::{RuleBotAgent, SeatAgent};
use ti4_engine_search::chance::SearchRng;
use ti4_engine_search::{Node, Position};

#[global_allocator]
static GLOBAL: ti4_perf_alloc::Counting = ti4_perf_alloc::Counting;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn bytes() -> u64 {
    ti4_perf_alloc::counts().1
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
}

/// Answer the question with the blind rule bot (the quick game's bot).
fn bot_answer(pos: &mut Position) -> Result<(), String> {
    let mut bot = RuleBotAgent::blind();
    let choice = pos.choice().cloned().ok_or("nothing asked")?;
    let d = bot.decide(pos, &choice)?;
    pos.apply(d).map_err(|e| e.to_string())
}

fn game() -> Result<(), String> {
    let mut pos = ti4_engine_search::start(&ti4_engine_search::quick_spec())?;
    let mut agents: Vec<Box<dyn SeatAgent>> = (0..4).map(|_| Box::new(RuleBotAgent::blind()) as Box<dyn SeatAgent>).collect();
    let t = Instant::now();
    let ended = ti4_engine_search::driver::play(&mut pos, &mut agents, ti4_engine_search::driver::Limits::default(), false, |_, _| {});
    let s = t.elapsed().as_secs_f64();
    println!(
        "game: quick game {} steps, {} answers, {:.0} ms: {:.0} steps/s, {:.0} answers/s (the blind rule bot answering; error {:?})",
        ended.steps,
        ended.decisions,
        s * 1000.0,
        ended.steps as f64 / s,
        ended.decisions as f64 / s,
        ended.error
    );
    Ok(())
}

fn fork() -> Result<(), String> {
    let mut pos = ti4_engine_search::start(&ti4_engine_search::quick_spec())?;
    let (mut t_fork, mut b_fork, mut b_own, mut b_after, mut b_after_trim, mut t_trim) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut parts: [Vec<f64>; 4] = Default::default();
    let mut t_parts: [Vec<f64>; 4] = Default::default();
    let mut n = 0usize;
    let mut ledger = [0usize; 4];
    let mut lens = [0usize; 4];
    while let Node::Decide(_) = pos.node() {
        n += 1;
        // The time of a fork, and of each of its parts.
        let mut times = Vec::with_capacity(16);
        for _ in 0..16 {
            let t = Instant::now();
            let f = pos.fork();
            times.push(t.elapsed().as_secs_f64() * 1e6);
            drop(f);
        }
        t_fork.push(median(times));
        for (part, (bytes_of, time_of)) in parts.iter_mut().zip(t_parts.iter_mut()).enumerate() {
            let b0 = bytes();
            pos.clone_part_for_bench(part as u8);
            bytes_of.push((bytes() - b0) as f64);
            let t = Instant::now();
            for _ in 0..8 {
                pos.clone_part_for_bench(part as u8);
            }
            time_of.push(t.elapsed().as_secs_f64() * 1e6 / 8.0);
        }
        // The bytes of one fork.
        let b0 = bytes();
        let f = pos.fork();
        b_fork.push((bytes() - b0) as f64);
        // The first answer of a fork (the original alive: the log tails it shares are copied on write) ...
        let mut g = f;
        let b0 = bytes();
        bot_answer(&mut g)?;
        b_after.push((bytes() - b0) as f64);
        drop(g);
        // ... of a fork with its ledgers trimmed first ...
        let mut h = pos.fork();
        let t = Instant::now();
        h.trim_ledger();
        t_trim.push(t.elapsed().as_secs_f64() * 1e6);
        let b0 = bytes();
        bot_answer(&mut h)?;
        b_after_trim.push((bytes() - b0) as f64);
        drop(h);
        ledger = pos.ledger_bytes();
        lens = pos.ledger_lens();
        // ... and of the original itself, no copy alive (its own tails, nothing copied).
        let b0 = bytes();
        bot_answer(&mut pos)?;
        b_own.push((bytes() - b0) as f64);
    }
    println!("fork: {n} questions of the quick game");
    println!("  Position::fork            median {:.2} us (mean of the medians {:.2} us): {:.0} forks/s", median(t_fork.clone()), mean(&t_fork), 1e6 / mean(&t_fork));
    println!("  bytes allocated per fork  mean {:.0} B, median {:.0} B", mean(&b_fork), median(b_fork.clone()));
    for (i, name) in ["core clone", "renderer warm copy", "(renderer cold, for comparison)", "step data + question (Rc)"].iter().enumerate() {
        println!("    {name:<32} mean {:>7.0} B, {:>6.2} us", mean(&parts[i]), mean(&t_parts[i]));
    }
    println!("  first answer, bytes allocated: the original alone {:.0} B; a fork {:.0} B; a fork after trim_ledger {:.0} B (trim {:.2} us)", mean(&b_own), mean(&b_after), mean(&b_after_trim), median(t_trim));
    println!("  ledger at the end: events {} ({} B inline), dice {} ({} B), vp {} ({} B), relics {} ({} B)", lens[0], ledger[0], lens[1], ledger[1], lens[2], ledger[2], lens[3], ledger[3]);
    Ok(())
}

fn sims(games: u64, first: u64, k: usize) -> Result<(), String> {
    let rules = Rules::default();
    let (mut sims, mut answers, mut nodes) = (0usize, 0usize, 0usize);
    let (mut t_sims, mut t_game) = (Duration::ZERO, Duration::ZERO);
    let mut heap = Vec::new();
    let mut halts = [0usize; 3];
    for seed in first..first + games {
        let mut pos = ti4_engine_search::start(&ti4_engine_search::mcts_spec(seed))?;
        let mut bot = RuleBotAgent::seeing();
        let t_all = Instant::now();
        let mut in_sims = Duration::ZERO;
        loop {
            let Node::Decide(choice) = pos.node() else { break };
            let choice = choice.clone();
            if default_tree(&pos, &choice) {
                nodes += 1;
                let t = Instant::now();
                let live0 = ti4_perf_alloc::live();
                let mut kept = Vec::with_capacity(k);
                for i in 0..k {
                    let mut f = pos.fork_search();
                    let mut rng = SearchRng::at(seed, &[pos.decisions() as u64, i as u64]);
                    let mut delegate = RuleBotAgent { view: true, seed: Some(seed ^ 0x9e37), index: None };
                    // The tree's own answer: a uniform legal one (a stand-in for the net).
                    let pick = rng.below(f.legal().len());
                    f.answer_index(pick).map_err(|e| e.to_string())?;
                    let (halt, walked) = advance_to_tree_decision(&mut f, &mut delegate, &mut rng, &rules).map_err(|e| e.to_string())?;
                    halts[match halt {
                        Halt::Tree => 0,
                        Halt::Chance(_) => 1,
                        Halt::End => 2,
                    }] += 1;
                    answers += 1 + walked.delegated + walked.single + walked.sampled;
                    sims += 1;
                    kept.push(f);
                }
                heap.push((ti4_perf_alloc::live().saturating_sub(live0)) as f64 / k as f64);
                drop(kept);
                in_sims += t.elapsed();
            }
            let d = bot.decide(&pos, &choice)?;
            pos.apply(d).map_err(|e| e.to_string())?;
        }
        t_sims += in_sims;
        t_game += t_all.elapsed() - in_sims;
        println!("  seed {seed}: {:?}, {} steps", pos.outcome(), pos.steps());
    }
    let s = t_sims.as_secs_f64();
    println!(
        "sims: {games} games, {nodes} tree decisions x {k}: {sims} simulations in {:.1} s = {:.0} sims/s ({:.2} ms each), {:.0} answers/s inside; halts tree/chance/end {:?}",
        s,
        sims as f64 / s,
        s * 1000.0 / sims.max(1) as f64,
        answers as f64 / s,
        halts
    );
    println!("  the game itself (rule bot with its view at every seat): {:.1} s", t_game.as_secs_f64());
    println!("  live heap kept per simulated fork (one segment played): mean {:.0} B, median {:.0} B", mean(&heap), median(heap.clone()));
    Ok(())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = arg(&args, "--seed").map_or(Ok(1), |s| s.parse()).map_err(|_| "--seed")?;
    let games: u64 = arg(&args, "--games").map_or(Ok(1), |s| s.parse()).map_err(|_| "--games")?;
    let k: usize = arg(&args, "--sims").map_or(Ok(8), |s| s.parse()).map_err(|_| "--sims")?;
    let what = arg(&args, "--only");
    if what.as_deref().is_none_or(|w| w == "game") {
        game()?;
    }
    if what.as_deref().is_none_or(|w| w == "fork") {
        fork()?;
    }
    if what.as_deref().is_none_or(|w| w == "sims") {
        sims(games, seed, k)?;
    }
    Ok(())
}
