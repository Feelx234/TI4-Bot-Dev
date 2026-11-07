"""Synthetic shards in the exact format of the Rust writer, with a planted signal, for training and tests until the dataset
generator (lane L2) exists; and the Python shard writer (also used for the parity fixtures).

The planted "teacher": the picked option maximises `w . option features + u . (system token of its ptr0)` plus noise; the
winner is drawn from a softmax over the seats' victory-point column. A model that learns must beat chance on both.
"""

from __future__ import annotations

import gzip
import json
from pathlib import Path

import numpy as np

from .schema import FEATURE_VERSION, Schema

#: The schema of feature version 1 (the widths of `crates/ti4-mcts-features/src/encode.rs`).
DEFAULT_DIMS = {
    "feature_version": FEATURE_VERSION, "max_sys": 40, "seats": 4, "max_obj": 16, "ptrs": 3, "adj_bytes": 5,
    "kind_buckets": 512, "fs": 112, "fp": 156, "fo": 18, "fg": 17, "fa": 40,
}
META = ["game", "seed", "round", "actor", "faction", "ask_kind", "tree_kind", "smart", "picked", "decision", "k", "net", "sims",
        "rel0", "rel1", "rel2", "rel3", "played", "noise"]
#: The core's `Faction` indexes of Sol, Letnev, Xxcha, Jol-Nar.
FACTIONS = [28, 15, 31, 10]

DTYPES = {
    "sys": "<f2", "sys_id": "<i2", "adj": "|u1", "seat": "<f2", "seat_fac": "<i2", "obj": "<f2", "obj_id": "<i2", "glob": "<f2",
    "ask_kind": "<i2", "opt": "<f2", "opt_kind": "<i2", "opt_ptr": "<i2", "opt_mask": "|u1", "opt_off": "<u4", "pi": "<f2",
    "z": "<f2", "vp": "<f2", "meta": "<i8",
}


def default_schema(names: dict | None = None) -> Schema:
    d = dict(DEFAULT_DIMS)
    d["meta"] = META
    d["names"] = names or {}
    return Schema.from_dict(d)


def write_shard(root: str | Path, arrays: dict[str, np.ndarray], schema: Schema, compress: bool = True) -> None:
    """Write `arrays` (the names of `data.ARRAYS`) as the Rust writer does."""
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)
    for name, dtype in DTYPES.items():
        arr = np.ascontiguousarray(arrays[name], dtype=np.dtype(dtype))
        if compress:
            with gzip.open(root / f"{name}.npy.gz", "wb", compresslevel=3) as f:
                np.lib.format.write_array(f, arr, version=(1, 0))
        else:
            np.save(root / f"{name}.npy", arr)
    (root / "schema.json").write_text(json.dumps(schema.to_dict(), indent=1))


def synthetic(n: int, seed: int = 0, games: int | None = None, schema: Schema | None = None, noise: float = 0.3) -> dict[str, np.ndarray]:
    s = schema or default_schema()
    rng = np.random.default_rng(seed)
    teacher = np.random.default_rng(12345)  # the same teacher for every seed
    w_opt = teacher.normal(size=s.fa)
    w_sys = teacher.normal(size=s.fs)
    games = games or max(2, n // 50)
    n_sys = 37
    # Features: sparse quarter-steps in [0, 1].
    sys = (rng.integers(0, 5, size=(n, s.max_sys, s.fs)) / 4.0) * (rng.random((n, s.max_sys, s.fs)) < 0.15)
    sys[:, n_sys:] = 0.0
    sys[:, :n_sys, 0] = 1.0
    sys_id = np.full((n, s.max_sys), -1, np.int16)
    sys_id[:, :n_sys] = np.arange(n_sys)
    # A ring plus random chords, symmetric.
    adj_full = np.zeros((s.max_sys, s.max_sys), bool)
    for i in range(n_sys):
        adj_full[i, (i + 1) % n_sys] = adj_full[(i + 1) % n_sys, i] = True
        j = int(teacher.integers(0, n_sys))
        if j != i:
            adj_full[i, j] = adj_full[j, i] = True
    adj = np.packbits(adj_full, axis=-1, bitorder="little")[None].repeat(n, axis=0)
    seat = (rng.integers(0, 5, size=(n, s.seats, s.fp)) / 4.0) * (rng.random((n, s.seats, s.fp)) < 0.2)
    vp_col = 5  # "vp" in the seat names
    seat[:, :, vp_col] = rng.integers(0, 11, size=(n, s.seats)) / 10.0
    seat_fac = np.array([rng.permutation(FACTIONS) for _ in range(n)], dtype=np.int16)
    n_obj = rng.integers(1, 8, size=n)
    obj = np.zeros((n, s.max_obj, s.fo))
    obj_id = np.full((n, s.max_obj), -1, np.int16)
    for i in range(n):
        obj[i, : n_obj[i]] = rng.random((n_obj[i], s.fo)) < 0.3
        obj_id[i, : n_obj[i]] = rng.choice(40, size=n_obj[i], replace=False)
    glob = rng.random((n, s.fg)) * (rng.random((n, s.fg)) < 0.5)
    ask_kind = rng.integers(0, s.kind_buckets, size=n).astype(np.int16)
    ks = rng.integers(2, 13, size=n)
    off = np.concatenate([[0], np.cumsum(ks)]).astype(np.uint32)
    total = int(off[-1])
    opt = rng.random((total, s.fa)) * (rng.random((total, s.fa)) < 0.4)
    opt_kind = rng.integers(0, s.kind_buckets, size=total).astype(np.int16)
    opt_ptr = np.full((total, s.ptrs), -1, np.int16)
    opt_ptr[:, 0] = np.where(rng.random(total) < 0.8, rng.integers(0, n_sys, size=total), -1)
    opt_mask = (rng.random(total) < 0.9).astype(np.uint8)
    pi = np.zeros(total)
    picked = np.zeros(n, np.int64)
    smart = rng.random(n) < 0.9
    for i in range(n):
        lo, hi = int(off[i]), int(off[i + 1])
        score = opt[lo:hi] @ w_opt
        ptr = opt_ptr[lo:hi, 0]
        score = score + np.where(ptr >= 0, sys[i, np.maximum(ptr, 0)] @ w_sys * 0.5, 0.0)
        score = score + rng.normal(scale=noise, size=hi - lo)
        mask = opt_mask[lo:hi].astype(bool)
        if not mask.any():
            opt_mask[lo:hi] = 1
            mask[:] = True
        score = np.where(mask, score, -np.inf)
        j = int(np.argmax(score))
        picked[i] = j
        if smart[i]:
            pi[lo + j] = 1.0
    # The winner: softmax over the seats' victory points (x 4).
    logit = seat[:, :, vp_col] * 4.0
    p = np.exp(logit - logit.max(axis=1, keepdims=True))
    p /= p.sum(axis=1, keepdims=True)
    winner = np.array([rng.choice(s.seats, p=row) for row in p])
    z = np.zeros((n, s.seats))
    z[np.arange(n), winner] = 1.0
    vp = seat[:, :, vp_col].copy()
    game = rng.integers(0, games, size=n)
    meta = np.zeros((n, len(s.meta)), np.int64)
    m = {name: i for i, name in enumerate(s.meta)}
    meta[:, m["game"]] = game
    meta[:, m["seed"]] = seed
    meta[:, m["round"]] = rng.integers(1, 8, size=n)
    meta[:, m["faction"]] = seat_fac[:, 0]
    meta[:, m["ask_kind"]] = ask_kind
    meta[:, m["tree_kind"]] = rng.integers(0, 7, size=n)
    meta[:, m["smart"]] = smart
    meta[:, m["picked"]] = picked
    meta[:, m["k"]] = ks
    for r in range(4):
        meta[:, m[f"rel{r}"]] = r
    return {
        "sys": sys, "sys_id": sys_id, "adj": adj, "seat": seat, "seat_fac": seat_fac, "obj": obj, "obj_id": obj_id, "glob": glob,
        "ask_kind": ask_kind, "opt": opt, "opt_kind": opt_kind, "opt_ptr": opt_ptr, "opt_mask": opt_mask, "opt_off": off, "pi": pi,
        "z": z, "vp": vp, "meta": meta,
    }


def write_synthetic(root: str | Path, n: int, seed: int = 0, compress: bool = True) -> Path:
    s = default_schema()
    write_shard(root, synthetic(n, seed=seed, schema=s), s, compress=compress)
    return Path(root)


def subset(arrays: dict[str, np.ndarray], rows: list[int]) -> dict[str, np.ndarray]:
    """The records `rows` of a shard's arrays (for the parity fixtures)."""
    off = np.asarray(arrays["opt_off"], dtype=np.int64)
    out = {}
    for name in ("sys", "sys_id", "adj", "seat", "seat_fac", "obj", "obj_id", "glob", "ask_kind", "z", "vp", "meta"):
        out[name] = np.asarray(arrays[name])[rows]
    spans = [np.arange(off[r], off[r + 1]) for r in rows]
    flat = np.concatenate(spans) if spans else np.zeros(0, np.int64)
    for name in ("opt", "opt_kind", "opt_ptr", "opt_mask", "pi"):
        out[name] = np.asarray(arrays[name])[flat]
    out["opt_off"] = np.concatenate([[0], np.cumsum([len(x) for x in spans])]).astype(np.uint32)
    return out
