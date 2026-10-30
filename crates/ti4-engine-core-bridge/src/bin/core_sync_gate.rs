//! Gate 4 of docs/ENGINE_CORE_STATE.md (round trip) for the core_sync codecs, all slices.
//!
//! ```text
//! core_sync_gate CONFIG GAMES STRIDE [--strict]
//! ```
//! Positions: the opening of every map template (all lobby factions), `fixtures::game` tables of 3,
//! 4 and 6 seats, and every STRIDE-th position of the first GAMES games of CONFIG. For each:
//!
//! 1. `load` every slice (an `Unmapped` is counted by reason; it is a fallback to the string engine,
//!    not a failure, unless `--strict`);
//! 2. `store` every slice into a clone of the **previous position's** state (so a field no codec
//!    writes would keep the other position's value) and compare the serialised bytes with the
//!    position: `store(load(g)) == g`;
//! 3. `load` the stored state again with the same tables and compare the core states:
//!    `load(store(x)) == x`;
//! 4. time every slice's load and store, and count its string-side JSON bytes (lend cost table).
//!
//! The per-position check is `ti4_engine_core_bridge::roundtrip` (shared with `alpha_roundtrip`). The process
//! counts every `Unmapped` it meets (`ti4_engine::core_sync::unmapped`) and prints the census by (slice, reason);
//! `--strict` exits 1 on any refusal (the whole census is printed first; `TI4_UNMAPPED=strict` in the environment
//! makes the first refusal a panic instead, to find its call site).
//!
//! Exit status 1 when any round trip differs.


use ti4_engine::core_sync::Slice;
use ti4_engine_core::state::CoreState;
use ti4_engine_core_bridge::roundtrip::{Checker, inline_size};
use ti4_engine_core_bridge::workload::{Position, golden_positions, template_openings};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn fixtures(galaxy: &ti4_content::galaxy::Galaxy) -> Vec<Position> {
    let mut out = Vec::new();
    for seats in [&["a", "b", "c"][..], &["a", "b", "c", "d"], &["a", "b", "c", "d", "e", "f"]] {
        out.push(Position {
            label: format!("fixture {} seats", seats.len()),
            // real lobby factions, deployed, notes dealt (the placeholder `generic` is not in the corpus)
            state: ti4_engine::fixtures::real_game(seats),
            galaxy: galaxy.clone(),
            sources: ti4_model::content_types::POK,
        });
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let strict = args.iter().any(|a| a == "--strict");
    ti4_engine::core_sync::unsupported::set_disabled_policy_for_gate();
    let config = args.first().map_or("tools/engine_profile/profile-20.json", String::as_str);
    let games: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    let stride: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);

    let (mut positions, refused) = template_openings();
    println!("template openings {} (refused {})", positions.len(), refused.len());
    let templates = positions.len();
    let galaxy = positions.first().expect("a template").galaxy.clone();
    positions.extend(fixtures(&galaxy));
    let fixture_count = positions.len() - templates;
    let (golden, hashes) = golden_positions(config, games, stride).expect("golden games");
    println!("golden games {} positions {}", hashes.len(), golden.len());
    positions.extend(golden);

    let mut checker = Checker::new();
    for p in &positions {
        checker.check_position(&p.label, &p.state, &p.galaxy);
    }
    let stats = &checker.stats;
    let (loaded, checked_core) = (stats.loaded, stats.checked_core);

    println!(
        "positions {} (templates {templates}, fixtures {fixture_count}, golden {}) | fully loaded {loaded} | unmapped positions {} | store(load) mismatches {} | load(store) core diffs {} of {checked_core}",
        positions.len(),
        positions.len() - templates - fixture_count,
        positions.len() - loaded,
        stats.mismatches.len(),
        stats.core_diffs.len()
    );
    for (reason, (n, example)) in &stats.unmapped {
        println!("UNMAPPED x{n}: {reason} (e.g. {example})");
    }
    for m in stats.mismatches.iter().take(40) {
        println!("MISMATCH {m}");
    }
    for m in stats.core_diffs.iter().take(20) {
        println!("CORE DIFF {m}");
    }
    let census = ti4_engine::core_sync::unmapped::take();
    println!("unmapped (census of the run, by slice and reason): {}", census.total);
    for (reason, n) in census.by_name() {
        println!("  {n:>8}  {reason}");
    }
    println!(
        "high-water marks: owners {}, texts {}, blobs {}",
        stats.max_owners, stats.max_texts, stats.max_blobs
    );
    println!("size_of CoreState {} B, Player {} B", std::mem::size_of::<CoreState>(), std::mem::size_of::<ti4_engine_core::state::Player>());
    println!("\nlend cost per slice (mean over {loaded} loaded positions): string JSON bytes, typed inline bytes, load ns, store ns");
    for s in Slice::ALL {
        let c = stats.costs[s as usize];
        let n = c.n.max(1);
        println!(
            "| {s:?} | {} | {} | {} | {} |",
            c.bytes / (positions.len() as u128).max(1),
            inline_size(s),
            c.load_ns / n,
            c.store_ns / n
        );
    }
    if !stats.round_trip_holds() || (strict && (loaded != positions.len() || census.total != 0)) {
        std::process::exit(1);
    }
}
