//! The evaluation harness (docs/MCTS_BOT_PLAN.md 4.3, lane L7).
//!
//! ```text
//! mcts_eval --agent policy:W.t4w [--mode bench] [--seeds 24] [--first 900000] [--workers 4] [--out report.json]
//!           [--sprt 0.25,0.30] [--agreement]
//! mcts_eval --mode gate --agent policy:CAND.t4w --vs policy:BEST.t4w [--sprt 0.5,0.55] ...
//! ```
//!
//! * **bench** (one agent against three rule bots): every seed `s` (seating `s % 24`, so a multiple of 24 seeds covers all 24
//!   seatings equally) is played 5 ways: the **paired baseline** (four rule bots) and the agent at each of the 4 seat places.
//!   The agent's game at place `p` is paired with the baseline's rule-bot seat at `p` (same seed, same faction, same dice
//!   stream at the start): the paired Δ win share and Δ VP remove most of the seat and faction variance.
//! * **gate** (candidate `--agent` vs best `--vs`): every seed is played twice, the candidate at places {0, 2} and the best at
//!   {1, 3}, then swapped. The observation per game is the candidate seats' share of the win.
//! * Win share with Wilson 95 % intervals (overall and per faction of the agent's seat), mean VP, VP margin over the best rival,
//!   and an SPRT (`--sprt p0,p1`, alpha = beta = 0.05) over the agent's games in seed order. Games ended by the round cap count
//!   with their victory points (most VP wins, ties split); errors are counted and left out.
//! * `--agreement`: at the net's asks also ask the rule bot (top-1 agreement in play).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use serde_json::{json, Value};
use ti4_mcts_lab::agents::{Agent, NetStats, Table4};
use ti4_mcts_lab::eval::{mean_ci, wilson, Sprt, Z95};
use ti4_mcts_lab::{outcome, start};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Clone, Copy, Debug)]
enum Kind {
    /// Four rule bots.
    Baseline,
    /// The agent at this place, rule bots elsewhere.
    At(usize),
    /// Gate: the candidate at {0, 2} (`false`) or at {1, 3} (`true`).
    Gate(bool),
}

#[derive(Clone, Debug)]
struct Job {
    seed: u64,
    kind: Kind,
}

#[derive(Clone, Debug, Default)]
struct Result1 {
    seed: u64,
    kind: Option<Kind>,
    places: Vec<usize>,
    factions: Vec<String>,
    z: Vec<f32>,
    vp: Vec<f32>,
    finished: bool,
    round: u32,
    error: Option<String>,
    net: NetStats,
    seconds: f64,
}

fn play(job: &Job, a: &Agent, b: &Agent, agreement: bool) -> Result1 {
    let t0 = Instant::now();
    let rule = Agent::Rule;
    let (agents, places): (Vec<Agent>, Vec<usize>) = match job.kind {
        Kind::Baseline => (vec![rule.clone(), rule.clone(), rule.clone(), rule], vec![]),
        Kind::At(p) => ((0..4).map(|i| if i == p { a.clone() } else { Agent::Rule }).collect(), vec![p]),
        Kind::Gate(swap) => ((0..4).map(|i| if (i % 2 == 0) != swap { a.clone() } else { b.clone() }).collect(), if swap { vec![1, 3] } else { vec![0, 2] }),
    };
    let mut r = Result1 { seed: job.seed, kind: Some(job.kind), places, ..Result1::default() };
    let (t, mut pos) = match start(&ti4_mcts_lab::spec(job.seed)) {
        Ok(x) => x,
        Err(e) => {
            r.error = Some(e);
            return r;
        }
    };
    let mut dec = Table4::new(agents, job.seed);
    dec.agreement = agreement;
    let end = ti4_mcts_lab::play(&t, &mut pos, &mut dec, 6000, 12, |_, _| {});
    r.factions.clone_from(&t.factions);
    r.error = end.error.clone();
    r.finished = end.finished;
    r.round = end.round;
    let (z, vp) = outcome(&end.vp);
    r.z = z;
    r.vp = vp;
    r.net = dec.stats;
    r.seconds = t0.elapsed().as_secs_f64();
    r
}

fn share(wins: f64, n: f64) -> Value {
    let (lo, hi) = wilson(wins, n, Z95);
    json!({"share": if n > 0.0 { wins / n } else { 0.0 }, "wilson95": [lo, hi], "games": n})
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let num = |flag: &str, d: u64| get(flag).and_then(|v| v.parse().ok()).unwrap_or(d);
    let mode = get("--mode").unwrap_or_else(|| "bench".to_owned());
    let agent_text = get("--agent").unwrap_or_else(|| "rule".to_owned());
    let vs_text = get("--vs").unwrap_or_else(|| "rule".to_owned());
    let parse = |t: &str| {
        Agent::parse(t).unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(2)
        })
    };
    let (a, b) = (parse(&agent_text), parse(&vs_text));
    let seeds = num("--seeds", 24);
    let first = num("--first", 900_000);
    let workers = num("--workers", 4).max(1) as usize;
    let agreement = args.iter().any(|x| x == "--agreement");
    let gate = mode == "gate";
    let sprt_pair = get("--sprt").map_or(if gate { (0.5, 0.55) } else { (0.25, 0.30) }, |t| {
        let v: Vec<f64> = t.split(',').filter_map(|x| x.parse().ok()).collect();
        (v.first().copied().unwrap_or(0.25), v.get(1).copied().unwrap_or(0.30))
    });
    let mut jobs = Vec::new();
    for seed in first..first + seeds {
        if gate {
            jobs.push(Job { seed, kind: Kind::Gate(false) });
            jobs.push(Job { seed, kind: Kind::Gate(true) });
        } else {
            jobs.push(Job { seed, kind: Kind::Baseline });
            for p in 0..4 {
                jobs.push(Job { seed, kind: Kind::At(p) });
            }
        }
    }
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, Result1)>> = Mutex::new(Vec::new());
    let started = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..workers {
            let (jobs, next, results, a, b) = (&jobs, &next, &results, &a, &b);
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn_scoped(s, move || loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    let r = play(job, a, b, agreement);
                    if let Ok(mut v) = results.lock() {
                        v.push((i, r));
                        if v.len() % 20 == 0 {
                            eprintln!("[{:.0} s] {} / {} games", started.elapsed().as_secs_f64(), v.len(), jobs.len());
                        }
                    }
                })
                .expect("a worker starts");
        }
    });
    let mut results = results.into_inner().unwrap_or_default();
    results.sort_by_key(|(i, _)| *i);
    let results: Vec<Result1> = results.into_iter().map(|(_, r)| r).collect();
    let errors: Vec<&Result1> = results.iter().filter(|r| r.error.is_some()).collect();
    let ok: Vec<&Result1> = results.iter().filter(|r| r.error.is_none()).collect();
    // The agent's games and observations, in seed order.
    let mut sprt = Sprt::new(sprt_pair.0, sprt_pair.1, 0.05, 0.05);
    let (mut wins, mut n, mut vp_sum, mut margin_sum) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut by_faction: BTreeMap<String, (f64, f64, f64)> = BTreeMap::new();
    let mut base_by_faction: BTreeMap<String, (f64, f64, f64)> = BTreeMap::new();
    let mut base_all = (0.0f64, 0.0f64);
    let mut by_place: BTreeMap<usize, (f64, f64)> = BTreeMap::new();
    let (mut dz, mut dvp) = (Vec::new(), Vec::new());
    let baseline: BTreeMap<u64, &Result1> = ok.iter().filter(|r| r.places.is_empty()).map(|r| (r.seed, *r)).collect();
    let mut net = NetStats::default();
    let mut capped = 0usize;
    for r in &ok {
        capped += usize::from(!r.finished);
        if r.places.is_empty() {
            for (i, f) in r.factions.iter().enumerate() {
                let e = base_by_faction.entry(f.clone()).or_default();
                e.0 += f64::from(r.z[i]);
                e.1 += 1.0;
                e.2 += f64::from(r.vp[i]);
                base_all.0 += f64::from(r.z[i]);
                base_all.1 += 1.0;
            }
            continue;
        }
        net.answered += r.net.answered;
        net.agree += r.net.agree;
        net.compared += r.net.compared;
        net.encode_ns += r.net.encode_ns;
        net.net_ns += r.net.net_ns;
        let x: f64 = r.places.iter().map(|p| f64::from(r.z[*p])).sum();
        sprt.push(x);
        wins += x;
        n += 1.0;
        for p in &r.places {
            let f = &r.factions[*p];
            let e = by_faction.entry(f.clone()).or_default();
            e.0 += f64::from(r.z[*p]);
            e.1 += 1.0;
            e.2 += f64::from(r.vp[*p]);
            vp_sum += f64::from(r.vp[*p]) / r.places.len() as f64;
            let best_rival = (0..r.vp.len()).filter(|i| !r.places.contains(i)).map(|i| r.vp[i]).fold(f32::MIN, f32::max);
            margin_sum += f64::from(r.vp[*p] - best_rival) / r.places.len() as f64;
            let bp = by_place.entry(*p).or_default();
            bp.0 += f64::from(r.z[*p]);
            bp.1 += 1.0;
        }
        if let (Some(Kind::At(p)), Some(base)) = (r.kind, baseline.get(&r.seed)) {
            dz.push(f64::from(r.z[p] - base.z[p]));
            dvp.push(f64::from(r.vp[p] - base.vp[p]));
        }
    }
    let (dz_m, dz_h) = mean_ci(&dz);
    let (dvp_m, dvp_h) = mean_ci(&dvp);
    let faction_table: Value = by_faction
        .iter()
        .map(|(f, (w, k, v))| {
            let base = base_by_faction.get(f).copied().unwrap_or_default();
            (f.clone(), json!({"agent": share(*w, *k), "agent_mean_vp": v / k.max(1.0), "rule_baseline": share(base.0, base.1), "rule_mean_vp": base.2 / base.1.max(1.0)}))
        })
        .collect::<serde_json::Map<String, Value>>()
        .into();
    let report = json!({
        "mode": mode,
        "agent": agent_text,
        "vs": if gate { vs_text.clone() } else { "rule".to_owned() },
        "seeds": [first, first + seeds],
        "seatings_covered": (seeds.min(24)),
        "games": results.len(),
        "errors": errors.len(),
        "error_samples": errors.iter().take(10).map(|r| json!({"seed": r.seed, "kind": format!("{:?}", r.kind), "error": r.error})).collect::<Vec<_>>(),
        "round_capped": capped,
        "seconds": started.elapsed().as_secs_f64(),
        "agent_games": n,
        "win_share": share(wins, n),
        "baseline_share": if gate { 0.5 } else { 0.25 },
        "mean_vp": vp_sum / n.max(1.0),
        "vp_margin_over_best_rival": margin_sum / n.max(1.0),
        "paired": {"pairs": dz.len(), "delta_win_share": dz_m, "delta_win_share_ci95": dz_h, "delta_vp": dvp_m, "delta_vp_ci95": dvp_h},
        "rule_baseline_all_seats": share(base_all.0, base_all.1),
        "by_faction": faction_table,
        "by_place": by_place.iter().map(|(p, (w, k))| (p.to_string(), share(*w, *k))).collect::<serde_json::Map<String, Value>>(),
        "sprt": {"p0": sprt.p0, "p1": sprt.p1, "alpha": sprt.alpha, "beta": sprt.beta, "llr": sprt.llr, "bounds": [sprt.bounds().0, sprt.bounds().1],
                  "verdict": sprt.verdict().name(), "decided_after": sprt.decided.map(|(k, _)| k), "observations": sprt.n},
        "net": {"answered": net.answered, "per_game": net.answered as f64 / n.max(1.0),
                "encode_us": net.encode_ns as f64 / 1e3 / net.answered.max(1) as f64, "forward_us": net.net_ns as f64 / 1e3 / net.answered.max(1) as f64,
                "agreement_with_rule_bot": if net.compared > 0 { Value::from(net.agree as f64 / net.compared as f64) } else { Value::Null }, "compared": net.compared},
        "games_list": results.iter().map(|r| json!({"seed": r.seed, "kind": format!("{:?}", r.kind), "places": r.places, "factions": r.factions, "z": r.z, "vp": r.vp, "finished": r.finished, "round": r.round, "error": r.error, "seconds": r.seconds})).collect::<Vec<_>>(),
    });
    let text = serde_json::to_string_pretty(&report).unwrap_or_default();
    if let Some(out) = get("--out").map(PathBuf::from) {
        if let Err(e) = std::fs::write(&out, &text) {
            eprintln!("{}: {e}", out.display());
        }
    }
    let mut short = report.clone();
    if let Some(m) = short.as_object_mut() {
        m.remove("games_list");
    }
    println!("{}", serde_json::to_string_pretty(&short).unwrap_or_default());
}
