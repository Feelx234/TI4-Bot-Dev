//! Offline dual of `Area::BoardQueries` (package A1): every typed board query next to the string
//! function it replaces, on golden positions and template openings.
//!
//! ```text
//! board_equivalence templates
//! board_equivalence golden CONFIG GAMES STRIDE [LAW_STRIDE]
//! board_equivalence cards templates
//! board_equivalence cards golden CONFIG GAMES STRIDE [VARIANTS]
//! ```
//! `cards` is the offline dual of `Area::ExplorationRelics` (package B5): the relic, promissory,
//! exploration, legendary and Fracture queries, on each position and on VARIANTS deterministic
//! variants of it that hand out relics, fragments and notes.
//! Exit status 1 when any answer differs.

use ti4_engine::board_check::{BoardCheck, compare, compare_laws};
use ti4_engine_core_bridge::workload::{golden_positions, template_openings};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("cards") {
        return cards(&args[1..]);
    }
    let mut out = BoardCheck::default();
    match args.first().map(String::as_str) {
        Some("templates") => {
            let (positions, refused) = template_openings();
            println!("templates built {}, refused {}", positions.len(), refused.len());
            for p in &positions {
                compare(&p.label, &p.state, Some(&p.galaxy), p.sources, &mut out);
            }
            if let Some(p) = positions.first() {
                compare_laws(&p.label, &p.state, Some(&p.galaxy), p.sources, &mut out);
            }
        }
        Some("golden") => {
            let games: usize = args[2].parse().expect("GAMES");
            let stride: usize = args[3].parse().expect("STRIDE");
            let law_stride: usize = args.get(4).map_or(40, |s| s.parse().expect("LAW_STRIDE"));
            let (positions, hashes) = golden_positions(&args[1], games, stride).expect("games");
            for h in &hashes {
                println!("game hash {h}");
            }
            for (i, p) in positions.iter().enumerate() {
                compare(&p.label, &p.state, Some(&p.galaxy), p.sources, &mut out);
                if i % law_stride == 0 {
                    compare_laws(&p.label, &p.state, Some(&p.galaxy), p.sources, &mut out);
                }
            }
        }
        _ => {
            eprintln!("usage: board_equivalence templates | golden CONFIG GAMES STRIDE [LAW_STRIDE]");
            std::process::exit(2);
        }
    }
    println!(
        "board queries: positions {} | law variants {} | answers compared {} | unmapped {} | mismatches {}",
        out.positions,
        out.law_variants,
        out.compared,
        out.unmapped,
        out.mismatches.len()
    );
    for m in &out.mismatches {
        println!("MISMATCH {m}");
    }
    if !out.mismatches.is_empty() {
        std::process::exit(1);
    }
}

fn cards(args: &[String]) {
    use ti4_engine::explore_check::{CardsCheck, compare};
    let mut out = CardsCheck::default();
    match args.first().map(String::as_str) {
        Some("templates") => {
            let (positions, refused) = template_openings();
            println!("templates built {}, refused {}", positions.len(), refused.len());
            for p in &positions {
                compare(&p.label, &p.state, p.sources, 24, &mut out);
            }
        }
        Some("golden") => {
            let games: usize = args[2].parse().expect("GAMES");
            let stride: usize = args[3].parse().expect("STRIDE");
            let variants: u64 = args.get(4).map_or(6, |s| s.parse().expect("VARIANTS"));
            let (positions, hashes) = golden_positions(&args[1], games, stride).expect("games");
            for h in &hashes {
                println!("game hash {h}");
            }
            for p in &positions {
                compare(&p.label, &p.state, p.sources, variants, &mut out);
            }
        }
        _ => {
            eprintln!("usage: board_equivalence cards templates | cards golden CONFIG GAMES STRIDE [VARIANTS]");
            std::process::exit(2);
        }
    }
    println!(
        "exploration queries: positions {} | variants {} | answers compared {} | unmapped {} | mismatches {}",
        out.positions,
        out.variants,
        out.compared,
        out.unmapped,
        out.mismatches.len()
    );
    for r in &out.reasons {
        println!("unmapped: {r}");
    }
    for m in &out.mismatches {
        println!("MISMATCH {m}");
    }
    if !out.mismatches.is_empty() {
        std::process::exit(1);
    }
}
