//! Micro-benchmark of the forward pass (lane L4).
//!
//! ```text
//! mcts_net_bench --weights W.t4w --inputs DIR [--reps 20]     DIR: a plain shard (a fixture's `inputs/`)
//! ```
//!
//! Prints the mean time per record and per option, and the split of the records by option count.

use std::path::PathBuf;
use std::time::Instant;

use ti4_mcts_net::{plain_shard, Net, Scratch};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let (Some(w), Some(inputs)) = (get("--weights"), get("--inputs")) else {
        eprintln!("usage: mcts_net_bench --weights W.t4w --inputs DIR [--reps N]");
        std::process::exit(2);
    };
    let reps: usize = get("--reps").and_then(|v| v.parse().ok()).unwrap_or(20);
    let net = Net::load(&PathBuf::from(w)).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let recs = plain_shard::read(&PathBuf::from(inputs)).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let mut s = Scratch::default();
    let mut sink = 0.0f32;
    for e in &recs {
        sink += net.forward(e, &mut s).win[0];
    }
    let opts: usize = recs.iter().map(|e| e.mask.len()).sum();
    let t0 = Instant::now();
    for _ in 0..reps {
        for e in &recs {
            sink += net.forward(e, &mut s).win[0];
        }
    }
    let us = t0.elapsed().as_secs_f64() * 1e6 / (reps * recs.len()).max(1) as f64;
    println!(
        "{} records ({:.1} options each), {reps} reps: {us:.1} us per record (checksum {sink:.3})",
        recs.len(),
        opts as f64 / recs.len().max(1) as f64
    );
}
