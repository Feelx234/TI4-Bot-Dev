//! A smoke exporter of training shards (not lane L2: no exploration noise, no subsampling): rule-bot games, every tree node with
//! two or more options after the filter becomes a record with the rule bot's pick as a one-hot target (no target when the bot had
//! no opinion, `smart == false`), the outcome filled at the end of the game.
//!
//! ```text
//! mcts_shards --games N [--first SEED] --out DIR      one shard directory DIR (overwritten)
//! ```
//!
//! Prints the encode time per record (the cost of the features in the search).

use std::time::{Duration, Instant};

use ti4_engine_core::seen::Seen;
use ti4_mcts_features::encode::{self, SEATS};
use ti4_mcts_features::kind;
use ti4_mcts_features::shard::{Meta, Record, ShardWriter};
use ti4_mcts_lab::{rule_bot, start, Asked, Decide, Table};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

struct Export {
    w: ShardWriter,
    game: u64,
    seed: u64,
    encode: Duration,
    records: u64,
    dropped_pick: u64,
    profile: bool,
    /// Frame, reach of every system, filter reasons of every system, objective progress.
    parts: [Duration; 4],
}

impl Decide for Export {
    fn decide(&mut self, t: &Table, a: &Asked<'_>) -> Result<usize, String> {
        let (picked, smart, _, _) = rule_bot(t, a);
        if let Some(tk) = kind::tree_kind_of(a.choice) {
            let seen = Seen::new(a.at, t.kernel, a.choice.actor);
            if self.profile {
                // The pieces, each on a fresh view (the view memoises the reach and the objective scans).
                let fresh = || Seen::new(a.at, t.kernel, a.choice.actor);
                let t0 = Instant::now();
                let f = encode::Frame::new(&fresh(), a.choice.actor);
                self.parts[0] += t0.elapsed();
                let v = fresh();
                let t0 = Instant::now();
                for s in &f.systems {
                    let _ = v.movable_into(a.choice.actor, *s);
                }
                self.parts[1] += t0.elapsed();
                let v = fresh();
                let t0 = Instant::now();
                let board = ti4_mcts_features::filter::Board::of(&v, a.choice.actor);
                for s in &f.systems {
                    let _ = board.reasons(&v, a.choice.actor, *s);
                }
                self.parts[2] += t0.elapsed();
                let v = fresh();
                let t0 = Instant::now();
                let _ = encode::progress_rows(&v, &f);
                self.parts[3] += t0.elapsed();
            }
            let t0 = Instant::now();
            let enc = encode::encode(&seen, a.choice);
            self.encode += t0.elapsed();
            if self.w.names.is_none() {
                self.w.names = Some(encode::column_names(&seen, a.choice));
            }
            if enc.mask.iter().filter(|m| **m).count() >= 2 {
                let mut enc = enc;
                // The rule bot's pick outside the filter (1 % of the activations): put it back into the kept set.
                if !enc.mask[picked] {
                    enc.mask[picked] = true;
                    self.dropped_pick += 1;
                }
                let mut pi = vec![0.0; enc.k()];
                if smart {
                    pi[picked] = 1.0;
                }
                let rel = encode::relative_seats(&seen, a.choice.actor);
                let mut rel4 = [0u8; SEATS];
                for (i, s) in rel.iter().take(SEATS).enumerate() {
                    rel4[i] = s.0;
                }
                let meta = Meta {
                    game: self.game,
                    seed: self.seed,
                    round: a.at.meta().round,
                    actor: a.choice.actor.0,
                    faction: enc.seat_fac[0],
                    ask_kind: enc.ask_kind,
                    tree_kind: i8::try_from(tk.index()).unwrap_or(-1),
                    smart,
                    picked: u16::try_from(picked).unwrap_or(u16::MAX),
                    decision: u32::try_from(a.index).unwrap_or(u32::MAX),
                    net: 0,
                    sims: 0,
                    rel: rel4,
                };
                self.w.push(Record { enc, pi, meta });
                self.records += 1;
            }
        }
        Ok(picked)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let games: u64 = get("--games").and_then(|v| v.parse().ok()).unwrap_or(4);
    let first: u64 = get("--first").and_then(|v| v.parse().ok()).unwrap_or(1);
    let Some(out) = get("--out") else {
        eprintln!("--out DIR is required");
        std::process::exit(2);
    };
    let mut ex = Export { w: ShardWriter::new(), game: 0, seed: 0, encode: Duration::ZERO, records: 0, dropped_pick: 0, profile: args.iter().any(|a| a == "--profile-encode"), parts: [Duration::ZERO; 4] };
    let started = Instant::now();
    for (g, seed) in (first..first + games).enumerate() {
        let (t, mut pos) = match start(&ti4_mcts_lab::spec(seed)) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("seed {seed}: {e}");
                continue;
            }
        };
        ex.game = g as u64;
        ex.seed = seed;
        let end = ti4_mcts_lab::play(&t, &mut pos, &mut ex, 6000, 12, |_, _| {});
        if end.error.is_some() {
            ex.w.drop_game();
            eprintln!("seed {seed}: {:?}", end.error);
            continue;
        }
        // The winner: the most victory points (a tie splits); a game stopped by the caps the same way.
        let vp: Vec<f32> = end.vp.iter().map(|v| f32::from(*v)).collect();
        let best = vp.iter().copied().fold(f32::MIN, f32::max);
        let winners = vp.iter().filter(|v| **v == best).count() as f32;
        let z: Vec<f32> = vp.iter().map(|v| if *v == best { 1.0 / winners } else { 0.0 }).collect();
        ex.w.end_game(&z, &vp);
    }
    let n = ex.w.write(std::path::Path::new(&out)).unwrap_or_else(|e| {
        eprintln!("write: {e}");
        std::process::exit(1)
    });
    println!(
        "{n} records from {games} games in {:.1} s; encode {:.1} us per tree ask ({} encodes); rule-bot picks put back into the filter {}",
        started.elapsed().as_secs_f64(),
        ex.encode.as_secs_f64() * 1e6 / ex.records.max(1) as f64,
        ex.records,
        ex.dropped_pick
    );
    if ex.profile {
        let per = |d: Duration| d.as_secs_f64() * 1e6 / ex.records.max(1) as f64;
        println!("per tree ask (us): frame {:.1}, movable_into x systems {:.1}, filter reasons x systems {:.1}, objective progress {:.1}", per(ex.parts[0]), per(ex.parts[1]), per(ex.parts[2]), per(ex.parts[3]));
    }
}
