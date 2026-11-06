//! The forward pass of the MCTS policy + value net (docs/MCTS_BOT_PLAN.md 3.2 and 3.4, lane L4), in plain Rust over the
//! `.t4w` weights that `tools/mcts/ti4mcts/export.py` writes.
//!
//! The specification is `tools/mcts/ti4mcts/reference.py` (the NumPy forward pass, itself checked against PyTorch). The parity
//! test (`tests/parity.rs`) compares this crate with the fixtures the trainer writes, within 1e-4.
//!
//! Exact rewrites of the reference that make one record cheap (none changes a result beyond float rounding):
//!
//! * **Compaction**: the empty system and objective slots are dropped before the attention. They are masked as keys in the
//!   reference (their weight is `exp(-1e9) = 0`), and their own outputs are never read (the pooled mean skips them, the
//!   option pointers name filled slots only). Seat tokens are kept, masked as keys when empty, as the reference does.
//! * **Sparse inputs**: a Linear skips the zero inputs (most features are zero).
//! * **Policy head split**: `p1 · [h, sys, obj, kind, opt]` is computed as `p1_h · h` once per record, `p1_sys · token` once
//!   per pointed system, `p1_obj · token` once per pointed objective, `p1_kind · kind_emb` once per kind bucket at load time,
//!   and only `p1_opt · opt` per option.
//!
//! The weights file: `b"T4MW"`, `u32` format (1), `u32` header length, the JSON header (`config`, `dims`, `tensors` with
//! `name`, `shape`, `offset` in floats), then the f32 data. A Linear weight is `[out, in]`; LayerNorm eps is 1e-5.

use std::path::Path;

use ti4_mcts_features::encode::Encoded;

/// The weights format this crate reads.
pub const FORMAT: u32 = 1;
const EPS: f32 = 1e-5;
const NEG: f32 = -1e9;

/// The model's hyper-parameters (`model.Config`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub d: usize,
    pub heads: usize,
    pub ff: usize,
    pub sys_blocks: usize,
    pub joint_blocks: usize,
    pub fac_dim: usize,
    pub obj_dim: usize,
    pub kind_dim: usize,
    pub head_hidden: usize,
}

/// The feature dimensions the model was trained for (`Schema.dims`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dims {
    pub feature_version: u32,
    pub max_sys: usize,
    pub seats: usize,
    pub max_obj: usize,
    pub ptrs: usize,
    pub kind_buckets: usize,
    pub fs: usize,
    pub fp: usize,
    pub fo: usize,
    pub fg: usize,
    pub fa: usize,
}

/// A Linear layer, the weight stored transposed (`[in][out]`) so that one input row adds a contiguous output row.
#[derive(Debug, Clone)]
struct Linear {
    n_in: usize,
    n_out: usize,
    wt: Vec<f32>,
    b: Vec<f32>,
}

// Index loops over equal-length reslices, not `zip`: without LTO, the generic `Zip::new` of the standard library is not
// always inlined across codegen units (it showed as 40 % of the time in a profile).
// Kernels are their own functions (not inlined): the `&mut` arguments are `noalias` there, which lets LLVM vectorise them.
#[inline(never)]
fn axpy(y: &mut [f32], a: f32, x: &[f32]) {
    let n = y.len().min(x.len());
    let (y, x) = (&mut y[..n], &x[..n]);
    for i in 0..n {
        y[i] += a * x[i];
    }
}

/// `x[j] = exp(x[j] - max)` for every `j`; returns the sum. A polynomial `exp` (relative error about 2e-7, well inside the
/// parity tolerance) that vectorises, instead of the libm call per element.
#[inline(never)]
fn exp_shifted(x: &mut [f32], max: f32) -> f32 {
    const LOG2E: f32 = std::f32::consts::LOG2_E;
    const LN2_HI: f32 = 0.693_145_75;
    const LN2_LO: f32 = 1.428_606_8e-6;
    const ROUND: f32 = 12_582_912.0; // 1.5 * 2^23: adding and subtracting it rounds to an integer.
    for v in x.iter_mut() {
        let z = (*v - max).max(-87.0);
        let k = (z * LOG2E + ROUND) - ROUND;
        let r = (z - k * LN2_HI) - k * LN2_LO;
        // exp(r) on |r| <= ln2/2: Taylor to degree 6.
        let p = 1.0 + r * (1.0 + r * (0.5 + r * (1.0 / 6.0 + r * (1.0 / 24.0 + r * (1.0 / 120.0 + r * (1.0 / 720.0))))));
        let scale = f32::from_bits(((k as i32 + 127) as u32) << 23);
        *v = p * scale;
    }
    let mut acc = [0.0f32; 8];
    let full = x.len() / 8 * 8;
    for c in x[..full].chunks_exact(8) {
        for l in 0..8 {
            acc[l] += c[l];
        }
    }
    acc.iter().sum::<f32>() + x[full..].iter().sum::<f32>()
}

/// `y += inv * sum_j p[j] * v[j]` (`v` is `[t][dh]`, `y` is `[dh]`).
#[inline(never)]
fn weighted_rows(y: &mut [f32], p: &[f32], inv: f32, v: &[f32]) {
    let dh = y.len();
    for (j, pj) in p.iter().enumerate() {
        let a = pj * inv;
        let row = &v[j * dh..(j + 1) * dh];
        for c in 0..dh {
            y[c] += a * row[c];
        }
    }
}

#[inline(always)]
fn add(y: &mut [f32], x: &[f32]) {
    let n = y.len().min(x.len());
    let (y, x) = (&mut y[..n], &x[..n]);
    for i in 0..n {
        y[i] += x[i];
    }
}

impl Linear {
    /// From the PyTorch layout `w [out, in]`.
    fn new(w: &[f32], b: &[f32], n_out: usize, n_in: usize) -> Self {
        let mut wt = vec![0.0; n_in * n_out];
        for o in 0..n_out {
            for i in 0..n_in {
                wt[i * n_out + o] = w[o * n_in + i];
            }
        }
        Self { n_in, n_out, wt, b: b.to_vec() }
    }

    /// The columns `lo..hi` of the input side as their own layer (no bias).
    fn columns(&self, lo: usize, hi: usize) -> Self {
        Self { n_in: hi - lo, n_out: self.n_out, wt: self.wt[lo * self.n_out..hi * self.n_out].to_vec(), b: vec![0.0; self.n_out] }
    }

    /// `y = b + x W^T` for one row; zero inputs are skipped.
    #[inline]
    fn row(&self, x: &[f32], y: &mut [f32]) {
        debug_assert_eq!(x.len(), self.n_in);
        y.copy_from_slice(&self.b);
        self.add_row(x, y);
    }

    /// `y += x W^T` (no bias).
    #[inline]
    fn add_row(&self, x: &[f32], y: &mut [f32]) {
        let n = self.n_out;
        for (i, xi) in x.iter().enumerate() {
            if *xi != 0.0 {
                axpy(y, *xi, &self.wt[i * n..(i + 1) * n]);
            }
        }
    }

    /// Every row of `x` (`rows x n_in`) into `y` (`rows x n_out`, resized): a register-blocked product, tiles of four rows by
    /// eight outputs accumulated over the whole input (the accumulators stay in registers; the outer-product form was bound
    /// by the loads and stores of `y`).
    fn rows(&self, x: &[f32], rows: usize, y: &mut Vec<f32>) {
        let (ni, no) = (self.n_in, self.n_out);
        y.clear();
        y.resize(rows * no, 0.0);
        let full_r = rows / 4 * 4;
        let full_o = no / TILE * TILE;
        for r in (0..full_r).step_by(4) {
            let xs = &x[r * ni..(r + 4) * ni];
            for o in (0..full_o).step_by(TILE) {
                let acc = tile(xs, ni, &self.wt, no, o);
                for (k, row) in acc.iter().enumerate() {
                    let at = (r + k) * no + o;
                    for l in 0..TILE {
                        y[at + l] = row[l] + self.b[o + l];
                    }
                }
            }
            for k in 0..4 {
                for o in full_o..no {
                    let mut v = self.b[o];
                    for i in 0..ni {
                        v += xs[k * ni + i] * self.wt[i * no + o];
                    }
                    y[(r + k) * no + o] = v;
                }
            }
        }
        for r in full_r..rows {
            self.row(&x[r * ni..(r + 1) * ni], &mut y[r * no..(r + 1) * no]);
        }
    }
}

/// Output columns per tile of [`Linear::rows`].
const TILE: usize = 8;

/// `acc[k][l] = sum_i xs[k * ni + i] * wt[i * no + o + l]` for four rows `k` and [`TILE`] columns `l`.
#[inline(never)]
fn tile(xs: &[f32], ni: usize, wt: &[f32], no: usize, o: usize) -> [[f32; TILE]; 4] {
    let (mut c0, mut c1, mut c2, mut c3) = ([0.0f32; TILE], [0.0f32; TILE], [0.0f32; TILE], [0.0f32; TILE]);
    let (x0, x1, x2, x3) = (&xs[..ni], &xs[ni..2 * ni], &xs[2 * ni..3 * ni], &xs[3 * ni..4 * ni]);
    let wt = &wt[o..];
    for i in 0..ni {
        let w = &wt[i * no..i * no + TILE];
        let (a0, a1, a2, a3) = (x0[i], x1[i], x2[i], x3[i]);
        for l in 0..TILE {
            c0[l] += a0 * w[l];
        }
        for l in 0..TILE {
            c1[l] += a1 * w[l];
        }
        for l in 0..TILE {
            c2[l] += a2 * w[l];
        }
        for l in 0..TILE {
            c3[l] += a3 * w[l];
        }
    }
    [c0, c1, c2, c3]
}

#[derive(Debug, Clone)]
struct Norm {
    g: Vec<f32>,
    b: Vec<f32>,
}

impl Norm {
    #[inline]
    fn row(&self, x: &[f32], y: &mut [f32]) {
        let n = x.len() as f32;
        let mu = x.iter().sum::<f32>() / n;
        let var = x.iter().map(|v| (v - mu) * (v - mu)).sum::<f32>() / n;
        let inv = 1.0 / (var + EPS).sqrt();
        for i in 0..x.len() {
            y[i] = (x[i] - mu) * inv * self.g[i] + self.b[i];
        }
    }

    fn rows(&self, x: &[f32], rows: usize, d: usize, y: &mut Vec<f32>) {
        y.clear();
        y.resize(rows * d, 0.0);
        for r in 0..rows {
            self.row(&x[r * d..(r + 1) * d], &mut y[r * d..(r + 1) * d]);
        }
    }
}

/// A pre-LayerNorm transformer block (`model.Block`).
#[derive(Debug, Clone)]
struct Block {
    ln1: Norm,
    qkv: Linear,
    out: Linear,
    ln2: Norm,
    ff1: Linear,
    ff2: Linear,
}

/// Buffers reused between records (one per thread).
#[derive(Debug, Default, Clone)]
pub struct Scratch {
    x: Vec<f32>,
    n: Vec<f32>,
    qkv: Vec<f32>,
    y: Vec<f32>,
    o: Vec<f32>,
    f: Vec<f32>,
    att: Vec<f32>,
    kt: Vec<f32>,
    vh: Vec<f32>,
    tokens: Vec<f32>,
    valid: Vec<bool>,
    sys_slot: Vec<usize>,
    sys_of_slot: Vec<i32>,
    obj_of_slot: Vec<i32>,
    bias: Vec<f32>,
    cat: Vec<f32>,
    row: Vec<f32>,
    proj_sys: Vec<Option<usize>>,
    proj_obj: Vec<Option<usize>>,
    proj: Vec<f32>,
    hid: Vec<f32>,
}

impl Block {
    /// `x` (`t x d`) in place. `bias(h)` is the additive attention bias of head `h` (`t x t`, or empty), `key_valid` masks
    /// keys (empty: all valid).
    fn forward(&self, x: &mut [f32], t: usize, d: usize, heads: usize, bias: &[f32], key_valid: &[bool], s: &mut Scratch) {
        let dh = d / heads;
        let scale = 1.0 / (dh as f32).sqrt();
        self.ln1.rows(x, t, d, &mut s.n);
        self.qkv.rows(&s.n, t, &mut s.qkv);
        s.y.clear();
        s.y.resize(t * d, 0.0);
        s.att.resize(t, 0.0);
        let w3 = 3 * d;
        s.kt.resize(dh * t, 0.0);
        s.vh.resize(t * dh, 0.0);
        for h in 0..heads {
            let (qo, ko, vo) = (h * dh, d + h * dh, 2 * d + h * dh);
            // The keys transposed (`[dh][t]`) and the values (`[t][dh]`) of this head, contiguous.
            for j in 0..t {
                for c in 0..dh {
                    s.kt[c * t + j] = s.qkv[j * w3 + ko + c];
                    s.vh[j * dh + c] = s.qkv[j * w3 + vo + c];
                }
            }
            for i in 0..t {
                let att = &mut s.att[..t];
                if bias.is_empty() {
                    att.fill(0.0);
                } else {
                    att.copy_from_slice(&bias[(h * t + i) * t..(h * t + i + 1) * t]);
                }
                for c in 0..dh {
                    axpy(att, s.qkv[i * w3 + qo + c] * scale, &s.kt[c * t..(c + 1) * t]);
                }
                if !key_valid.is_empty() {
                    for j in 0..t {
                        if !key_valid[j] {
                            att[j] = NEG;
                        }
                    }
                }
                let max = att.iter().copied().fold(f32::MIN, f32::max);
                let sum = exp_shifted(att, max);
                let inv = 1.0 / sum;
                let yrow = &mut s.y[i * d + qo..i * d + qo + dh];
                weighted_rows(yrow, att, inv, &s.vh[..t * dh]);
            }
        }
        self.out.rows(&s.y, t, &mut s.o);
        add(x, &s.o);
        self.ln2.rows(x, t, d, &mut s.n);
        self.ff1.rows(&s.n, t, &mut s.f);
        for v in &mut s.f {
            *v = v.max(0.0);
        }
        self.ff2.rows(&s.f, t, &mut s.o);
        add(x, &s.o);
    }
}

/// The outputs of one record.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Out {
    /// Win logits, actor-relative (softmax gives the win probabilities).
    pub win: Vec<f32>,
    /// Final victory points / 10, actor-relative.
    pub vp: Vec<f32>,
    /// Raw option logits (before the filter mask).
    pub logits: Vec<f32>,
}

impl Out {
    /// The win probabilities.
    #[must_use]
    pub fn win_probs(&self) -> Vec<f32> {
        softmax(&self.win)
    }
}

/// The softmax of `x`.
#[must_use]
pub fn softmax(x: &[f32]) -> Vec<f32> {
    let max = x.iter().copied().fold(f32::MIN, f32::max);
    let e: Vec<f32> = x.iter().map(|v| (v - max).exp()).collect();
    let s: f32 = e.iter().sum();
    e.into_iter().map(|v| v / s).collect()
}

/// The network.
#[derive(Debug, Clone)]
pub struct Net {
    pub cfg: Config,
    pub dims: Dims,
    sys_in: Linear,
    adj_bias: Vec<f32>,
    self_bias: Vec<f32>,
    sys_blocks: Vec<Block>,
    fac_emb: Vec<f32>,
    seat_in: Linear,
    seat_pos: Vec<f32>,
    obj_emb: Vec<f32>,
    obj_in: Linear,
    ask_emb: Vec<f32>,
    glob_in: Linear,
    type_emb: Vec<f32>,
    joint_blocks: Vec<Block>,
    ln_out: Norm,
    v1: Linear,
    v_win: Linear,
    v_vp: Linear,
    /// `p1` split by input block: the pooled `h`, the system token, the objective token, the option features.
    p1_h: Linear,
    p1_sys: Linear,
    p1_obj: Linear,
    p1_opt: Linear,
    /// `p1_kind · kind_emb[k]` per kind bucket (`[kind_buckets][head_hidden]`).
    p1_kind: Vec<f32>,
    p2: Vec<f32>,
    p2_b: f32,
}

fn usize_of(v: &serde_json::Value, key: &str) -> Result<usize, String> {
    v.get(key).and_then(serde_json::Value::as_u64).map(|x| x as usize).ok_or_else(|| format!("header: no {key}"))
}

struct Tensors {
    names: Vec<(String, Vec<usize>, usize)>,
    data: Vec<f32>,
}

impl Tensors {
    fn get(&self, name: &str, shape: &[usize]) -> Result<&[f32], String> {
        let (_, sh, off) = self.names.iter().find(|(n, _, _)| n == name).ok_or_else(|| format!("weights: no tensor {name}"))?;
        if sh.as_slice() != shape {
            return Err(format!("weights: {name} has shape {sh:?}, expected {shape:?}"));
        }
        let size: usize = sh.iter().product();
        self.data.get(*off..off + size).ok_or_else(|| format!("weights: {name} runs past the data"))
    }

    fn linear(&self, name: &str, n_out: usize, n_in: usize) -> Result<Linear, String> {
        Ok(Linear::new(self.get(&format!("{name}.weight"), &[n_out, n_in])?, self.get(&format!("{name}.bias"), &[n_out])?, n_out, n_in))
    }

    fn norm(&self, name: &str, d: usize) -> Result<Norm, String> {
        Ok(Norm { g: self.get(&format!("{name}.weight"), &[d])?.to_vec(), b: self.get(&format!("{name}.bias"), &[d])?.to_vec() })
    }

    fn block(&self, name: &str, c: &Config) -> Result<Block, String> {
        Ok(Block {
            ln1: self.norm(&format!("{name}.ln1"), c.d)?,
            qkv: self.linear(&format!("{name}.qkv"), 3 * c.d, c.d)?,
            out: self.linear(&format!("{name}.out"), c.d, c.d)?,
            ln2: self.norm(&format!("{name}.ln2"), c.d)?,
            ff1: self.linear(&format!("{name}.ff1"), c.ff, c.d)?,
            ff2: self.linear(&format!("{name}.ff2"), c.d, c.ff)?,
        })
    }
}

/// Embedding rows of the faction ids (`model.FACTION_ROWS`) and the objective ids (`model.OBJECTIVE_ROWS`).
const FACTION_ROWS: usize = 35;
const OBJECTIVE_ROWS: usize = 106;

impl Net {
    /// Read a `.t4w` file.
    ///
    /// # Errors
    /// I/O, a wrong format, or a tensor missing or of the wrong shape.
    pub fn load(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_bytes(&raw)
    }

    /// [`Self::load`] from the bytes of a weights file.
    ///
    /// # Errors
    /// As [`Self::load`].
    pub fn from_bytes(raw: &[u8]) -> Result<Self, String> {
        if raw.len() < 12 || &raw[..4] != b"T4MW" {
            return Err("not a T4MW weights file".to_owned());
        }
        let fmt = u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
        if fmt != FORMAT {
            return Err(format!("weights format {fmt}, this reader knows {FORMAT}"));
        }
        let n = u32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]) as usize;
        let text = raw.get(12..12 + n).ok_or("short header")?;
        let header: serde_json::Value = serde_json::from_slice(text).map_err(|e| format!("header: {e}"))?;
        let c = header.get("config").ok_or("header: no config")?;
        let cfg = Config {
            d: usize_of(c, "d")?,
            heads: usize_of(c, "heads")?,
            ff: usize_of(c, "ff")?,
            sys_blocks: usize_of(c, "sys_blocks")?,
            joint_blocks: usize_of(c, "joint_blocks")?,
            fac_dim: usize_of(c, "fac_dim")?,
            obj_dim: usize_of(c, "obj_dim")?,
            kind_dim: usize_of(c, "kind_dim")?,
            head_hidden: usize_of(c, "head_hidden")?,
        };
        let dm = header.get("dims").ok_or("header: no dims")?;
        let dims = Dims {
            feature_version: usize_of(dm, "feature_version")? as u32,
            max_sys: usize_of(dm, "max_sys")?,
            seats: usize_of(dm, "seats")?,
            max_obj: usize_of(dm, "max_obj")?,
            ptrs: usize_of(dm, "ptrs")?,
            kind_buckets: usize_of(dm, "kind_buckets")?,
            fs: usize_of(dm, "fs")?,
            fp: usize_of(dm, "fp")?,
            fo: usize_of(dm, "fo")?,
            fg: usize_of(dm, "fg")?,
            fa: usize_of(dm, "fa")?,
        };
        if cfg.heads == 0 || cfg.d % cfg.heads != 0 {
            return Err(format!("d {} is not a multiple of heads {}", cfg.d, cfg.heads));
        }
        let mut names = Vec::new();
        for t in header.get("tensors").and_then(serde_json::Value::as_array).ok_or("header: no tensors")? {
            let name = t.get("name").and_then(serde_json::Value::as_str).ok_or("tensor without name")?.to_owned();
            let shape: Vec<usize> = t.get("shape").and_then(serde_json::Value::as_array).ok_or("tensor without shape")?.iter().filter_map(serde_json::Value::as_u64).map(|x| x as usize).collect();
            names.push((name, shape, usize_of(t, "offset")?));
        }
        let data: Vec<f32> = raw[12 + n..].chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        let t = Tensors { names, data };
        let (d, hh) = (cfg.d, cfg.head_hidden);
        let p1 = t.linear("p1", hh, 4 * d + cfg.kind_dim + dims.fa)?;
        let kind_emb = t.get("kind_emb.weight", &[dims.kind_buckets, cfg.kind_dim])?;
        let p1_k = p1.columns(4 * d, 4 * d + cfg.kind_dim);
        let mut p1_kind = vec![0.0; dims.kind_buckets * hh];
        for k in 0..dims.kind_buckets {
            p1_k.add_row(&kind_emb[k * cfg.kind_dim..(k + 1) * cfg.kind_dim], &mut p1_kind[k * hh..(k + 1) * hh]);
        }
        let mut p1_h = p1.columns(0, 2 * d);
        p1_h.b.clone_from(&p1.b);
        Ok(Self {
            sys_in: t.linear("sys_in", d, dims.fs)?,
            adj_bias: t.get("adj_bias", &[cfg.heads])?.to_vec(),
            self_bias: t.get("self_bias", &[cfg.heads])?.to_vec(),
            sys_blocks: (0..cfg.sys_blocks).map(|i| t.block(&format!("sys_blocks.{i}"), &cfg)).collect::<Result<_, _>>()?,
            fac_emb: t.get("fac_emb.weight", &[FACTION_ROWS, cfg.fac_dim])?.to_vec(),
            seat_in: t.linear("seat_in", d, dims.fp + cfg.fac_dim)?,
            seat_pos: t.get("seat_pos", &[dims.seats, d])?.to_vec(),
            obj_emb: t.get("obj_emb.weight", &[OBJECTIVE_ROWS, cfg.obj_dim])?.to_vec(),
            obj_in: t.linear("obj_in", d, dims.fo + cfg.obj_dim)?,
            ask_emb: t.get("ask_emb.weight", &[dims.kind_buckets, cfg.kind_dim])?.to_vec(),
            glob_in: t.linear("glob_in", d, dims.fg + cfg.kind_dim)?,
            type_emb: t.get("type_emb", &[4, d])?.to_vec(),
            joint_blocks: (0..cfg.joint_blocks).map(|i| t.block(&format!("joint_blocks.{i}"), &cfg)).collect::<Result<_, _>>()?,
            ln_out: t.norm("ln_out", d)?,
            v1: t.linear("v1", hh, 2 * d)?,
            v_win: t.linear("v_win", dims.seats, hh)?,
            v_vp: t.linear("v_vp", dims.seats, hh)?,
            p1_h,
            p1_sys: p1.columns(2 * d, 3 * d),
            p1_obj: p1.columns(3 * d, 4 * d),
            p1_opt: p1.columns(4 * d + cfg.kind_dim, 4 * d + cfg.kind_dim + dims.fa),
            p1_kind,
            p2: t.get("p2.weight", &[1, hh])?.to_vec(),
            p2_b: t.get("p2.bias", &[1])?[0],
            cfg,
            dims,
        })
    }

    /// Whether the net reads records of these feature widths (the features crate's constants).
    #[must_use]
    pub fn fits(&self, fs: usize, fp: usize, fo: usize, fg: usize, fa: usize, max_sys: usize, max_obj: usize, seats: usize) -> bool {
        let d = &self.dims;
        (d.fs, d.fp, d.fo, d.fg, d.fa, d.max_sys, d.max_obj, d.seats) == (fs, fp, fo, fg, fa, max_sys, max_obj, seats)
    }

    /// The forward pass of one record.
    #[must_use]
    pub fn forward(&self, e: &Encoded, s: &mut Scratch) -> Out {
        let (c, dm) = (&self.cfg, &self.dims);
        let d = c.d;
        // 1. System tokens (filled slots only).
        s.sys_slot.clear();
        s.sys_of_slot.clear();
        s.sys_of_slot.resize(dm.max_sys, -1);
        for (slot, id) in e.sys_id.iter().enumerate().take(dm.max_sys) {
            if *id >= 0 {
                s.sys_of_slot[slot] = s.sys_slot.len() as i32;
                s.sys_slot.push(slot);
            }
        }
        let ns = s.sys_slot.len();
        let mut x = std::mem::take(&mut s.x);
        x.clear();
        x.resize(ns * d, 0.0);
        for (i, slot) in s.sys_slot.iter().enumerate() {
            self.sys_in.row(&e.sys[slot * dm.fs..(slot + 1) * dm.fs], &mut x[i * d..(i + 1) * d]);
        }
        // The adjacency and self biases per head.
        let ab = dm.max_sys.div_ceil(8);
        let mut bias = std::mem::take(&mut s.bias);
        bias.clear();
        bias.resize(c.heads * ns * ns, 0.0);
        for (i, si) in s.sys_slot.iter().enumerate() {
            for (j, sj) in s.sys_slot.iter().enumerate() {
                let adj = (e.adj[si * ab + sj / 8] >> (sj % 8)) & 1 == 1;
                for h in 0..c.heads {
                    let mut v = 0.0;
                    if adj {
                        v += self.adj_bias[h];
                    }
                    if i == j {
                        v += self.self_bias[h];
                    }
                    bias[(h * ns + i) * ns + j] = v;
                }
            }
        }
        for blk in &self.sys_blocks {
            blk.forward(&mut x, ns, d, c.heads, &bias, &[], s);
        }
        s.bias = bias;
        // 2. The joint tokens: systems, seats, objectives (filled), global.
        let mut tokens = std::mem::take(&mut s.tokens);
        tokens.clear();
        let te = &self.type_emb;
        for i in 0..ns {
            for k in 0..d {
                tokens.push(x[i * d + k] + te[k]);
            }
        }
        s.x = x;
        let mut valid = std::mem::take(&mut s.valid);
        valid.clear();
        valid.resize(ns, true);
        let mut cat = std::mem::take(&mut s.cat);
        let mut row = std::mem::take(&mut s.row);
        row.resize(d, 0.0);
        for r in 0..dm.seats {
            let fac = e.seat_fac.get(r).copied().unwrap_or(-1);
            let fr = usize::try_from(i32::from(fac) + 1).unwrap_or(0).min(FACTION_ROWS - 1);
            cat.clear();
            cat.extend_from_slice(&e.seat[r * dm.fp..(r + 1) * dm.fp]);
            cat.extend_from_slice(&self.fac_emb[fr * c.fac_dim..(fr + 1) * c.fac_dim]);
            self.seat_in.row(&cat, &mut row);
            for k in 0..d {
                tokens.push(row[k] + self.seat_pos[r * d + k] + te[d + k]);
            }
            valid.push(fac >= 0);
        }
        s.obj_of_slot.clear();
        s.obj_of_slot.resize(dm.max_obj, -1);
        let mut no = 0;
        for slot in 0..dm.max_obj {
            let id = e.obj_id.get(slot).copied().unwrap_or(-1);
            if id < 0 {
                continue;
            }
            let orow = usize::try_from(i32::from(id) + 1).unwrap_or(0).min(OBJECTIVE_ROWS - 1);
            cat.clear();
            cat.extend_from_slice(&e.obj[slot * dm.fo..(slot + 1) * dm.fo]);
            cat.extend_from_slice(&self.obj_emb[orow * c.obj_dim..(orow + 1) * c.obj_dim]);
            self.obj_in.row(&cat, &mut row);
            for k in 0..d {
                tokens.push(row[k] + te[2 * d + k]);
            }
            valid.push(true);
            s.obj_of_slot[slot] = no;
            no += 1;
        }
        let ak = usize::try_from(e.ask_kind).unwrap_or(0).min(dm.kind_buckets - 1);
        cat.clear();
        cat.extend_from_slice(&e.glob);
        cat.extend_from_slice(&self.ask_emb[ak * c.kind_dim..(ak + 1) * c.kind_dim]);
        self.glob_in.row(&cat, &mut row);
        for k in 0..d {
            tokens.push(row[k] + te[3 * d + k]);
        }
        valid.push(true);
        s.cat = cat;
        let t = valid.len();
        let all_valid = valid.iter().all(|v| *v);
        for blk in &self.joint_blocks {
            blk.forward(&mut tokens, t, d, c.heads, &[], if all_valid { &[] } else { &valid }, s);
        }
        // ln_out in place (row by row through `row`).
        for i in 0..t {
            self.ln_out.row(&tokens[i * d..(i + 1) * d], &mut row);
            tokens[i * d..(i + 1) * d].copy_from_slice(&row);
        }
        // 3. Pooled h = [mean of the valid tokens, the actor's seat token].
        let mut h = vec![0.0f32; 2 * d];
        let nv = valid.iter().filter(|v| **v).count().max(1) as f32;
        for i in 0..t {
            if valid[i] {
                axpy(&mut h[..d], 1.0, &tokens[i * d..(i + 1) * d]);
            }
        }
        for v in &mut h[..d] {
            *v /= nv;
        }
        h[d..].copy_from_slice(&tokens[ns * d..(ns + 1) * d]);
        let hh = c.head_hidden;
        let mut hv = vec![0.0; hh];
        self.v1.row(&h, &mut hv);
        for v in &mut hv {
            *v = v.max(0.0);
        }
        let mut win = vec![0.0; dm.seats];
        self.v_win.row(&hv, &mut win);
        let mut vp = vec![0.0; dm.seats];
        self.v_vp.row(&hv, &mut vp);
        // 4. The policy head.
        let k = e.mask.len();
        let mut base = vec![0.0; hh];
        self.p1_h.row(&h, &mut base);
        s.proj_sys.clear();
        s.proj_sys.resize(ns, None);
        s.proj_obj.clear();
        s.proj_obj.resize(no as usize, None);
        s.proj.clear();
        let obj0 = ns + dm.seats;
        let mut logits = Vec::with_capacity(k);
        let mut hid = std::mem::take(&mut s.hid);
        hid.resize(hh, 0.0);
        for j in 0..k {
            hid.copy_from_slice(&base);
            let p0 = e.opt_ptr.get(j * dm.ptrs).copied().unwrap_or(-1);
            let p1 = e.opt_ptr.get(j * dm.ptrs + 1).copied().unwrap_or(-1);
            if let Some(ci) = usize::try_from(p0).ok().and_then(|p| s.sys_of_slot.get(p).copied()).and_then(|c| usize::try_from(c).ok()) {
                let at = match s.proj_sys[ci] {
                    Some(at) => at,
                    None => {
                        let at = s.proj.len();
                        s.proj.resize(at + hh, 0.0);
                        self.p1_sys.add_row(&tokens[ci * d..(ci + 1) * d], &mut s.proj[at..at + hh]);
                        s.proj_sys[ci] = Some(at);
                        at
                    }
                };
                axpy(&mut hid, 1.0, &s.proj[at..at + hh]);
            }
            if let Some(ci) = usize::try_from(p1).ok().and_then(|p| s.obj_of_slot.get(p).copied()).and_then(|c| usize::try_from(c).ok()) {
                let at = match s.proj_obj[ci] {
                    Some(at) => at,
                    None => {
                        let at = s.proj.len();
                        s.proj.resize(at + hh, 0.0);
                        let tok = obj0 + ci;
                        self.p1_obj.add_row(&tokens[tok * d..(tok + 1) * d], &mut s.proj[at..at + hh]);
                        s.proj_obj[ci] = Some(at);
                        at
                    }
                };
                axpy(&mut hid, 1.0, &s.proj[at..at + hh]);
            }
            let kb = usize::try_from(e.opt_kind.get(j).copied().unwrap_or(0)).unwrap_or(0).min(dm.kind_buckets - 1);
            axpy(&mut hid, 1.0, &self.p1_kind[kb * hh..(kb + 1) * hh]);
            self.p1_opt.add_row(&e.opt[j * dm.fa..(j + 1) * dm.fa], &mut hid);
            let mut l = self.p2_b;
            for (i, a) in hid.iter().enumerate() {
                l += a.max(0.0) * self.p2[i];
            }
            logits.push(l);
        }
        s.hid = hid;
        s.tokens = tokens;
        s.valid = valid;
        s.row = row;
        Out { win, vp, logits }
    }

    /// The index of the best kept option (`mask`), the net's greedy answer; `None` for a record without a kept option.
    #[must_use]
    pub fn greedy(&self, e: &Encoded, s: &mut Scratch) -> Option<usize> {
        let out = self.forward(e, s);
        out.logits.iter().enumerate().filter(|(i, _)| e.mask.get(*i).copied().unwrap_or(false)).max_by(|a, b| a.1.total_cmp(b.1)).map(|(i, _)| i)
    }
}

/// Records of a plain (not gzip) shard directory as [`Encoded`] values: the parity fixtures' `inputs/` and the benchmark.
pub mod plain_shard {
    use std::path::Path;

    use ti4_mcts_features::encode::Encoded;
    use ti4_mcts_features::npy;

    fn arr(dir: &Path, name: &str) -> Result<npy::Array, String> {
        npy::read(&dir.join(format!("{name}.npy"))).map_err(|e| e.to_string())
    }

    fn i16s(v: &[i64]) -> Vec<i16> {
        v.iter().map(|x| *x as i16).collect()
    }

    /// Every record of the shard at `dir`.
    ///
    /// # Errors
    /// A missing or unreadable array.
    pub fn read(dir: &Path) -> Result<Vec<Encoded>, String> {
        let sys = arr(dir, "sys")?;
        let n = sys.shape[0];
        let (s_sys, s_id, s_adj) = (sys.f.len() / n.max(1), arr(dir, "sys_id")?, arr(dir, "adj")?);
        let (seat, seat_fac, obj, obj_id, glob, ask) = (arr(dir, "seat")?, arr(dir, "seat_fac")?, arr(dir, "obj")?, arr(dir, "obj_id")?, arr(dir, "glob")?, arr(dir, "ask_kind")?);
        let (opt, opt_kind, opt_ptr, opt_mask, off) = (arr(dir, "opt")?, arr(dir, "opt_kind")?, arr(dir, "opt_ptr")?, arr(dir, "opt_mask")?, arr(dir, "opt_off")?);
        let per = |a: &npy::Array| a.f.len().max(a.i.len()) / n.max(1);
        let fa = opt.shape.get(1).copied().unwrap_or(0);
        let ptrs = opt_ptr.shape.get(1).copied().unwrap_or(0);
        let mut out = Vec::with_capacity(n);
        for r in 0..n {
            let (lo, hi) = (off.i[r] as usize, off.i[r + 1] as usize);
            let sl = |a: &npy::Array| -> std::ops::Range<usize> { r * per(a)..(r + 1) * per(a) };
            out.push(Encoded {
                sys: sys.f[r * s_sys..(r + 1) * s_sys].to_vec(),
                sys_id: i16s(&s_id.i[sl(&s_id)]),
                adj: s_adj.i[sl(&s_adj)].iter().map(|b| *b as u8).collect(),
                seat: seat.f[sl(&seat)].to_vec(),
                seat_fac: i16s(&seat_fac.i[sl(&seat_fac)]),
                obj: obj.f[sl(&obj)].to_vec(),
                obj_id: i16s(&obj_id.i[sl(&obj_id)]),
                glob: glob.f[sl(&glob)].to_vec(),
                ask_kind: ask.i[r] as i16,
                opt: opt.f[lo * fa..hi * fa].to_vec(),
                opt_kind: i16s(&opt_kind.i[lo..hi]),
                opt_ptr: i16s(&opt_ptr.i[lo * ptrs..hi * ptrs]),
                mask: opt_mask.i[lo..hi].iter().map(|m| *m != 0).collect(),
            });
        }
        Ok(out)
    }
}
