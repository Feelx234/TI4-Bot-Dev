//! The evaluation harness (lane L7) end to end: a bench run of the greedy policy net (the tiny parity model) and a gate run.

use std::path::PathBuf;
use std::process::Command;

fn run(args: &[&str]) -> serde_json::Value {
    let o = Command::new(env!("CARGO_BIN_EXE_mcts_eval")).args(args).output().expect("the harness runs");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    serde_json::from_slice(&o.stdout).expect("a JSON report")
}

#[test]
fn a_bench_run_pairs_every_agent_game_with_the_baseline() {
    let w = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ti4-mcts-net/tests/fixtures/tiny/weights.t4w");
    let agent = format!("policy:{}", w.display());
    let r = run(&["--agent", &agent, "--seeds", "1", "--first", "900000", "--workers", "2"]);
    assert_eq!(r["games"], 5, "the baseline and the agent at 4 places");
    assert_eq!(r["errors"], 0);
    assert_eq!(r["paired"]["pairs"], 4);
    assert_eq!(r["agent_games"], 4.0);
    assert!(r["net"]["answered"].as_u64().unwrap_or(0) > 100, "the net answered the tree asks: {}", r["net"]);
    let share = r["rule_baseline_all_seats"]["share"].as_f64().unwrap_or(-1.0);
    assert!((share - 0.25).abs() < 1e-6, "the baseline's four seats share one win: {share}");
    let lo = r["win_share"]["wilson95"][0].as_f64().unwrap_or(-1.0);
    let hi = r["win_share"]["wilson95"][1].as_f64().unwrap_or(-1.0);
    assert!((0.0..=1.0).contains(&lo) && lo <= hi && hi <= 1.0);
}

#[test]
fn a_gate_run_swaps_the_seats() {
    let r = run(&["--mode", "gate", "--agent", "random", "--vs", "rule", "--seeds", "1", "--first", "900001", "--workers", "2"]);
    assert_eq!(r["games"], 2);
    assert_eq!(r["errors"], 0);
    assert_eq!(r["by_place"].as_object().map(|m| m.len()), Some(4), "the candidate sat at every place");
    assert_eq!(r["sprt"]["p0"], 0.5);
}
