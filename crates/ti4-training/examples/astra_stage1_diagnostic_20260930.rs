//! Inspect the unchanged failing test configuration; do not change its assertion.
fn main() {
    let mut plan = ti4_training::stage1::FactionPlan::python_reference();
    plan.generations = 1;
    plan.train_seeds = std::env::args()
        .nth(1)
        .map_or(1, |s| s.parse().expect("seed count"));
    let run = ti4_training::stage1::train_factions(ti4_content::ContentStore::embedded(), &plan);
    println!(
        "GENERATIONS {}",
        serde_json::to_string(&run.generations).unwrap()
    );
    for (faction, profile) in &run.profiles {
        let weights: Vec<_> = profile
            .learned
            .heads
            .values()
            .flat_map(|h| h.weights.iter())
            .collect();
        println!(
            "PROFILE {faction} schema={} explicit={} weights={} named={} nonzero={} sample={:?}",
            profile.schema,
            profile.is_explicit(),
            weights.len(),
            weights.iter().filter(|(n, _)| !n.starts_with('h')).count(),
            weights.iter().filter(|(_, v)| **v != 0.0).count(),
            weights.iter().take(5).collect::<Vec<_>>()
        );
    }
}
