//! Running many games on worker threads, and the A/B comparison.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use ti4_rule_bot::tuning::Tuning;

use crate::play::{GameResult, PlayOptions, TraceLine, play_guarded};
use crate::spec::{GameSpec, Limits};

/// One game to play.
pub struct Job {
    pub spec: GameSpec,
    pub options: PlayOptions,
}

/// Called on the worker thread when a game is done (to write a trace, for instance).
pub type OnDone<'a> = &'a (dyn Fn(&GameResult, Option<&[TraceLine]>) + Sync);

/// Play `jobs` on `workers` threads. Results come back in job order, whatever the order of
/// completion, so a run is the same list on any number of workers.
pub fn run_jobs(
    jobs: &[Job],
    workers: usize,
    progress: bool,
    on_done: Option<OnDone<'_>>,
) -> Vec<GameResult> {
    crate::play::install_panic_capture();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let slots: Mutex<Vec<Option<GameResult>>> = Mutex::new((0..jobs.len()).map(|_| None).collect());
    let started = Instant::now();
    let workers = workers.clamp(1, jobs.len().max(1));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            std::thread::Builder::new()
                .stack_size(128 << 20)
                .spawn_scoped(scope, || {
                    loop {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        let Some(job) = jobs.get(i) else { break };
                        let (result, trace) = play_guarded(&job.spec, &job.options);
                        if let Some(f) = on_done {
                            f(&result, trace.as_deref());
                        }
                        slots.lock().expect("slots")[i] = Some(result);
                        let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                        if progress && (n % 10 == 0 || n == jobs.len()) {
                            let secs = started.elapsed().as_secs_f64();
                            eprintln!(
                                "  {n}/{} games, {:.0} s, eta {:.0} s",
                                jobs.len(),
                                secs,
                                secs / n as f64 * (jobs.len() - n) as f64
                            );
                        }
                    }
                })
                .expect("a worker thread starts");
        }
    });
    slots
        .into_inner()
        .expect("slots")
        .into_iter()
        .map(|r| r.expect("every job ran"))
        .collect()
}

// -- A/B -------------------------------------------------------------------------------------

/// Mean and 95% interval of a paired difference.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Estimate {
    pub n: usize,
    pub mean: f64,
    pub se: f64,
    pub ci95: [f64; 2],
}

#[must_use]
pub fn estimate(values: &[f64]) -> Estimate {
    let n = values.len();
    if n == 0 {
        return Estimate::default();
    }
    let mean = values.iter().sum::<f64>() / n as f64;
    let var = if n > 1 {
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64
    } else {
        0.0
    };
    let se = (var / n as f64).sqrt();
    Estimate {
        n,
        mean,
        se,
        ci95: [mean - 1.96 * se, mean + 1.96 * se],
    }
}

/// The result of an A/B run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comparison {
    pub mode: String,
    pub baseline: std::collections::BTreeMap<String, String>,
    pub candidate: std::collections::BTreeMap<String, String>,
    pub tables: usize,
    /// Tables dropped because a game of the pair did not end in a victory.
    pub excluded: usize,
    /// Mixed mode: share of games won by a candidate seat, per pair of swapped games, minus the
    /// 50% that equal bots would get. Positive: the candidate is stronger.
    pub win_share_advantage: Estimate,
    /// The candidate's wins over its games, pooled, with a Wilson interval.
    pub candidate_win_rate: f64,
    pub candidate_win_rate_ci95: [f64; 2],
    /// Mixed mode: mean final points of the candidate's seats minus the baseline's.
    pub vp_advantage: Estimate,
    /// Both modes: paired differences (candidate minus baseline) of game length.
    pub rounds_diff: Estimate,
    pub decisions_diff: Estimate,
    pub wall_ms_diff: Estimate,
    pub finished_rate_baseline: f64,
    pub finished_rate_candidate: f64,
    /// `candidate` stronger / weaker / not distinguishable at 95%.
    pub verdict: String,
    pub wall_s: f64,
}

fn seats_mask(n: usize, shift: usize) -> Vec<bool> {
    (0..n).map(|i| (i + shift) % 2 == 0).collect()
}

fn tunings_for(mask: &[bool], baseline: Tuning, candidate: Tuning) -> Vec<Tuning> {
    mask.iter()
        .map(|&c| if c { candidate } else { baseline })
        .collect()
}

/// Run the comparison on `specs`.
///
/// `mixed`: each table is played twice with the same seed, map and factions; in the first game
/// the even seats play the candidate and the odd seats the baseline, in the second they swap.
/// A candidate seat wins when the winner is one of its seats. `separate`: the table is played
/// once with the baseline on every seat and once with the candidate on every seat, and the
/// games are compared as to length and how often they end.
pub fn compare(
    specs: &[GameSpec],
    limits: &Limits,
    baseline: Tuning,
    candidate: Tuning,
    mixed: bool,
    workers: usize,
    progress: bool,
) -> (Comparison, Vec<GameResult>) {
    let started = Instant::now();
    let mut jobs = Vec::new();
    for spec in specs {
        let n = spec.players;
        for shift in 0..2 {
            let (mask, tunings) = if mixed {
                let mask = seats_mask(n, shift);
                (Some(mask.clone()), tunings_for(&mask, baseline, candidate))
            } else {
                let all = shift == 1;
                (
                    Some(vec![all; n]),
                    vec![if all { candidate } else { baseline }],
                )
            };
            jobs.push(Job {
                spec: spec.clone(),
                options: PlayOptions {
                    limits: limits.clone(),
                    tunings,
                    candidate_seats: mask,
                    trace: false,
                },
            });
        }
    }
    let results = run_jobs(&jobs, workers, progress, None);
    let (mut win_share, mut vp_adv, mut rounds, mut decisions, mut wall) =
        (vec![], vec![], vec![], vec![], vec![]);
    let (mut excluded, mut cand_games, mut cand_wins) = (0usize, 0usize, 0usize);
    let (mut base_done, mut cand_done) = (0usize, 0usize);
    for pair in results.chunks(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let ok = |g: &GameResult| g.termination == "victory";
        if !mixed {
            base_done += usize::from(ok(a));
            cand_done += usize::from(ok(b));
        }
        if !(ok(a) && ok(b)) {
            excluded += 1;
            continue;
        }
        rounds.push(f64::from(b.rounds) - f64::from(a.rounds));
        decisions.push(b.decisions as f64 - a.decisions as f64);
        wall.push(b.wall_ms - a.wall_ms);
        if mixed {
            let mut wins = 0.0;
            let mut vp = 0.0;
            for g in [a, b] {
                let mask = g
                    .candidate_seats
                    .as_ref()
                    .expect("mixed games carry the mask");
                let winner_is_cand = g.winner.is_some_and(|w| mask[w]);
                wins += f64::from(u8::from(winner_is_cand));
                cand_games += 1;
                cand_wins += usize::from(winner_is_cand);
                let mean = |want: bool| {
                    let v: Vec<f64> =
                        g.vp.iter()
                            .zip(mask)
                            .filter(|(_, c)| **c == want)
                            .map(|(v, _)| f64::from(*v))
                            .collect();
                    v.iter().sum::<f64>() / v.len().max(1) as f64
                };
                vp += mean(true) - mean(false);
            }
            win_share.push(wins / 2.0 - 0.5);
            vp_adv.push(vp / 2.0);
        }
    }
    let n = specs.len();
    let (lo, hi) = crate::stats::wilson(cand_wins, cand_games);
    let adv = estimate(&win_share);
    let verdict = if !mixed {
        "separate mode: compare the length and finish figures; strength needs mixed mode".to_owned()
    } else if adv.n == 0 {
        "no finished pairs".to_owned()
    } else if adv.ci95[0] > 0.0 {
        "candidate stronger (95%)".to_owned()
    } else if adv.ci95[1] < 0.0 {
        "candidate weaker (95%)".to_owned()
    } else {
        "no significant difference at 95%".to_owned()
    };
    let comparison = Comparison {
        mode: if mixed { "mixed" } else { "separate" }.to_owned(),
        baseline: baseline_diff(baseline),
        candidate: baseline_diff(candidate),
        tables: n,
        excluded,
        win_share_advantage: adv,
        candidate_win_rate: if cand_games == 0 {
            0.0
        } else {
            cand_wins as f64 / cand_games as f64
        },
        candidate_win_rate_ci95: [lo, hi],
        vp_advantage: estimate(&vp_adv),
        rounds_diff: estimate(&rounds),
        decisions_diff: estimate(&decisions),
        wall_ms_diff: estimate(&wall),
        finished_rate_baseline: if mixed {
            0.0
        } else {
            base_done as f64 / n.max(1) as f64
        },
        finished_rate_candidate: if mixed {
            0.0
        } else {
            cand_done as f64 / n.max(1) as f64
        },
        verdict,
        wall_s: started.elapsed().as_secs_f64(),
    };
    (comparison, results)
}

/// The numbers of `t` that differ from the shipped bot.
#[must_use]
pub fn baseline_diff(t: Tuning) -> std::collections::BTreeMap<String, String> {
    let default = Tuning::default().entries();
    t.entries()
        .into_iter()
        .zip(default)
        .filter(|(a, b)| a.1 != b.1)
        .map(|(a, _)| (a.0.to_owned(), a.1))
        .collect()
}

/// Text of a comparison.
#[must_use]
pub fn render_comparison(c: &Comparison) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "== A/B ({}) : baseline {:?} vs candidate {:?} ==",
        c.mode, c.baseline, c.candidate
    );
    let _ = writeln!(
        out,
        "tables {} (pairs excluded: {}), {:.1} s",
        c.tables, c.excluded, c.wall_s
    );
    let row = |out: &mut String, name: &str, e: &Estimate, scale: f64, unit: &str| {
        let _ = writeln!(
            out,
            "  {:<34} {:+.3}{unit}  95% CI [{:+.3}, {:+.3}]  (n={})",
            name,
            e.mean * scale,
            e.ci95[0] * scale,
            e.ci95[1] * scale,
            e.n
        );
    };
    if c.mode == "mixed" {
        row(
            &mut out,
            "win share vs 50% (candidate)",
            &c.win_share_advantage,
            100.0,
            " pp",
        );
        let _ = writeln!(
            out,
            "  candidate seats won {:.1}% of games [{:.1}, {:.1}] (equal bots: ~50%)",
            100.0 * c.candidate_win_rate,
            100.0 * c.candidate_win_rate_ci95[0],
            100.0 * c.candidate_win_rate_ci95[1]
        );
        row(
            &mut out,
            "VP per seat, candidate - baseline",
            &c.vp_advantage,
            1.0,
            " VP",
        );
    } else {
        let _ = writeln!(
            out,
            "  games finished: baseline {:.1}%  candidate {:.1}%",
            100.0 * c.finished_rate_baseline,
            100.0 * c.finished_rate_candidate
        );
    }
    row(
        &mut out,
        "rounds, candidate - baseline",
        &c.rounds_diff,
        1.0,
        "",
    );
    row(
        &mut out,
        "decisions, candidate - baseline",
        &c.decisions_diff,
        1.0,
        "",
    );
    row(
        &mut out,
        "wall ms, candidate - baseline",
        &c.wall_ms_diff,
        1.0,
        " ms",
    );
    let _ = writeln!(out, "verdict: {}", c.verdict);
    out
}
