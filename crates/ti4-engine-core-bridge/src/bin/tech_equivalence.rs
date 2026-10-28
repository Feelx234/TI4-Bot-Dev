//! Dual run of the typed technology queries (package A3) against the string engine.
//!
//! ```text
//! tech_equivalence templates                    the opening of every map template
//! tech_equivalence golden CONFIG GAMES STRIDE   every STRIDE-th position of the first GAMES games
//! ```
//! Every `(position, seat, technology)` is asked of the string implementation, of the core over a
//! lent view and of the adapter's `Core` path. Exit status 1 when any answer differs.

use ti4_engine_core_bridge::check_tech::{TechReport, check_tech_content, check_tech_position};
use ti4_engine_core_bridge::workload::{golden_positions, template_openings};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut report = TechReport::default();
    check_tech_content(&mut report);
    match args.first().map(String::as_str) {
        Some("templates") => {
            let (positions, refused) = template_openings();
            for p in &positions {
                check_tech_position(p, &mut report);
            }
            println!("templates built {}, refused {}", positions.len(), refused.len());
        }
        Some("golden") => {
            let config = &args[1];
            let games: usize = args[2].parse().expect("GAMES");
            let stride: usize = args[3].parse().expect("STRIDE");
            let (positions, hashes) = golden_positions(config, games, stride).expect("games");
            for h in &hashes {
                println!("game hash {h}");
            }
            for p in &positions {
                check_tech_position(p, &mut report);
            }
        }
        _ => {
            eprintln!("usage: tech_equivalence templates|golden CONFIG GAMES STRIDE");
            std::process::exit(2);
        }
    }
    println!(
        "positions {} | seats {} | per-technology answers {} | per-seat answers {} | printed-data answers {} | unmapped {} | mismatches {}",
        report.positions, report.seats, report.tech_answers, report.seat_answers, report.content_answers, report.unmapped, report.mismatches.len()
    );
    for m in &report.mismatches {
        println!("MISMATCH {m}");
    }
    if !report.mismatches.is_empty() {
        std::process::exit(1);
    }
}
