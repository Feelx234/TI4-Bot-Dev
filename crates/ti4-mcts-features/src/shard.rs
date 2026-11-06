//! Training shards (docs/MCTS_BOT_PLAN.md 3.4): one directory of `.npy` files per shard plus `schema.json`.
//!
//! | file | dtype | shape |
//! |---|---|---|
//! | `sys` | f16 | `[N, MAX_SYS, FS]` |
//! | `sys_id` | i16 | `[N, MAX_SYS]` |
//! | `adj` | u8 | `[N, MAX_SYS, ADJ_BYTES]` (bit `j % 8` of byte `j / 8` of row `i`) |
//! | `seat` | f16 | `[N, SEATS, FP]` (actor first) |
//! | `seat_fac` | i16 | `[N, SEATS]` |
//! | `obj` | f16 | `[N, MAX_OBJ, FO]` |
//! | `obj_id` | i16 | `[N, MAX_OBJ]` |
//! | `glob` | f16 | `[N, FG]` |
//! | `ask_kind` | i16 | `[N]` |
//! | `opt` | f16 | `[ΣK, FA]` |
//! | `opt_kind` | i16 | `[ΣK]` |
//! | `opt_ptr` | i16 | `[ΣK, PTRS]` |
//! | `opt_mask` | u8 | `[ΣK]` |
//! | `opt_off` | u32 | `[N + 1]` (options of record `i`: `opt_off[i]..opt_off[i+1]`) |
//! | `pi` | f16 | `[ΣK]` (the policy target over the options; sums to 1 per record, or all 0 = no policy target) |
//! | `z` | f16 | `[N, SEATS]` (outcome, actor-relative: 1 for the winner, a tie split, 0 else) |
//! | `vp` | f16 | `[N, SEATS]` (final victory points / 10, actor-relative) |
//! | `meta` | i64 | `[N, META.len()]` |
//!
//! Outcomes are known at the end of a game only: [`ShardWriter::push`] buffers the game's records, [`ShardWriter::end_game`]
//! fills `z` and `vp` (given by absolute seat) and rotates them to the actor of each record.

use std::path::Path;

use crate::encode::{Encoded, ADJ_BYTES, FA, FG, FO, FP, FS, MAX_OBJ, MAX_SYS, PTRS, SEATS};
use crate::npy::{self, Dtype};

/// The meta columns, in order.
pub const META: [&str; 17] = [
    "game", "seed", "round", "actor", "faction", "ask_kind", "tree_kind", "smart", "picked", "decision", "k", "net", "sims", "rel0", "rel1", "rel2", "rel3",
];

/// What a record says about itself (see [`META`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Meta {
    pub game: u64,
    pub seed: u64,
    pub round: u32,
    /// The actor's absolute seat.
    pub actor: u8,
    pub faction: i16,
    pub ask_kind: i16,
    /// `TreeKind::index`, -1 for a delegated ask.
    pub tree_kind: i8,
    pub smart: bool,
    /// The index of the played option (the rule bot's pick in pretraining).
    pub picked: u16,
    pub decision: u32,
    /// The net version that played (0: the rule bot).
    pub net: u32,
    /// Search simulations (0: none).
    pub sims: u32,
    /// The absolute seat of each actor-relative block.
    pub rel: [u8; SEATS],
}

/// One decision with its policy target.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub enc: Encoded,
    /// `[K]`: the target distribution (or all 0: no policy loss).
    pub pi: Vec<f32>,
    pub meta: Meta,
}

/// Collects records; writes them as one shard.
#[derive(Default)]
pub struct ShardWriter {
    pending: Vec<Record>,
    done: Vec<(Record, [f32; SEATS], [f32; SEATS])>,
    /// Names of the columns (from [`crate::encode::column_names`]), written to `schema.json`.
    pub names: Option<[Vec<String>; 5]>,
}

impl ShardWriter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records finished (with an outcome).
    #[must_use]
    pub fn len(&self) -> usize {
        self.done.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.done.is_empty()
    }

    /// A record of the game in progress.
    pub fn push(&mut self, r: Record) {
        debug_assert_eq!(r.pi.len(), r.enc.k());
        self.pending.push(r);
    }

    /// The game ended: `z` and `vp` by absolute seat. Records are rotated to their actor.
    pub fn end_game(&mut self, z: &[f32], vp: &[f32]) {
        for r in self.pending.drain(..) {
            let mut zr = [0.0; SEATS];
            let mut vr = [0.0; SEATS];
            for (i, seat) in r.meta.rel.iter().enumerate() {
                zr[i] = z.get(usize::from(*seat)).copied().unwrap_or(0.0);
                vr[i] = vp.get(usize::from(*seat)).copied().unwrap_or(0.0) / 10.0;
            }
            self.done.push((r, zr, vr));
        }
    }

    /// Drop the records of a game that did not end (an error).
    pub fn drop_game(&mut self) {
        self.pending.clear();
    }

    /// Write the finished records to `dir` (created) and forget them.
    ///
    /// # Errors
    /// I/O.
    pub fn write(&mut self, dir: &Path) -> std::io::Result<usize> {
        std::fs::create_dir_all(dir)?;
        let n = self.done.len();
        let mut sys = Vec::with_capacity(n * MAX_SYS * FS);
        let mut sys_id = Vec::with_capacity(n * MAX_SYS);
        let mut adj = Vec::with_capacity(n * MAX_SYS * ADJ_BYTES);
        let mut seat = Vec::with_capacity(n * SEATS * FP);
        let mut seat_fac = Vec::with_capacity(n * SEATS);
        let mut obj = Vec::with_capacity(n * MAX_OBJ * FO);
        let mut obj_id = Vec::with_capacity(n * MAX_OBJ);
        let mut glob = Vec::with_capacity(n * FG);
        let mut ask_kind = Vec::with_capacity(n);
        let (mut opt, mut opt_kind, mut opt_ptr, mut opt_mask, mut pi) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut opt_off = vec![0u32];
        let mut z = Vec::with_capacity(n * SEATS);
        let mut vp = Vec::with_capacity(n * SEATS);
        let mut meta = Vec::with_capacity(n * META.len());
        for (r, zr, vr) in &self.done {
            let e = &r.enc;
            sys.extend_from_slice(&e.sys);
            sys_id.extend_from_slice(&e.sys_id);
            adj.extend_from_slice(&e.adj);
            seat.extend_from_slice(&e.seat);
            seat_fac.extend_from_slice(&e.seat_fac);
            obj.extend_from_slice(&e.obj);
            obj_id.extend_from_slice(&e.obj_id);
            glob.extend_from_slice(&e.glob);
            ask_kind.push(e.ask_kind);
            opt.extend_from_slice(&e.opt);
            opt_kind.extend_from_slice(&e.opt_kind);
            opt_ptr.extend_from_slice(&e.opt_ptr);
            opt_mask.extend(e.mask.iter().map(|m| u8::from(*m)));
            pi.extend_from_slice(&r.pi);
            opt_off.push(u32::try_from(opt_kind.len()).unwrap_or(u32::MAX));
            z.extend_from_slice(zr);
            vp.extend_from_slice(vr);
            let m = &r.meta;
            meta.extend_from_slice(&[
                m.game as i64,
                m.seed as i64,
                i64::from(m.round),
                i64::from(m.actor),
                i64::from(m.faction),
                i64::from(m.ask_kind),
                i64::from(m.tree_kind),
                i64::from(m.smart),
                i64::from(m.picked),
                i64::from(m.decision),
                e.k() as i64,
                i64::from(m.net),
                i64::from(m.sims),
                i64::from(m.rel[0]),
                i64::from(m.rel[1]),
                i64::from(m.rel[2]),
                i64::from(m.rel[3]),
            ]);
        }
        let k = opt_kind.len();
        npy::write(&dir.join("sys.npy"), Dtype::F16, &[n, MAX_SYS, FS], &npy::f16_bytes(&sys))?;
        npy::write(&dir.join("sys_id.npy"), Dtype::I16, &[n, MAX_SYS], &npy::i16_bytes(&sys_id))?;
        npy::write(&dir.join("adj.npy"), Dtype::U8, &[n, MAX_SYS, ADJ_BYTES], &adj)?;
        npy::write(&dir.join("seat.npy"), Dtype::F16, &[n, SEATS, FP], &npy::f16_bytes(&seat))?;
        npy::write(&dir.join("seat_fac.npy"), Dtype::I16, &[n, SEATS], &npy::i16_bytes(&seat_fac))?;
        npy::write(&dir.join("obj.npy"), Dtype::F16, &[n, MAX_OBJ, FO], &npy::f16_bytes(&obj))?;
        npy::write(&dir.join("obj_id.npy"), Dtype::I16, &[n, MAX_OBJ], &npy::i16_bytes(&obj_id))?;
        npy::write(&dir.join("glob.npy"), Dtype::F16, &[n, FG], &npy::f16_bytes(&glob))?;
        npy::write(&dir.join("ask_kind.npy"), Dtype::I16, &[n], &npy::i16_bytes(&ask_kind))?;
        npy::write(&dir.join("opt.npy"), Dtype::F16, &[k, FA], &npy::f16_bytes(&opt))?;
        npy::write(&dir.join("opt_kind.npy"), Dtype::I16, &[k], &npy::i16_bytes(&opt_kind))?;
        npy::write(&dir.join("opt_ptr.npy"), Dtype::I16, &[k, PTRS], &npy::i16_bytes(&opt_ptr))?;
        npy::write(&dir.join("opt_mask.npy"), Dtype::U8, &[k], &opt_mask)?;
        npy::write(&dir.join("opt_off.npy"), Dtype::U32, &[n + 1], &npy::u32_bytes(&opt_off))?;
        npy::write(&dir.join("pi.npy"), Dtype::F16, &[k], &npy::f16_bytes(&pi))?;
        npy::write(&dir.join("z.npy"), Dtype::F16, &[n, SEATS], &npy::f16_bytes(&z))?;
        npy::write(&dir.join("vp.npy"), Dtype::F16, &[n, SEATS], &npy::f16_bytes(&vp))?;
        npy::write(&dir.join("meta.npy"), Dtype::I64, &[n, META.len()], &npy::i64_bytes(&meta))?;
        std::fs::write(dir.join("schema.json"), crate::schema::json(self.names.as_ref()))?;
        self.done.clear();
        Ok(n)
    }
}
