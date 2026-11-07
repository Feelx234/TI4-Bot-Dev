//! A/B smoke of an MCTS seat against the rule bot (docs/L6_RESUME.md). Plumbing, not a strength measure (that is lane L7's
//! `mcts_eval` with hundreds of games and the Wilson / paired intervals).
//!
//! ```text
//! mcts_ab [--first S] [--seeds N] [--eval rule|rule-flat|uniform|rollout:H|rollout-rule:H] [--full F] [--fast K] [--gumbel G] [--rescale 0|1]
//!         [--out FILE.json]
//! ```
//!
//! Per seed: the all-rule-bot game (the baseline) and the game with an MCTS seat at place `seed % 4`; the win of that seat and its VP
//! in both. Prints the totals; `--out` writes them as JSON.

use ti4_engine_search::driver::Limits;
use ti4_mcts::Config;
use ti4_mcts::play::{EvalSpec, play_game};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn num<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> Result<T, String> {
    arg(args, name).map_or(Ok(default), |v| v.parse().map_err(|_| format!("{name}: {v}")))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let first: u64 = num(&args, "--first", 101)?;
    let seeds: u64 = num(&args, "--seeds", 4)?;
    let eval = EvalSpec::parse(&arg(&args, "--eval").unwrap_or_else(|| "rule".to_owned()))?;
    let cfg = Config {
        sims_full: num(&args, "--full", 8)?,
        sims_fast: num(&args, "--fast", 2)?,
        gumbel_scale: num(&args, "--gumbel", 1.0)?,
        rescale_q: num(&args, "--rescale", 1u8)? != 0,
        ..Config::default()
    };
    let (mut wins, mut base_wins, mut vp, mut base_vp, mut secs, mut base_secs, mut sims, mut searches) = (0u32, 0u32, 0i64, 0i64, 0.0, 0.0, 0u64, 0u64);
    let mut rows = Vec::new();
    let mut identical = 0u32;
    for seed in first..first + seeds {
        let seat = u8::try_from(seed % 4).unwrap_or(0);
        let base = play_game(seed, &[], &cfg, eval, Limits::default())?;
        let ab = play_game(seed, &[seat], &cfg, eval, Limits::default())?;
        if let Some(e) = base.error.as_ref().or(ab.error.as_ref()) {
            return Err(format!("seed {seed}: {e}"));
        }
        let s = usize::from(seat);
        let (w, bw) = (ab.outcome.winner == Some(s), base.outcome.winner == Some(s));
        wins += u32::from(w);
        base_wins += u32::from(bw);
        vp += i64::from(ab.outcome.vp[s]);
        base_vp += i64::from(base.outcome.vp[s]);
        secs += ab.secs;
        base_secs += base.secs;
        sims += ab.stats.sims;
        searches += ab.counts.searched;
        println!(
            "seed {seed} seat {seat}: mcts {} vp {} (round {}), rule bot {} vp {} (round {}); {} searches, {} sims, {:.1} s",
            if w { "WIN " } else { "lose" },
            ab.outcome.vp[s],
            ab.outcome.round,
            if bw { "WIN " } else { "lose" },
            base.outcome.vp[s],
            base.outcome.round,
            ab.counts.searched,
            ab.stats.sims,
            ab.secs
        );
        // (The records differ in form: the MCTS seat answers by pick, the rule bot by option id.)
        let same = ab.outcome == base.outcome && ab.steps == base.steps && ab.decisions == base.decisions;
        identical += u32::from(same);
        rows.push(serde_json::json!({"identical": same, "seed": seed, "seat": seat, "mcts_win": w, "base_win": bw, "mcts_vp": ab.outcome.vp[s], "base_vp": base.outcome.vp[s], "searches": ab.counts.searched, "sims": ab.stats.sims, "secs": ab.secs}));
    }
    let n = seeds.max(1) as f64;
    println!(
        "A/B smoke, {} ({} / {} sims), {seeds} seeds: MCTS seat wins {wins} ({:.0} %), the rule bot in that seat {base_wins} ({:.0} %); mean VP {:.2} vs {:.2}; {searches} searches, {sims} sims; {:.1} s vs {:.1} s per game; {identical} games identical to the baseline",
        eval.build().name(),
        cfg.sims_full,
        cfg.sims_fast,
        100.0 * f64::from(wins) / n,
        100.0 * f64::from(base_wins) / n,
        vp as f64 / n,
        base_vp as f64 / n,
        secs / n,
        base_secs / n
    );
    if let Some(out) = arg(&args, "--out") {
        let doc = serde_json::json!({"eval": eval.build().name(), "sims_full": cfg.sims_full, "sims_fast": cfg.sims_fast, "seeds": seeds, "first": first, "mcts_wins": wins, "base_wins": base_wins, "mcts_vp": vp, "base_vp": base_vp, "games": rows});
        std::fs::write(&out, serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?).map_err(|e| format!("{out}: {e}"))?;
    }
    Ok(())
}
