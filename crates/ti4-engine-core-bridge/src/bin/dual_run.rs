//! The dual run in a game (docs/ENGINE_CORE_STATE.md, 3.3): golden games and the template openings
//! played with the chosen areas on `Dual`, every answer compared, then the games played again on
//! `String` and their decision hashes compared.
//!
//! ```text
//! dual_run CONFIG GAMES [AREAS]     AREAS as TI4_AREAS (default: movement=dual)
//! ```
//! Prints the [`DualReport`](ti4_engine::dual::DualReport); exit 1 on any difference or hash change.

use ti4_engine::dual::{self, Area, AreaMask, Backend};
use ti4_engine_core_bridge::workload::golden_positions;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(config), Some(games)) = (args.first(), args.get(1).and_then(|g| g.parse::<usize>().ok())) else {
        eprintln!("usage: dual_run CONFIG GAMES [AREAS]");
        std::process::exit(2);
    };
    let spec = args.get(2).map_or("movement=dual", String::as_str);
    let dual_mask = AreaMask::parse(spec, AreaMask::STRING).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let _ = dual::take_report();

    dual::set_default_mask(dual_mask);
    // Stride beyond any game length: only the games are played, positions are not kept.
    let (_, dual_hashes) = golden_positions(config, games, usize::MAX).expect("dual games");
    let report = dual::take_report();

    dual::set_default_mask(AreaMask::all(Backend::String));
    let (_, string_hashes) = golden_positions(config, games, usize::MAX).expect("string games");
    let mut bad = report.total_differences() > 0;
    for (i, (d, s)) in dual_hashes.iter().zip(&string_hashes).enumerate() {
        if d != s {
            println!("HASH CHANGED game {i}: dual {d} string {s}");
            bad = true;
        }
    }
    let on_dual: Vec<_> = Area::ALL.into_iter().filter(|a| dual_mask.backend(*a) == Backend::Dual).map(Area::name).collect();
    println!("games {} | areas on dual: {on_dual:?} | hashes equal: {}", dual_hashes.len(), dual_hashes == string_hashes);
    print!("{report}");
    std::process::exit(i32::from(bad));
}
