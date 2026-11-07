//! The pretraining dataset generator (docs/MCTS_BOT_PLAN.md 4.1, lane L2): rule-bot games with epsilon-noise exploration.
//!
//! ```text
//! mcts_dataset --out DIR [--games N] [--first SEED] [--workers W] [--per-shard G] [--eps 0.15] [--eps-early 0.3]
//!              [--early-rounds 2] [--rates draft=0.4,menu=0.12,...] [--cap 200]
//! ```
//!
//! * Seeds `first .. first + N`; the seating is `seed % 24`, so a run over a multiple of 24 seeds plays all 24 seatings equally.
//! * At every **tree-kind** ask with two or more options after the hard filter the rule bot answers; with probability `eps`
//!   (`eps-early` in rounds `1..=early-rounds`) the **played** option is uniform among the kept options instead. The recorded
//!   policy target is always the rule bot's pick (DAgger-style), none when the bot had no opinion (`smart == false`).
//! * Subsampling per game: a tree ask is recorded with the probability of its kind (`--rates`; the rare kinds always), at most
//!   `cap` records per game. The decision is made before the encoding (the encoding costs nothing for the others).
//! * Every recorded ask asserts the 1:1 wire / core option order ([`ti4_mcts_lab::aligned`]); a misaligned ask stops the game.
//! * The outcome (`z`: winner one-hot with ties split, final victory points) is filled at the end of the game. Games that end
//!   by an error are dropped; games stopped by the round cap keep their records (outcome by victory points) and are counted.
//! * Workers are threads; each writes a shard directory `DIR/g<first seed>-w<worker>` every `G` games, and at the end
//!   `DIR/summary-<first>.json` (games, records per kind, wins per faction, errors, timings).
//!
//! The noise is a function of (seed, decision index): a run is reproducible.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ti4_engine_core::seen::Seen;
use ti4_mcts_features::encode::{self, Encoder, SEATS};
use ti4_mcts_features::filter;
use ti4_mcts_features::kind::{self, TreeKind};
use ti4_mcts_features::shard::{Meta, Record, ShardWriter};
use ti4_mcts_lab::{aligned, mix, outcome, rule_bot, start, unit, Asked, Decide, Table};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// The default recording probability per tree kind (index = `TreeKind::index`): about 110 records per game.
const RATES: [f64; 7] = [0.4, 0.12, 0.07, 0.3, 0.15, 1.0, 1.0];

#[derive(Clone)]
struct Opts {
    eps: f64,
    eps_early: f64,
    early_rounds: u32,
    rates: [f64; 7],
    cap: usize,
}

#[derive(Default, Clone)]
struct Stats {
    games: u64,
    finished: u64,
    capped: u64,
    errors: u64,
    records: u64,
    tree_asks: u64,
    noisy: u64,
    put_back: u64,
    no_target: u64,
    by_kind: [u64; 7],
    wins: BTreeMap<String, f64>,
    vp: BTreeMap<String, f64>,
    seats: BTreeMap<String, u64>,
    rounds: BTreeMap<u32, u64>,
    encode: Duration,
    error_texts: Vec<String>,
}

impl Stats {
    fn add(&mut self, o: &Self) {
        self.games += o.games;
        self.finished += o.finished;
        self.capped += o.capped;
        self.errors += o.errors;
        self.records += o.records;
        self.tree_asks += o.tree_asks;
        self.noisy += o.noisy;
        self.put_back += o.put_back;
        self.no_target += o.no_target;
        for i in 0..7 {
            self.by_kind[i] += o.by_kind[i];
        }
        for (k, v) in &o.wins {
            *self.wins.entry(k.clone()).or_default() += v;
        }
        for (k, v) in &o.vp {
            *self.vp.entry(k.clone()).or_default() += v;
        }
        for (k, v) in &o.seats {
            *self.seats.entry(k.clone()).or_default() += v;
        }
        for (k, v) in &o.rounds {
            *self.rounds.entry(*k).or_default() += v;
        }
        self.encode += o.encode;
        self.error_texts.extend(o.error_texts.iter().take(20).cloned());
    }
}

struct Gen {
    o: Opts,
    w: ShardWriter,
    enc: Encoder,
    seed: u64,
    in_game: usize,
    misaligned: Option<String>,
    st: Stats,
}

impl Decide for Gen {
    fn decide(&mut self, t: &Table, a: &Asked<'_>) -> Result<usize, String> {
        let (picked, smart, _, _) = rule_bot(t, a);
        let Some(tk) = kind::tree_kind_of(a.choice) else { return Ok(picked) };
        let seen = Seen::new(a.at, t.kernel, a.choice.actor);
        let kept = filter::options(&seen, a.choice);
        if kept.len() < 2 {
            return Ok(picked);
        }
        self.st.tree_asks += 1;
        let round = a.at.meta().round;
        let key = self.seed.wrapping_mul(0x0001_0000_0001) ^ (a.index as u64).wrapping_mul(0x9e37_79b9);
        let eps = if round <= self.o.early_rounds { self.o.eps_early } else { self.o.eps };
        let noise = unit(key ^ 1) < eps;
        let played = if noise { kept[(mix(key ^ 2) % kept.len() as u64) as usize] } else { picked };
        self.st.noisy += u64::from(noise && played != picked);
        if self.in_game < self.o.cap && unit(key ^ 3) < self.o.rates[tk.index()] {
            if !aligned(a) {
                self.misaligned = Some(format!("decision {}: the wire options are not the core options", a.index));
                return Err("misaligned options".to_owned());
            }
            let t0 = Instant::now();
            let mut enc = self.enc.encode(&seen, a.choice);
            self.st.encode += t0.elapsed();
            if self.w.names.is_none() {
                self.w.names = Some(encode::column_names(&seen, a.choice));
            }
            // The rule bot's pick outside the filter (about 1 % of the activations): put it back into the kept set.
            if !enc.mask[picked] {
                enc.mask[picked] = true;
                self.st.put_back += 1;
            }
            let mut pi = vec![0.0; enc.k()];
            if smart {
                pi[picked] = 1.0;
            } else {
                self.st.no_target += 1;
            }
            let rel = encode::relative_seats(&seen, a.choice.actor);
            let mut rel4 = [0u8; SEATS];
            for (i, s) in rel.iter().take(SEATS).enumerate() {
                rel4[i] = s.0;
            }
            let meta = Meta {
                game: self.seed,
                seed: self.seed,
                round,
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
                played: u16::try_from(played).unwrap_or(u16::MAX),
                noise,
            };
            self.w.push(Record { enc, pi, meta });
            self.in_game += 1;
            self.st.by_kind[tk.index()] += 1;
        }
        Ok(played)
    }
}

fn parse_rates(text: &str) -> Result<[f64; 7], String> {
    let mut rates = RATES;
    for part in text.split(',').filter(|p| !p.is_empty()) {
        let (name, v) = part.split_once('=').ok_or_else(|| format!("rate {part}: want kind=p"))?;
        let k = TreeKind::ALL.iter().find(|k| k.name() == name).ok_or_else(|| format!("rate {part}: no kind {name}"))?;
        rates[k.index()] = v.parse().map_err(|e| format!("rate {part}: {e}"))?;
    }
    Ok(rates)
}

fn worker(id: usize, o: &Opts, next: &AtomicU64, end: u64, per_shard: u64, out: &Path, total: &Mutex<Stats>, started: Instant) {
    let mut g = Gen { o: o.clone(), w: ShardWriter::new(), enc: Encoder::default(), seed: 0, in_game: 0, misaligned: None, st: Stats::default() };
    let mut shard_first: Option<u64> = None;
    let mut in_shard = 0u64;
    let flush = |g: &mut Gen, first: u64| {
        let dir = out.join(format!("g{first:07}-w{id:02}"));
        match g.w.write(&dir) {
            Ok(n) => eprintln!("[{:.0} s] worker {id}: shard {} ({n} records)", started.elapsed().as_secs_f64(), dir.display()),
            Err(e) => eprintln!("worker {id}: write {}: {e}", dir.display()),
        }
    };
    loop {
        let seed = next.fetch_add(1, Ordering::Relaxed);
        if seed >= end {
            break;
        }
        let (t, mut pos) = match start(&ti4_mcts_lab::spec(seed)) {
            Ok(x) => x,
            Err(e) => {
                g.st.errors += 1;
                g.st.error_texts.push(format!("seed {seed}: {e}"));
                continue;
            }
        };
        g.seed = seed;
        g.in_game = 0;
        g.misaligned = None;
        g.st.games += 1;
        let end_ = ti4_mcts_lab::play(&t, &mut pos, &mut g, 6000, 12, |_, _| {});
        if let Some(e) = end_.error.clone() {
            g.w.drop_game();
            g.st.errors += 1;
            g.st.error_texts.push(format!("seed {seed}: {}", g.misaligned.clone().unwrap_or(e)));
            continue;
        }
        g.st.finished += u64::from(end_.finished);
        g.st.capped += u64::from(!end_.finished);
        *g.st.rounds.entry(end_.round).or_default() += 1;
        let (z, vp) = outcome(&end_.vp);
        for (i, f) in t.factions.iter().enumerate() {
            *g.st.wins.entry(f.clone()).or_default() += f64::from(z[i]);
            *g.st.vp.entry(f.clone()).or_default() += f64::from(vp[i]);
            *g.st.seats.entry(f.clone()).or_default() += 1;
        }
        g.w.end_game(&z, &vp);
        g.st.records += g.in_game as u64;
        shard_first.get_or_insert(seed);
        in_shard += 1;
        if in_shard >= per_shard {
            flush(&mut g, shard_first.take().unwrap_or(seed));
            in_shard = 0;
        }
    }
    if let Some(first) = shard_first {
        if !g.w.is_empty() {
            flush(&mut g, first);
        }
    }
    if let Ok(mut t) = total.lock() {
        t.add(&g.st);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let num = |flag: &str, d: f64| get(flag).and_then(|v| v.parse::<f64>().ok()).unwrap_or(d);
    let Some(out) = get("--out").map(PathBuf::from) else {
        eprintln!("--out DIR is required");
        std::process::exit(2);
    };
    let games = num("--games", 24.0) as u64;
    let first = num("--first", 100_000.0) as u64;
    let workers = (num("--workers", 4.0) as usize).max(1);
    let per_shard = (num("--per-shard", 100.0) as u64).max(1);
    let rates = match get("--rates").map_or(Ok(RATES), |t| parse_rates(&t)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let o = Opts { eps: num("--eps", 0.15), eps_early: num("--eps-early", 0.3), early_rounds: num("--early-rounds", 2.0) as u32, rates, cap: num("--cap", 200.0) as usize };
    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("{}: {e}", out.display());
        std::process::exit(1);
    }
    let next = AtomicU64::new(first);
    let total = Mutex::new(Stats::default());
    let started = Instant::now();
    std::thread::scope(|s| {
        for id in 0..workers {
            let (o, next, total, out) = (&o, &next, &total, &out);
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn_scoped(s, move || worker(id, o, next, first + games, per_shard, out, total, started))
                .expect("a worker thread starts");
        }
    });
    let st = total.into_inner().unwrap_or_default();
    let secs = started.elapsed().as_secs_f64();
    let mut j = String::from("{\n");
    let _ = writeln!(j, "  \"first\": {first}, \"games\": {games}, \"workers\": {workers}, \"seconds\": {secs:.1},");
    let _ = writeln!(j, "  \"eps\": {}, \"eps_early\": {}, \"early_rounds\": {}, \"cap\": {},", o.eps, o.eps_early, o.early_rounds, o.cap);
    let _ = writeln!(j, "  \"rates\": {{{}}},", TreeKind::ALL.iter().map(|k| format!("\"{}\": {}", k.name(), o.rates[k.index()])).collect::<Vec<_>>().join(", "));
    let _ = writeln!(j, "  \"played\": {}, \"finished\": {}, \"round_capped\": {}, \"errors\": {},", st.games, st.finished, st.capped, st.errors);
    let _ = writeln!(j, "  \"records\": {}, \"tree_asks\": {}, \"noisy_plays\": {}, \"picks_put_back\": {}, \"no_policy_target\": {},", st.records, st.tree_asks, st.noisy, st.put_back, st.no_target);
    let _ = writeln!(j, "  \"records_by_kind\": {{{}}},", TreeKind::ALL.iter().map(|k| format!("\"{}\": {}", k.name(), st.by_kind[k.index()])).collect::<Vec<_>>().join(", "));
    let _ = writeln!(j, "  \"encode_us_per_record\": {:.1},", st.encode.as_secs_f64() * 1e6 / st.records.max(1) as f64);
    let _ = writeln!(j, "  \"win_share\": {{{}}},", st.wins.iter().map(|(k, v)| format!("\"{k}\": {:.4}", v / st.seats.get(k).copied().unwrap_or(1).max(1) as f64)).collect::<Vec<_>>().join(", "));
    let _ = writeln!(j, "  \"mean_vp\": {{{}}},", st.vp.iter().map(|(k, v)| format!("\"{k}\": {:.3}", v / st.seats.get(k).copied().unwrap_or(1).max(1) as f64)).collect::<Vec<_>>().join(", "));
    let _ = writeln!(j, "  \"rounds\": {{{}}},", st.rounds.iter().map(|(k, v)| format!("\"{k}\": {v}")).collect::<Vec<_>>().join(", "));
    let _ = writeln!(j, "  \"errors_sample\": [{}]", st.error_texts.iter().take(20).map(|e| format!("{e:?}")).collect::<Vec<_>>().join(", "));
    j.push_str("}\n");
    let path = out.join(format!("summary-{first}.json"));
    if let Err(e) = std::fs::write(&path, &j) {
        eprintln!("{}: {e}", path.display());
    }
    print!("{j}");
}
