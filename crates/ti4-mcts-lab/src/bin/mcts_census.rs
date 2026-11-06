//! Lanes M0 + M1 of docs/MCTS_BOT_PLAN.md: engine speed and the ask census over rule-bot 4p Learn-to-play games
//! (Sol, Letnev, Xxcha, Jol-Nar, all 24 seatings cycled over the seeds).
//!
//! ```text
//! mcts_census [--games N] [--first SEED] [--workers W] [--replay-every K] [--json PATH]
//! ```
//!
//! Prints markdown tables (docs/MCTS_M0_M1.md) and writes the raw numbers as JSON.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use ti4_engine::generated::IdStr;
use ti4_engine_core::choice::{strategy, tactical, Choice, Opt};
use ti4_engine_core::seen::Seen;
use ti4_engine_core::state::RollPurpose;
use ti4_mcts_features::{filter, kind};
use ti4_mcts_lab::{rule_bot, start, Asked, Decide, Pos, Stepped, Table};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const MAX_STEPS: usize = 6000;
const MAX_ROUNDS: u32 = 12;

#[derive(Default, Clone)]
struct KindStat {
    n: u64,
    smart: u64,
    tree: bool,
    opts: BTreeMap<usize, u64>,
    filtered: BTreeMap<usize, u64>,
    /// Asks with one option after the filter (never a node).
    single: u64,
    view_ns: u128,
    bot_ns: u128,
    filter_ns: u128,
    bot_us: Vec<u32>,
    /// Rendered option count differs from the core's.
    misaligned: u64,
    /// An option whose rendered id does not name the core option (checked for Activate and Draft).
    misnamed: u64,
    /// Activation filter: nothing survived (all kept); the rule bot's pick was dropped by the filter; options kept per
    /// reason (move, produce, shoot); the rule bot's picks per reason.
    passthrough: u64,
    pick_dropped: u64,
    reasons: [u64; 3],
    pick_reasons: [u64; 3],
}

impl KindStat {
    fn merge(&mut self, o: &Self) {
        self.n += o.n;
        self.smart += o.smart;
        self.tree |= o.tree;
        for (k, v) in &o.opts {
            *self.opts.entry(*k).or_default() += v;
        }
        for (k, v) in &o.filtered {
            *self.filtered.entry(*k).or_default() += v;
        }
        self.single += o.single;
        self.view_ns += o.view_ns;
        self.bot_ns += o.bot_ns;
        self.filter_ns += o.filter_ns;
        self.bot_us.extend_from_slice(&o.bot_us);
        self.misaligned += o.misaligned;
        self.misnamed += o.misnamed;
        self.passthrough += o.passthrough;
        self.pick_dropped += o.pick_dropped;
        for i in 0..3 {
            self.reasons[i] += o.reasons[i];
            self.pick_reasons[i] += o.pick_reasons[i];
        }
    }
}

#[derive(Default, Clone)]
struct DiceStat {
    records: u64,
    dice: u64,
    no_threshold: u64,
    rerolled: u64,
    by_n: BTreeMap<usize, u64>,
}

#[derive(Default, Clone)]
struct Stats {
    games: u64,
    finished: u64,
    errors: Vec<String>,
    rounds: BTreeMap<u32, u64>,
    steps: u64,
    decisions: u64,
    loop_ns: u128,
    step_ns: Vec<u32>,
    env_ns: u128,
    fork_ns: u128,
    forks: u64,
    clone_ns: u128,
    replay_ns: u128,
    replays: u64,
    replay_asks: u64,
    kinds: BTreeMap<String, KindStat>,
    /// Per ask kind per game: list of counts (for the per-game mean and range).
    per_game: BTreeMap<String, Vec<u64>>,
    by_phase: BTreeMap<String, u64>,
    tree_per_game: Vec<u64>,
    tree_per_seat_game: Vec<u64>,
    /// Delegated asks between two tree asks (all seats), histogram.
    seg_len: BTreeMap<u64, u64>,
    /// Same-seat segments: delegated asks between two tree asks of one seat... (the other seats' tree asks count as tree asks).
    seg_wall_us: Vec<u32>,
    seg_bot_us: Vec<u32>,
    dice: BTreeMap<String, DiceStat>,
    rolls_per_game: Vec<u64>,
    dice_per_game: Vec<u64>,
    move_seqs: u64,
    move_seq_len: BTreeMap<usize, u64>,
    move_canonical: u64,
    move_reordered_ok: u64,
    move_reordered_lost: u64,
    move_replay_failed: u64,
    winners: BTreeMap<String, u64>,
    vp_sum: BTreeMap<String, i64>,
}

impl Stats {
    fn merge(&mut self, o: &Self) {
        self.games += o.games;
        self.finished += o.finished;
        self.errors.extend(o.errors.iter().cloned());
        for (k, v) in &o.rounds {
            *self.rounds.entry(*k).or_default() += v;
        }
        self.steps += o.steps;
        self.decisions += o.decisions;
        self.loop_ns += o.loop_ns;
        self.step_ns.extend_from_slice(&o.step_ns);
        self.env_ns += o.env_ns;
        self.fork_ns += o.fork_ns;
        self.forks += o.forks;
        self.clone_ns += o.clone_ns;
        self.replay_ns += o.replay_ns;
        self.replays += o.replays;
        self.replay_asks += o.replay_asks;
        for (k, v) in &o.kinds {
            self.kinds.entry(k.clone()).or_default().merge(v);
        }
        for (k, v) in &o.per_game {
            self.per_game.entry(k.clone()).or_default().extend_from_slice(v);
        }
        for (k, v) in &o.by_phase {
            *self.by_phase.entry(k.clone()).or_default() += v;
        }
        self.tree_per_game.extend_from_slice(&o.tree_per_game);
        self.tree_per_seat_game.extend_from_slice(&o.tree_per_seat_game);
        for (k, v) in &o.seg_len {
            *self.seg_len.entry(*k).or_default() += v;
        }
        self.seg_wall_us.extend_from_slice(&o.seg_wall_us);
        self.seg_bot_us.extend_from_slice(&o.seg_bot_us);
        for (k, v) in &o.dice {
            let d = self.dice.entry(k.clone()).or_default();
            d.records += v.records;
            d.dice += v.dice;
            d.no_threshold += v.no_threshold;
            d.rerolled += v.rerolled;
            for (n, c) in &v.by_n {
                *d.by_n.entry(*n).or_default() += c;
            }
        }
        self.rolls_per_game.extend_from_slice(&o.rolls_per_game);
        self.dice_per_game.extend_from_slice(&o.dice_per_game);
        self.move_seqs += o.move_seqs;
        for (k, v) in &o.move_seq_len {
            *self.move_seq_len.entry(*k).or_default() += v;
        }
        self.move_canonical += o.move_canonical;
        self.move_reordered_ok += o.move_reordered_ok;
        self.move_reordered_lost += o.move_reordered_lost;
        self.move_replay_failed += o.move_replay_failed;
        for (k, v) in &o.winners {
            *self.winners.entry(k.clone()).or_default() += v;
        }
        for (k, v) in &o.vp_sum {
            *self.vp_sum.entry(k.clone()).or_default() += v;
        }
    }
}

/// A ship's identity in a movement step (the option without its index, capacity and route).
type ShipKey = (u16, u16, bool, bool, bool);

fn ship_key(o: &Opt) -> Option<ShipKey> {
    match o {
        Opt::Tactical(tactical::Opt::Move { origin, kind, damaged, gd, ionian, .. }) => Some((origin.index() as u16, kind.index() as u16, *damaged, *gd, *ionian)),
        _ => None,
    }
}

/// The canonical order of the plan (2.4): capacity providers first, then origin, kind, damaged.
fn canon_key(o: &Opt) -> Option<(bool, u16, u16, bool)> {
    match o {
        Opt::Tactical(tactical::Opt::Move { origin, kind, damaged, capacity, .. }) => Some((*capacity <= 0, origin.index() as u16, kind.index() as u16, *damaged)),
        _ => None,
    }
}

/// One movement step of the rule bot: the moves it made, in order.
struct MoveSeq {
    actor: u8,
    /// The decision index of the first Move ask, and the step it was asked in.
    first_index: usize,
    first_step: usize,
    moves: Vec<Opt>,
}

struct Recorder {
    stats: Stats,
    game: BTreeMap<String, u64>,
    picks: Vec<usize>,
    last_tree: Option<Instant>,
    seg_delegated: u64,
    seg_bot: Duration,
    tree_count: u64,
    tree_by_seat: [u64; 8],
    moving: Option<MoveSeq>,
    done_seqs: Vec<MoveSeq>,
    step_now: usize,
}

fn name_matches(rendered: &str, o: &Opt) -> Option<bool> {
    match o {
        Opt::Tactical(tactical::Opt::Activate(s)) => Some(rendered == s.as_str()),
        Opt::Strategy(strategy::Opt::Card(c)) => Some(rendered == c.as_str()),
        Opt::Strategy(strategy::Opt::System(s)) => Some(rendered == s.as_str()),
        _ => None,
    }
}

impl Recorder {
    fn new() -> Self {
        Self { stats: Stats::default(), game: BTreeMap::new(), picks: Vec::new(), last_tree: None, seg_delegated: 0, seg_bot: Duration::ZERO, tree_count: 0, tree_by_seat: [0; 8], moving: None, done_seqs: Vec::new(), step_now: 0 }
    }

    fn start_game(&mut self) {
        self.game.clear();
        self.picks.clear();
        self.last_tree = None;
        self.seg_delegated = 0;
        self.seg_bot = Duration::ZERO;
        self.tree_count = 0;
        self.tree_by_seat = [0; 8];
        self.moving = None;
        self.done_seqs.clear();
    }
}

impl Decide for Recorder {
    fn decide(&mut self, t: &Table, a: &Asked<'_>) -> Result<usize, String> {
        let choice: &Choice = a.choice;
        let name = kind::ask_name(&choice.ask);
        let tree = kind::tree_kind_of(choice);
        let raw = choice.opts.len();
        // The hard filter (on the actor's view of the position).
        let tf = Instant::now();
        let kept = if tree == Some(kind::TreeKind::Activate) {
            let seen = Seen::new(a.at, t.kernel, choice.actor);
            filter::activate(&seen, choice)
        } else {
            None
        };
        let filtered = kept.as_ref().map_or(raw, filter::Kept::kept);
        let filter_ns = tf.elapsed().as_nanos();
        let (picked, smart, view, bot) = rule_bot(t, a);
        let is_node = tree.is_some() && filtered >= 2;
        // Segments: between two tree nodes (all seats).
        if is_node {
            if let Some(last) = self.last_tree {
                let wall = last.elapsed();
                *self.stats.seg_len.entry(self.seg_delegated).or_default() += 1;
                self.stats.seg_wall_us.push(u32::try_from(wall.as_micros()).unwrap_or(u32::MAX));
                self.stats.seg_bot_us.push(u32::try_from(self.seg_bot.as_micros()).unwrap_or(u32::MAX));
            }
            self.seg_delegated = 0;
            self.seg_bot = Duration::ZERO;
            self.tree_count += 1;
            self.tree_by_seat[usize::from(choice.actor.0).min(7)] += 1;
        } else {
            self.seg_delegated += 1;
            self.seg_bot += view + bot;
        }
        let st = self.stats.kinds.entry(name.clone()).or_default();
        st.n += 1;
        st.smart += u64::from(smart);
        st.tree = tree.is_some();
        *st.opts.entry(raw).or_default() += 1;
        *st.filtered.entry(filtered).or_default() += 1;
        st.single += u64::from(filtered < 2);
        st.view_ns += view.as_nanos();
        st.bot_ns += bot.as_nanos();
        st.filter_ns += filter_ns;
        if let Some(k) = &kept {
            st.passthrough += u64::from(k.passthrough);
            st.pick_dropped += u64::from(!k.keep[picked]);
            for (bit, slot) in [(filter::MOVE, 0), (filter::PRODUCE, 1), (filter::SHOOT, 2)] {
                st.reasons[slot] += k.why.iter().filter(|w| **w & bit != 0).count() as u64;
            }
            if let Some(w) = k.why.get(picked) {
                for (bit, slot) in [(filter::MOVE, 0), (filter::PRODUCE, 1), (filter::SHOOT, 2)] {
                    st.pick_reasons[slot] += u64::from(*w & bit != 0);
                }
            }
        }
        st.bot_us.push(u32::try_from((view + bot).as_micros()).unwrap_or(u32::MAX));
        if a.rendered.options.len() != raw {
            st.misaligned += 1;
        } else if choice.opts.iter().zip(&a.rendered.options).any(|(o, r)| name_matches(&r.id, o) == Some(false)) {
            st.misnamed += 1;
        }
        *self.game.entry(name).or_default() += 1;
        *self.stats.by_phase.entry(format!("{:?}", a.at.meta().phase)).or_default() += 1;
        // Movement steps.
        if tree == Some(kind::TreeKind::Move) {
            let o = choice.opts.as_slice()[picked];
            let seq = self.moving.get_or_insert_with(|| MoveSeq { actor: choice.actor.0, first_index: a.index, first_step: self.step_now, moves: Vec::new() });
            if matches!(o, Opt::Tactical(tactical::Opt::DoneMoving)) {
                if let Some(seq) = self.moving.take() {
                    self.done_seqs.push(seq);
                }
            } else {
                seq.moves.push(o);
            }
        }
        if self.picks.len() == a.index {
            self.picks.push(picked);
        }
        if is_node {
            self.last_tree = Some(Instant::now());
        }
        Ok(picked)
    }
}

/// The canonical replay of one movement step: the answers before the first move as recorded, the moves in canonical order
/// (always the smallest offered key among the ships still to move), everything else by the rule bot.
struct Canonical<'a> {
    picks: &'a [usize],
    seq: &'a MoveSeq,
    left: Vec<ShipKey>,
    last: Option<(bool, u16, u16, bool)>,
    outcome: Option<bool>,
    asks: u64,
}

impl Decide for Canonical<'_> {
    fn decide(&mut self, t: &Table, a: &Asked<'_>) -> Result<usize, String> {
        self.asks += 1;
        if self.outcome.is_some() {
            return Err("replay over".to_owned());
        }
        if a.index < self.seq.first_index {
            return self.picks.get(a.index).copied().ok_or_else(|| "no recorded pick".to_owned());
        }
        let is_move = matches!(a.choice.ask, ti4_engine_core::choice::Ask::Tactical(tactical::Ask::Move { .. })) && a.choice.actor.0 == self.seq.actor;
        if !is_move {
            return Ok(rule_bot(t, a).0);
        }
        let opts = a.choice.opts.as_slice();
        if self.left.is_empty() {
            self.outcome = Some(true);
            return opts.iter().position(|o| matches!(o, Opt::Tactical(tactical::Opt::DoneMoving))).ok_or_else(|| "no done".to_owned());
        }
        // The offered ships still to move, smallest canonical key first.
        let best = opts
            .iter()
            .enumerate()
            .filter(|(_, o)| ship_key(o).is_some_and(|k| self.left.contains(&k)))
            .min_by_key(|(_, o)| canon_key(o));
        match best {
            None => {
                self.outcome = Some(false);
                Err("a ship to move is not offered".to_owned())
            }
            Some((i, o)) => {
                let key = canon_key(o);
                if self.last.is_some_and(|l| key < Some(l)) {
                    self.outcome = Some(false);
                    return Err("only a non-canonical order reaches the set".to_owned());
                }
                self.last = key;
                let sk = ship_key(o).unwrap_or_default();
                if let Some(at) = self.left.iter().position(|k| *k == sk) {
                    self.left.remove(at);
                }
                Ok(i)
            }
        }
    }
}

fn pct(v: &mut [u32], p: f64) -> u32 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    let i = ((v.len() as f64 - 1.0) * p).round() as usize;
    v[i.min(v.len() - 1)]
}

fn purpose_name(p: &RollPurpose) -> String {
    match p {
        RollPurpose::Ability(_) => "Ability".to_owned(),
        RollPurpose::Other(_) => "Other".to_owned(),
        other => format!("{other:?}"),
    }
}

fn play_one(seed: u64, replay_every: usize, rec: &mut Recorder) {
    let spec = ti4_mcts_lab::spec(seed);
    let (t, mut pos) = match start(&spec) {
        Ok(x) => x,
        Err(e) => {
            rec.stats.errors.push(format!("seed {seed}: setup: {e}"));
            return;
        }
    };
    rec.start_game();
    ti4_rule_bot::backward::set_memory(ti4_rule_bot::backward::Memory::default());
    // Step-start snapshots (for the canonical replays) and the M0 fork + replay sample.
    let mut snaps: Vec<(Pos, usize)> = Vec::new();
    let mut index = 0usize;
    let mut steps = 0usize;
    let mut error = None;
    let started = Instant::now();
    let mut excluded = Duration::ZERO;
    while steps < MAX_STEPS && pos.round() <= MAX_ROUNDS {
        let tf = Instant::now();
        let snap = pos.fork(&t);
        let fork = tf.elapsed();
        let tc = Instant::now();
        let core_copy = pos.core.clone();
        let clone = tc.elapsed();
        drop(core_copy);
        rec.stats.fork_ns += fork.as_nanos();
        rec.stats.clone_ns += clone.as_nanos();
        rec.stats.forks += 1;
        // M0: fork + re-run the step on the fork with the rule bot, every `replay_every`-th step (the bot's memory saved around it).
        if replay_every > 0 && steps % replay_every == 0 {
            let memory = ti4_rule_bot::backward::memory();
            let tr = Instant::now();
            let mut fork_pos = pos.fork(&t);
            let mut i2 = index;
            let mut bot = ti4_mcts_lab::RuleBot;
            let _ = ti4_mcts_lab::step(&t, &mut fork_pos, &mut bot, &mut i2);
            rec.stats.replay_ns += tr.elapsed().as_nanos();
            rec.stats.replays += 1;
            rec.stats.replay_asks += (i2 - index) as u64;
            ti4_rule_bot::backward::set_memory(memory);
        }
        excluded += tf.elapsed();
        snaps.push((snap, index));
        rec.step_now = steps;
        let ts = Instant::now();
        ti4_mcts_lab::env_ns_reset();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ti4_mcts_lab::step(&t, &mut pos, rec, &mut index)));
        let dt = ts.elapsed();
        rec.stats.env_ns += ti4_mcts_lab::env_ns_take();
        match r {
            Ok(Ok(Stepped::Finished)) => break,
            Ok(Ok(Stepped::Played { .. })) => {
                steps += 1;
                rec.stats.step_ns.push(u32::try_from(dt.as_nanos()).unwrap_or(u32::MAX));
            }
            Ok(Err(e)) => {
                error = Some(e);
                break;
            }
            Err(_) => {
                error = Some("panic".to_owned());
                break;
            }
        }
    }
    rec.stats.loop_ns += (started.elapsed() - excluded).as_nanos();
    // Canonical move-order replays (outside the timed loop).
    let seqs = std::mem::take(&mut rec.done_seqs);
    for seq in &seqs {
        rec.stats.move_seqs += 1;
        *rec.stats.move_seq_len.entry(seq.moves.len()).or_default() += 1;
        let keys: Vec<_> = seq.moves.iter().map(canon_key).collect();
        if keys.windows(2).all(|w| w[0] <= w[1]) {
            rec.stats.move_canonical += 1;
            continue;
        }
        let Some((snap, snap_index)) = snaps.get(seq.first_step) else {
            rec.stats.move_replay_failed += 1;
            continue;
        };
        let memory = ti4_rule_bot::backward::memory();
        let mut p2 = snap.fork(&t);
        let mut i2 = *snap_index;
        let mut can = Canonical { picks: &rec.picks, seq, left: seq.moves.iter().filter_map(ship_key).collect(), last: None, outcome: None, asks: 0 };
        for _ in 0..16 {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ti4_mcts_lab::step(&t, &mut p2, &mut can, &mut i2)));
            if can.outcome.is_some() || !matches!(r, Ok(Ok(Stepped::Played { .. }))) {
                break;
            }
        }
        ti4_rule_bot::backward::set_memory(memory);
        match can.outcome {
            Some(true) => rec.stats.move_reordered_ok += 1,
            Some(false) => rec.stats.move_reordered_lost += 1,
            None => rec.stats.move_replay_failed += 1,
        }
    }
    // The dice of the game (the committed ledger).
    let (mut rolls, mut dice) = (0u64, 0u64);
    for r in pos.core.ledger.dice.iter() {
        let d = rec.stats.dice.entry(purpose_name(&r.purpose)).or_default();
        d.records += 1;
        d.dice += r.faces.len() as u64;
        d.no_threshold += u64::from(r.hits_on.is_none());
        d.rerolled += u64::from(r.rerolled != 0);
        *d.by_n.entry(r.faces.len()).or_default() += 1;
        rolls += 1;
        dice += r.faces.len() as u64;
    }
    rec.stats.rolls_per_game.push(rolls);
    rec.stats.dice_per_game.push(dice);
    rec.stats.games += 1;
    rec.stats.finished += u64::from(pos.finished());
    *rec.stats.rounds.entry(pos.round()).or_default() += 1;
    rec.stats.steps += steps as u64;
    rec.stats.decisions += index as u64;
    if let Some(e) = error {
        rec.stats.errors.push(format!("seed {seed}: {e}"));
    }
    for k in rec.stats.kinds.keys().cloned().collect::<Vec<_>>() {
        let n = rec.game.get(&k).copied().unwrap_or(0);
        rec.stats.per_game.entry(k).or_default().push(n);
    }
    rec.stats.tree_per_game.push(rec.tree_count);
    for s in 0..4 {
        rec.stats.tree_per_seat_game.push(rec.tree_by_seat[s]);
    }
    let vp = pos.vp();
    for (i, f) in t.factions.iter().enumerate() {
        *rec.stats.vp_sum.entry(f.clone()).or_default() += i64::from(vp.get(i).copied().unwrap_or(0));
    }
    if pos.finished() {
        if let Some((i, _)) = vp.iter().enumerate().max_by_key(|(_, v)| **v) {
            *rec.stats.winners.entry(t.factions[i].clone()).or_default() += 1;
        }
    }
}

fn mean(v: &[u64]) -> f64 {
    if v.is_empty() { 0.0 } else { v.iter().sum::<u64>() as f64 / v.len() as f64 }
}

fn hist_mean(h: &BTreeMap<usize, u64>) -> (f64, usize, usize) {
    let n: u64 = h.values().sum();
    let s: u64 = h.iter().map(|(k, v)| *k as u64 * v).sum();
    let lo = h.keys().next().copied().unwrap_or(0);
    let hi = h.keys().next_back().copied().unwrap_or(0);
    (if n == 0 { 0.0 } else { s as f64 / n as f64 }, lo, hi)
}

fn hist_pct(h: &BTreeMap<usize, u64>, p: f64) -> usize {
    let n: u64 = h.values().sum();
    let want = (n as f64 * p).ceil() as u64;
    let mut acc = 0;
    for (k, v) in h {
        acc += v;
        if acc >= want.max(1) {
            return *k;
        }
    }
    0
}

#[allow(clippy::too_many_lines)]
fn report(s: &mut Stats, workers: usize, wall: Duration) -> (String, Value) {
    let mut out = String::new();
    let g = s.games.max(1) as f64;
    let _ = writeln!(out, "games {} (finished {}), errors {}, workers {workers}, wall {:.1} s", s.games, s.finished, s.errors.len(), wall.as_secs_f64());
    for e in s.errors.iter().take(10) {
        let _ = writeln!(out, "  error: {e}");
    }
    let _ = writeln!(out, "rounds at the end: {:?}", s.rounds);
    let _ = writeln!(out, "winners: {:?}; mean VP: {:?}", s.winners, s.vp_sum.iter().map(|(k, v)| (k.clone(), format!("{:.2}", *v as f64 / g))).collect::<Vec<_>>());
    // M0.
    let loop_s = s.loop_ns as f64 / 1e9;
    let bot_ns: u128 = s.kinds.values().map(|k| k.view_ns + k.bot_ns).sum();
    let view_ns: u128 = s.kinds.values().map(|k| k.view_ns).sum();
    let filter_ns: u128 = s.kinds.values().map(|k| k.filter_ns).sum();
    let engine_ns = s.loop_ns.saturating_sub(bot_ns + filter_ns);
    let p50 = pct(&mut s.step_ns, 0.5);
    let p99 = pct(&mut s.step_ns, 0.99);
    let _ = writeln!(out, "\n## M0\n");
    let _ = writeln!(out, "| quantity | value |\n|---|---|");
    let _ = writeln!(out, "| steps / game | {:.0} |", s.steps as f64 / g);
    let _ = writeln!(out, "| decisions (asks) / game | {:.0} |", s.decisions as f64 / g);
    let _ = writeln!(out, "| loop time / game (ms) | {:.0} |", loop_s * 1e3 / g);
    let _ = writeln!(out, "| of it engine (ms) | {:.0} ({:.0} %) |", engine_ns as f64 / 1e6 / g, engine_ns as f64 * 100.0 / s.loop_ns.max(1) as f64);
    let _ = writeln!(out, "| of it env build (ms) | {:.1} |", s.env_ns as f64 / 1e6 / g);
    let _ = writeln!(out, "| of it rule bot incl. view (ms) | {:.0} ({:.0} %) |", bot_ns as f64 / 1e6 / g, bot_ns as f64 * 100.0 / s.loop_ns.max(1) as f64);
    let _ = writeln!(out, "| of it SeenView build (ms) | {:.1} |", view_ns as f64 / 1e6 / g);
    let _ = writeln!(out, "| of it Activate filter (ms) | {:.1} |", filter_ns as f64 / 1e6 / g);
    let _ = writeln!(out, "| steps / s (whole loop) | {:.0} |", s.steps as f64 / loop_s.max(1e-9));
    let _ = writeln!(out, "| engine steps / s (bot time removed) | {:.0} |", s.steps as f64 / (engine_ns as f64 / 1e9).max(1e-9));
    let _ = writeln!(out, "| asks / s (whole loop) | {:.0} |", s.decisions as f64 / loop_s.max(1e-9));
    let _ = writeln!(out, "| step wall p50 / p99 (us) | {:.0} / {:.0} |", f64::from(p50) / 1e3, f64::from(p99) / 1e3);
    let _ = writeln!(out, "| `Pos::fork` (core clone + render rebuild) (us) | {:.2} |", s.fork_ns as f64 / 1e3 / s.forks.max(1) as f64);
    let _ = writeln!(out, "| `CoreState::clone` alone (us) | {:.2} |", s.clone_ns as f64 / 1e3 / s.forks.max(1) as f64);
    let _ = writeln!(out, "| fork + replay one step with the rule bot (us) | {:.0} ({} samples, {:.2} asks each) |", s.replay_ns as f64 / 1e3 / s.replays.max(1) as f64, s.replays, s.replay_asks as f64 / s.replays.max(1) as f64);
    let tree_total: u64 = s.tree_per_game.iter().sum();
    let _ = writeln!(out, "| tree nodes / game (all seats) | {:.0} |", mean(&s.tree_per_game));
    let _ = writeln!(out, "| tree nodes / seat / game | {:.0} |", mean(&s.tree_per_seat_game));
    let _ = writeln!(out, "| loop time per tree node (ms): the cost of one segment | {:.3} |", loop_s * 1e3 / tree_total.max(1) as f64);
    let _ = writeln!(out, "| of it engine per tree node (ms) | {:.3} |", engine_ns as f64 / 1e6 / tree_total.max(1) as f64);
    let mut sw = s.seg_wall_us.clone();
    let mut sb = s.seg_bot_us.clone();
    let sw_mean = mean(&sw.iter().map(|x| u64::from(*x)).collect::<Vec<_>>());
    let sb_mean = mean(&sb.iter().map(|x| u64::from(*x)).collect::<Vec<_>>());
    let _ = writeln!(out, "| segment wall between tree nodes: mean / p50 / p90 / p99 (us) | {:.0} / {} / {} / {} |", sw_mean, pct(&mut sw, 0.5), pct(&mut sw, 0.9), pct(&mut sw, 0.99));
    let _ = writeln!(out, "| of it delegated rule-bot answers: mean / p50 / p90 (us) | {:.0} / {} / {} |", sb_mean, pct(&mut sb, 0.5), pct(&mut sb, 0.9));
    // Rule bot per ask kind.
    let _ = writeln!(out, "\n## Asks per kind\n");
    let _ = writeln!(out, "| ask kind | tree | per game | share smart=false | opts mean (min-max) | p90 opts | after filter mean (min-max) | p90 filt | 1-opt share | bot+view us mean | p50 | p99 | misaligned | misnamed |");
    let _ = writeln!(out, "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    let mut rows: Vec<(&String, &mut KindStat)> = s.kinds.iter_mut().collect();
    rows.sort_by(|a, b| b.1.n.cmp(&a.1.n));
    let mut kinds_json = serde_json::Map::new();
    for (name, k) in rows {
        let (om, olo, ohi) = hist_mean(&k.opts);
        let (fm, flo, fhi) = hist_mean(&k.filtered);
        let mean_us = (k.bot_ns + k.view_ns) as f64 / 1e3 / k.n.max(1) as f64;
        let p50 = pct(&mut k.bot_us, 0.5);
        let p99 = pct(&mut k.bot_us, 0.99);
        let _ = writeln!(
            out,
            "| {name} | {} | {:.1} | {:.0} % | {om:.1} ({olo}-{ohi}) | {} | {fm:.1} ({flo}-{fhi}) | {} | {:.0} % | {mean_us:.0} | {p50} | {p99} | {} | {} |",
            if k.tree { "tree" } else { "" },
            k.n as f64 / g,
            (k.n - k.smart) as f64 * 100.0 / k.n.max(1) as f64,
            hist_pct(&k.opts, 0.9),
            hist_pct(&k.filtered, 0.9),
            k.single as f64 * 100.0 / k.n.max(1) as f64,
            k.misaligned,
            k.misnamed
        );
        kinds_json.insert(name.clone(), json!({"n": k.n, "smart": k.smart, "tree": k.tree, "opts": k.opts.iter().map(|(a, b)| (a.to_string(), json!(b))).collect::<serde_json::Map<_, _>>(), "filtered": k.filtered.iter().map(|(a, b)| (a.to_string(), json!(b))).collect::<serde_json::Map<_, _>>(), "single": k.single, "bot_ns": k.bot_ns, "view_ns": k.view_ns, "filter_ns": k.filter_ns, "misaligned": k.misaligned, "misnamed": k.misnamed, "passthrough": k.passthrough, "pick_dropped": k.pick_dropped, "reasons": k.reasons, "pick_reasons": k.pick_reasons, "bot_us_p50": p50, "bot_us_p99": p99}));
    }
    for (name, k) in &s.kinds {
        if k.reasons.iter().sum::<u64>() > 0 || k.passthrough > 0 {
            let _ = writeln!(
                out,
                "\nfilter {name}: passthrough {:.1} %, rule-bot pick dropped by the filter {:.2} % ({}), kept options per ask by reason move {:.1} / produce {:.1} / shoot {:.1}; rule-bot picks with reason move {:.1} % / produce {:.1} % / shoot {:.1} %",
                k.passthrough as f64 * 100.0 / k.n.max(1) as f64,
                k.pick_dropped as f64 * 100.0 / k.n.max(1) as f64,
                k.pick_dropped,
                k.reasons[0] as f64 / k.n.max(1) as f64,
                k.reasons[1] as f64 / k.n.max(1) as f64,
                k.reasons[2] as f64 / k.n.max(1) as f64,
                k.pick_reasons[0] as f64 * 100.0 / k.n.max(1) as f64,
                k.pick_reasons[1] as f64 * 100.0 / k.n.max(1) as f64,
                k.pick_reasons[2] as f64 * 100.0 / k.n.max(1) as f64
            );
        }
    }
    let _ = writeln!(out, "\nasks by phase (per game): {:?}", s.by_phase.iter().map(|(k, v)| (k.clone(), format!("{:.0}", *v as f64 / g))).collect::<Vec<_>>());
    // Segment lengths.
    let total_seg: u64 = s.seg_len.values().sum();
    let _ = writeln!(out, "\n## Delegated asks between two tree nodes\n\n| delegated asks | share |\n|---|---|");
    let mut acc = 0u64;
    let mut buckets: BTreeMap<String, u64> = BTreeMap::new();
    for (k, v) in &s.seg_len {
        let b = match *k {
            0 => "0".to_owned(),
            1 => "1".to_owned(),
            2 => "2".to_owned(),
            3..=4 => "3-4".to_owned(),
            5..=8 => "5-8".to_owned(),
            9..=16 => "9-16".to_owned(),
            17..=32 => "17-32".to_owned(),
            _ => "33+".to_owned(),
        };
        *buckets.entry(b).or_default() += v;
        acc += k * v;
    }
    for b in ["0", "1", "2", "3-4", "5-8", "9-16", "17-32", "33+"] {
        let v = buckets.get(b).copied().unwrap_or(0);
        let _ = writeln!(out, "| {b} | {:.1} % |", v as f64 * 100.0 / total_seg.max(1) as f64);
    }
    let _ = writeln!(out, "\nmean delegated asks per segment {:.2}", acc as f64 / total_seg.max(1) as f64);
    // Chance.
    let _ = writeln!(out, "\n## Dice (committed roll records of the ledger)\n");
    let _ = writeln!(out, "rolls (requests) per game {:.0}, dice per game {:.0}\n", mean(&s.rolls_per_game), mean(&s.dice_per_game));
    let _ = writeln!(out, "| purpose | records / game | dice / record | no threshold (faces read) | rerolled | dice-count histogram |\n|---|---|---|---|---|---|");
    for (p, d) in &s.dice {
        let _ = writeln!(
            out,
            "| {p} | {:.1} | {:.2} | {:.1} % | {:.1} % | {} |",
            d.records as f64 / g,
            d.dice as f64 / d.records.max(1) as f64,
            d.no_threshold as f64 * 100.0 / d.records.max(1) as f64,
            d.rerolled as f64 * 100.0 / d.records.max(1) as f64,
            d.by_n.iter().map(|(n, c)| format!("{n}:{c}")).collect::<Vec<_>>().join(" ")
        );
    }
    // Movement order.
    let _ = writeln!(out, "\n## Canonical move order\n");
    let _ = writeln!(out, "movement steps {} ({:.1} / game); length histogram {:?}", s.move_seqs, s.move_seqs as f64 / g, s.move_seq_len);
    let _ = writeln!(
        out,
        "already canonical {} ({:.1} %), reordered and reachable {} ({:.1} %), reachable only out of canonical order {} ({:.2} %), replay inconclusive {}",
        s.move_canonical,
        s.move_canonical as f64 * 100.0 / s.move_seqs.max(1) as f64,
        s.move_reordered_ok,
        s.move_reordered_ok as f64 * 100.0 / s.move_seqs.max(1) as f64,
        s.move_reordered_lost,
        s.move_reordered_lost as f64 * 100.0 / s.move_seqs.max(1) as f64,
        s.move_replay_failed
    );
    let value = json!({
        "games": s.games, "finished": s.finished, "errors": s.errors, "steps": s.steps, "decisions": s.decisions,
        "loop_ns": s.loop_ns.to_string(), "engine_ns": engine_ns.to_string(), "bot_ns": bot_ns.to_string(), "env_ns": s.env_ns.to_string(),
        "fork_ns_mean": s.fork_ns as f64 / s.forks.max(1) as f64, "clone_ns_mean": s.clone_ns as f64 / s.forks.max(1) as f64,
        "replay_ns_mean": s.replay_ns as f64 / s.replays.max(1) as f64, "tree_per_game": mean(&s.tree_per_game),
        "kinds": Value::Object(kinds_json), "seg_len": s.seg_len.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
        "dice": s.dice.iter().map(|(k, d)| (k.clone(), json!({"records": d.records, "dice": d.dice, "no_threshold": d.no_threshold, "rerolled": d.rerolled}))).collect::<serde_json::Map<_, _>>(),
        "moves": {"seqs": s.move_seqs, "canonical": s.move_canonical, "reordered_ok": s.move_reordered_ok, "lost": s.move_reordered_lost, "inconclusive": s.move_replay_failed},
        "winners": s.winners, "vp_sum": s.vp_sum, "rounds": s.rounds.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
    });
    (out, value)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |flag: &str, default: u64| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(default);
    let games = get("--games", 200);
    let first = get("--first", 1);
    let workers = usize::try_from(get("--workers", 4)).unwrap_or(4).max(1);
    let replay_every = usize::try_from(get("--replay-every", 8)).unwrap_or(8);
    let json_path = args.iter().position(|a| a == "--json").and_then(|i| args.get(i + 1)).cloned();
    let started = Instant::now();
    let next = std::sync::atomic::AtomicU64::new(first);
    let results: Vec<Stats> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut rec = Recorder::new();
                    loop {
                        let seed = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if seed >= first + games {
                            break;
                        }
                        play_one(seed, replay_every, &mut rec);
                        if seed % 20 == 0 {
                            eprintln!("game {seed} done ({:.0} s)", started.elapsed().as_secs_f64());
                        }
                    }
                    rec.stats
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
    });
    let mut all = Stats::default();
    for r in &results {
        all.merge(r);
    }
    let (text, value) = report(&mut all, workers, started.elapsed());
    println!("{text}");
    if let Some(p) = json_path {
        let _ = std::fs::write(p, serde_json::to_string_pretty(&value).unwrap_or_default());
    }

}
