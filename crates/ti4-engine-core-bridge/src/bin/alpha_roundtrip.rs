//! The alpha-log round trip (package D2-1): `store(load(g)) == g` over every position of every stored game.
//!
//! ```text
//! D=$(mktemp -d); rsync -a --exclude feedback /var/lib/ti4-public/ $D/
//! alpha_roundtrip $D [--every K] [--strict]
//! ```
//!
//! `DIR` is always a **copy** of the server data directory; the live one is refused. Per game: decisions, positions
//! checked, differences, refusals by reason and the high-water marks of the per-game tables. Exit status 1 on any
//! difference, any failure to read or replay a game, and with `--strict` on any refusal (`Unmapped`); the census is
//! printed first. `TI4_UNMAPPED=strict` in the environment makes the first refusal a panic instead (to find its call
//! site). See `ti4_engine_core_bridge::alpha`.

use std::path::Path;

use ti4_engine_core_bridge::alpha::{check_game, list_games, refuse_live_dir};
use ti4_engine_core_bridge::roundtrip::{Checker, Stats};
use ti4_server::storage::FileGameStore;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let strict = args.iter().any(|a| a == "--strict");
    ti4_engine_legacy::core_sync::unsupported::set_disabled_policy_for_gate();
    let every = args
        .iter()
        .position(|a| a == "--every")
        .and_then(|i| args.get(i + 1))
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(1);
    let Some(dir) = args.first().filter(|a| !a.starts_with("--")) else {
        eprintln!("usage: alpha_roundtrip DIR [--every K] [--strict]   (DIR = a copy of /var/lib/ti4-public)");
        std::process::exit(2);
    };
    let dir = Path::new(dir);
    if let Err(e) = refuse_live_dir(dir) {
        eprintln!("{e}");
        std::process::exit(2);
    }
    let storage = FileGameStore::new(dir).unwrap_or_else(|e| {
        eprintln!("cannot open {}: {e}", dir.display());
        std::process::exit(2);
    });
    let ids = list_games(&storage).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });
    let mut total = Stats::default();
    let (mut checked, mut skipped, mut failed) = (0usize, 0usize, 0usize);
    let mut rules_versions = std::collections::BTreeSet::new();
    for id in &ids {
        let mut checker = Checker::new();
        match check_game(&storage, id, every, &mut checker) {
            Ok(o) if o.skipped.is_some() => {
                skipped += 1;
                println!("{id}: skipped ({})", o.skipped.unwrap_or_default());
            }
            Ok(o) => {
                checked += 1;
                rules_versions.insert(o.rules_version);
                let s = &checker.stats;
                println!(
                    "{id}: decisions {} | positions {} | differences {} | core diffs {} | unmapped positions {} | max owners {} texts {} blobs {} | rules {}",
                    o.decisions,
                    o.positions,
                    s.mismatches.len(),
                    s.core_diffs.len(),
                    s.unmapped_positions(),
                    s.max_owners,
                    s.max_texts,
                    s.max_blobs,
                    o.rules_version
                );
                for m in s.mismatches.iter().take(5) {
                    println!("  MISMATCH {m}");
                }
                for m in s.core_diffs.iter().take(5) {
                    println!("  CORE DIFF {m}");
                }
                total.add(checker.stats);
            }
            Err(e) => {
                failed += 1;
                println!("{id}: FAILED {e}");
            }
        }
    }
    for (reason, (n, example)) in &total.unmapped {
        println!("UNMAPPED x{n}: {reason} (e.g. {example})");
    }
    let census = ti4_engine_legacy::core_sync::unmapped::take();
    println!(
        "alpha_roundtrip: {} games listed, {checked} checked, {skipped} skipped, {failed} failed | positions {} | differences {} | core diffs {} | unmapped {} (census {}) | max owners {} texts {} blobs {} | rules versions {rules_versions:?}",
        ids.len(),
        total.positions,
        total.mismatches.len(),
        total.core_diffs.len(),
        total.unmapped_positions(),
        census.total,
        total.max_owners,
        total.max_texts,
        total.max_blobs,
    );
    for (reason, n) in census.by_name() {
        println!("  {n:>8}  {reason}");
    }
    if failed > 0 || !total.round_trip_holds() || (strict && (total.unmapped_positions() > 0 || census.total > 0)) {
        std::process::exit(1);
    }
}
