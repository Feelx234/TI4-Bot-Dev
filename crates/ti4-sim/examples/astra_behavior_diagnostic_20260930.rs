//! Read-only census for the 2026-09-30 merge review. Never updates behavioral bounds.
use std::collections::BTreeMap;
use ti4_sim::{
    behavior,
    result::Batch,
    run::{self, Horizon, Seats},
};

fn main() {
    let players: Vec<_> = (1..=6)
        .map(|n| ti4_model::id::PlayerId::new(format!("p{n}")))
        .collect();
    let content = ti4_content::ContentStore::embedded();
    // Serial execution bounds resource use; run_with's default would occupy all cores.
    let mut batch = Batch::default();
    let mut totals = BTreeMap::<String, usize>::new();
    for seed in behavior::SEEDS {
        let result = run::play_with(
            content,
            &players,
            ti4_model::content_types::DEFAULT,
            seed,
            Horizon::default(),
            Seats::Scored,
        );
        for (label, count) in &result.events {
            *totals.entry(label.clone()).or_default() += count;
        }
        println!(
            "SEED {}",
            serde_json::json!({"seed": seed, "rounds": result.rounds,
            "decisions": result.decisions, "ending": result.ended_because.label(),
            "error": result.error, "vp": result.victory_points, "events": result.events})
        );
        batch.results.push(result);
    }
    println!("TOTALS {}", serde_json::to_string(&totals).unwrap());
    let bounds = behavior::baseline_bounds();
    for (metric, value) in behavior::batch_metrics(&batch) {
        let (lo, hi) = bounds[&metric];
        println!(
            "METRIC {metric} {value:.12} [{lo:.12}, {hi:.12}] inside={}",
            value >= lo && value <= hi
        );
    }
}
