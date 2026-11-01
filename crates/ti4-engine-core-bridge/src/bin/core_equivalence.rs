//! Dual run of the typed movement slice against the string engine (docs/ENGINE_CORE.md).
//!
//! ```text
//! core_equivalence templates [--routes]                    opening of every map template (POK and
//!                                                         FULL), plus 37 map-switch variants each
//! core_equivalence golden CONFIG GAMES STRIDE [--routes] [--shard i/N]
//!                                                         every STRIDE-th position of the first
//!                                                         GAMES games of a selfplay config; with
//!                                                         --shard only the positions whose index % N == i
//!                                                         (the games are played by every shard)
//! ```
//! Exit status 1 when any answer differs.

use ti4_engine_core_bridge::check::{Report, check_content, check_galaxy_variants, check_neighbours, check_observed, check_position};
use ti4_engine_core_bridge::workload::{golden_positions, template_openings};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn print(report: &Report) {
    println!(
        "positions {} | adjacency checks {} | rules built {} (plain {}, with hook state {}, board facts changed by a hook {}, hook map copies {}) | \
         path queries {} (reachable {}, plain-builder queries {}) | route sets {} | seat rules {} | observed answers {} | neighbour answers {} | content answers {} | hook answers {} | unmapped {} | \
         mismatches {}",
        report.positions,
        report.adjacency_checks,
        report.rules_built,
        report.plain_rules,
        report.hooked,
        report.board_differs,
        report.hook_copies,
        report.queries,
        report.reachable,
        report.plain_queries,
        report.route_sets,
        report.seat_rules,
        report.observed_answers,
        report.neighbour_answers,
        report.content_answers,
        report.hook_answers,
        report.unmapped,
        report.mismatches.len()
    );
    for m in &report.mismatches {
        println!("MISMATCH {m}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let routes = args.iter().any(|a| a == "--routes");
    let mut report = Report::default();
    match args.first().map(String::as_str) {
        Some("templates") => {
            check_content(&mut report);
            let (positions, refused) = template_openings();
            for p in &positions {
                check_galaxy_variants(p, &mut report);
                check_position(p, routes, &mut report);
                check_observed(p, &mut report);
                check_neighbours(p, &mut report);
            }
            println!("templates built {}, refused {}", positions.len(), refused.len());
            for r in refused {
                println!("refused: {r}");
            }
        }
        Some("golden") => {
            let config = &args[1];
            let games: usize = args[2].parse().expect("GAMES");
            let stride: usize = args[3].parse().expect("STRIDE");
            let shard = args.iter().position(|a| a == "--shard").map(|at| {
                let spec = args.get(at + 1).expect("--shard i/N");
                let (i, n) = spec.split_once('/').expect("--shard i/N");
                (i.parse::<usize>().expect("shard index"), n.parse::<usize>().expect("shard count"))
            });
            let (positions, hashes) = golden_positions(config, games, stride).expect("games");
            for h in &hashes {
                println!("game hash {h}");
            }
            for (at, p) in positions.iter().enumerate() {
                if shard.is_some_and(|(i, n)| at % n != i) {
                    continue;
                }
                check_position(p, routes, &mut report);
                check_observed(p, &mut report);
                check_neighbours(p, &mut report);
            }
        }
        Some("sizes") => {
            use std::mem::size_of;
            println!(
                "size_of: MovementRules {} | CoreRules {} | Galaxy {} | CoreGalaxy {} | Adjacency per board {} B | Unit {} | typed unit (u16 type, u8 owner, 2 flags) 5 | SystemId {} | SystemIx {} | SysSet {}",
                size_of::<ti4_engine_legacy::movement::MovementRules<'_>>(),
                size_of::<ti4_engine_core::movement::CoreRules>(),
                size_of::<ti4_content::galaxy::Galaxy>(),
                size_of::<ti4_engine_core::galaxy::CoreGalaxy>(),
                ti4_engine_core::tables::SYSTEM_COUNT * size_of::<ti4_engine_core::SysSet>(),
                size_of::<ti4_model::units::Unit>(),
                size_of::<ti4_model::id::SystemId>(),
                size_of::<ti4_engine_core::SystemIx>(),
                size_of::<ti4_engine_core::SysSet>(),
            );
            return;
        }
        _ => {
            eprintln!("usage: core_equivalence templates|golden CONFIG GAMES STRIDE [--routes]");
            std::process::exit(2);
        }
    }
    print(&report);
    if !report.mismatches.is_empty() {
        std::process::exit(1);
    }
}
