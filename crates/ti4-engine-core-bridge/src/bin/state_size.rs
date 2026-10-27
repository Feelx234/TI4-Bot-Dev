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

use ti4_engine::core_sync::{Codec, Ctx, Sources};
use ti4_engine_core::state::{CoreState, SliceSet};
use ti4_engine_core_bridge::workload::{Position, golden_positions};
use ti4_model::state::GameState;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// One measured state kind: its name, `size_of`, and the closures over the position with this index.
struct Kind {
    name: &'static str,
    size_of: usize,
    bytes: Box<dyn Fn(usize) -> usize>,
    clone: Box<dyn Fn(usize) -> usize>,
}

fn kinds(positions: &[Position]) -> Vec<Kind> {
    let states: Vec<GameState> = positions.iter().map(|p| p.state.clone()).collect();
    let states_clone = states.clone();
    // The typed state of every position, loaded whole once (outside the timed clone). A position the
    // codec cannot hold (an `Unmapped`) has no entry and counts as 0.
    let cores: Vec<Option<CoreState>> = positions
        .iter()
        .map(|p| {
            let mut cx = Ctx::of(&p.state).ok()?;
            let src = Sources { galaxy: Some(&p.galaxy), rng: None };
            Codec::load(&mut cx, &p.state, src, SliceSet::ALL).ok()
        })
        .collect();
    vec![
        Kind {
            name: "GameState",
            size_of: std::mem::size_of::<GameState>(),
            bytes: Box::new(move |i| serde_json::to_vec(&states[i]).map_or(0, |v| v.len())),
            clone: Box::new(move |i| {
                let c = std::hint::black_box(&states_clone[i]).clone();
                std::hint::black_box(&c);
                c.players.len()
            }),
        },
        // `CoreState` has no serialised form (it is never sent over the wire): 0 in the bytes column.
        Kind {
            name: "CoreState",
            size_of: std::mem::size_of::<CoreState>(),
            bytes: Box::new(|_| 0),
            clone: Box::new(move |i| {
                cores[i].as_ref().map_or(0, |core| {
                    let c = std::hint::black_box(core).clone();
                    std::hint::black_box(&c);
                    1
                })
            }),
        },
    ]
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
    for kind in kinds(&positions) {
        let bytes: usize = (0..positions.len()).map(&kind.bytes).sum::<usize>() / positions.len().max(1);
        let start = Instant::now();
        let mut sink = 0;
        for _ in 0..reps {
            for i in 0..positions.len() {
                sink += (kind.clone)(i);
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
