//! Size and clone cost of the states (docs/ENGINE_CORE_STATE.md 1.6, gate 10).
//!
//! ```text
//! state_size CONFIG GAMES STRIDE [REPS]
//! ```
//! Plays the first GAMES games of CONFIG and keeps every STRIDE-th position; then, per state kind,
//! prints `size_of`, the serialised size and the CPU time of one clone (mean over the positions and
//! REPS repetitions, thread CPU time). `GameState` is the string engine's state; each state kind the
//! foundation adds (`CoreState`, F1) is one more entry of [`kinds`].
//!
//! Allocation counts need an allocator hook, which the workspace forbids (`unsafe_code`): run with
//! `MIMALLOC_SHOW_STATS=1` and REPS=0 vs REPS=n to difference the allocator's own counters
//! (`tools/engine_core/state_size.sh` does).

use std::time::Instant;

use ti4_engine_core_bridge::workload::{Position, golden_positions};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// One measured state kind: its name, `size_of`, and the closures over a position.
struct Kind {
    name: &'static str,
    size_of: usize,
    bytes: fn(&Position) -> usize,
    clone: fn(&Position) -> usize,
}

fn kinds() -> Vec<Kind> {
    vec![Kind {
        name: "GameState",
        size_of: std::mem::size_of::<ti4_model::state::GameState>(),
        bytes: |p| serde_json::to_vec(&p.state).map_or(0, |v| v.len()),
        clone: |p| {
            let c = std::hint::black_box(&p.state).clone();
            std::hint::black_box(&c);
            c.players.len()
        },
    }]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(config), Some(games), Some(stride)) = (
        args.first(),
        args.get(1).and_then(|v| v.parse::<usize>().ok()),
        args.get(2).and_then(|v| v.parse::<usize>().ok()),
    ) else {
        eprintln!("usage: state_size CONFIG GAMES STRIDE [REPS]");
        std::process::exit(2);
    };
    let reps: usize = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(5);
    let (positions, _) = golden_positions(config, games, stride).expect("games");
    println!("positions {} | reps {reps}", positions.len());
    for kind in kinds() {
        let bytes: usize = positions.iter().map(kind.bytes).sum::<usize>() / positions.len().max(1);
        let start = Instant::now();
        let mut sink = 0;
        for _ in 0..reps {
            for p in &positions {
                sink += (kind.clone)(p);
            }
        }
        let n = (reps * positions.len()).max(1);
        let ns = start.elapsed().as_nanos() / n as u128;
        println!(
            "{}: size_of {} B | serialised mean {} B | clone mean {} ns ({} clones, sink {sink})",
            kind.name, kind.size_of, bytes, ns, reps * positions.len()
        );
    }
}
