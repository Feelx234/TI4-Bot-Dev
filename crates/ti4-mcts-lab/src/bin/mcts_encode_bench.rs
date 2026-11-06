//! Micro-benchmark of the feature encoding (lane L4): positions of rule-bot games at tree asks, each encoded on a fresh `Seen`
//! (as the search will: one view per node), by the reference [`encode::encode`] and by the caching [`encode::Encoder`]; the
//! two must give identical records.
//!
//! ```text
//! mcts_encode_bench [--games 3] [--first 11] [--reps 3]
//! ```

use std::time::{Duration, Instant};

use ti4_engine_core::choice::Choice;
use ti4_engine_core::seen::Seen;
use ti4_engine_core::state::CoreState;
use ti4_mcts_features::encode::{self, Encoder};
use ti4_mcts_features::{filter, kind};
use ti4_mcts_lab::{rule_bot, start, Asked, Decide, Table};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

struct Collect(Vec<(CoreState, Choice)>);

impl Decide for Collect {
    fn decide(&mut self, t: &Table, a: &Asked<'_>) -> Result<usize, String> {
        if kind::tree_kind_of(a.choice).is_some() && a.choice.opts.len() >= 2 {
            self.0.push((a.at.clone(), a.choice.clone()));
        }
        Ok(rule_bot(t, a).0)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |flag: &str, d: u64| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(d);
    let (games, first, reps) = (get("--games", 3), get("--first", 11), get("--reps", 3) as usize);
    let mut total = [Duration::ZERO; 10];
    let mut cache = encode::MapCache::default();
    let mut n = 0usize;
    let mut differ = 0usize;
    for seed in first..first + games {
        let (t, mut pos) = start(&ti4_mcts_lab::spec(seed)).expect("the table builds");
        let mut c = Collect(Vec::new());
        let _ = ti4_mcts_lab::play(&t, &mut pos, &mut c, 6000, 12, |_, _| {});
        let mut enc = Encoder::default();
        for (core, choice) in &c.0 {
            for _ in 0..reps {
                let fresh = || Seen::new(core, t.kernel, choice.actor);
                let t0 = Instant::now();
                let reference = encode::encode(&fresh(), choice);
                total[0] += t0.elapsed();
                let t0 = Instant::now();
                let fast = enc.encode(&fresh(), choice);
                total[1] += t0.elapsed();
                differ += usize::from(reference != fast);
                let v = fresh();
                let t0 = Instant::now();
                let f = encode::Frame::new(&v, choice.actor);
                total[2] += t0.elapsed();
                let t0 = Instant::now();
                for s in &f.systems {
                    let _ = v.movable_into(choice.actor, *s);
                }
                total[3] += t0.elapsed();
                let t0 = Instant::now();
                let _ = encode::progress_rows(&v, &f);
                total[4] += t0.elapsed();
                let v = fresh();
                let t0 = Instant::now();
                let _ = filter::activate(&v, choice);
                total[5] += t0.elapsed();
                let v = fresh();
                let t0 = Instant::now();
                let f = encode::Frame::cached(&v, choice.actor, &mut cache);
                total[6] += t0.elapsed();
                let t0 = Instant::now();
                let _ = encode::progress_rows(&v, &f);
                total[7] += t0.elapsed();
                let v = fresh();
                let t0 = Instant::now();
                let _ = v.movable_counts(choice.actor, &f.systems);
                total[8] += t0.elapsed();
                let t0 = Instant::now();
                let ms = v.map_systems();
                let _ = v.adjacent_of_each(&ms);
                total[9] += t0.elapsed();
                n += 1;
            }
        }
    }
    let us = |d: Duration| d.as_secs_f64() * 1e6 / n.max(1) as f64;
    println!(
        "{n} encodes: reference {:.1} us, Encoder {:.1} us (records differing: {differ}); parts of the reference: frame {:.1}, movable_into x systems {:.1}, objective progress {:.1}; activation filter alone {:.1}; Encoder parts: cached frame {:.1}, objective progress {:.1}, movable_counts {:.1}, map systems + adjacency {:.1}",
        us(total[0]),
        us(total[1]),
        us(total[2]),
        us(total[3]),
        us(total[4]),
        us(total[5]),
        us(total[6]),
        us(total[7]),
        us(total[8]),
        us(total[9])
    );
    if differ > 0 {
        std::process::exit(1);
    }
}
