"""Shards and batches.

A shard is a directory of `.npy` (or gzip-compressed `.npy.gz`) files plus `schema.json`, as
`crates/ti4-mcts-features/src/shard.rs` writes it. Plain files are memory-mapped; gzip files are read into memory.

A batch pads the ragged option block to the largest option count of the batch: `opt_valid` marks the real options,
`opt_mask` the ones the hard filter keeps (the policy softmax runs over `opt_valid & opt_mask`).
"""

from __future__ import annotations

import gzip
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import torch

from .schema import Schema

ARRAYS = (
    "sys", "sys_id", "adj", "seat", "seat_fac", "obj", "obj_id", "glob", "ask_kind",
    "opt", "opt_kind", "opt_ptr", "opt_mask", "opt_off", "pi", "z", "vp", "meta",
)


def read_array(path_base: Path) -> np.ndarray:
    """`name.npy` (memory-mapped) or `name.npy.gz`."""
    plain = path_base.with_suffix(".npy")
    if plain.exists():
        return np.load(plain, mmap_mode="r")
    packed = Path(str(plain) + ".gz")
    with gzip.open(packed, "rb") as f:
        return np.lib.format.read_array(f)


class Shard:
    """One shard directory."""

    def __init__(self, root: str | Path):
        self.root = Path(root)
        self.schema = Schema.load(self.root)
        self.a = {name: read_array(self.root / name) for name in ARRAYS}
        s = self.schema
        n = len(self.a["ask_kind"])
        expect = {
            "sys": (n, s.max_sys, s.fs), "sys_id": (n, s.max_sys), "adj": (n, s.max_sys, s.adj_bytes),
            "seat": (n, s.seats, s.fp), "seat_fac": (n, s.seats), "obj": (n, s.max_obj, s.fo), "obj_id": (n, s.max_obj),
            "glob": (n, s.fg), "opt_off": (n + 1,), "z": (n, s.seats), "vp": (n, s.seats), "meta": (n, len(s.meta)),
        }
        for name, shape in expect.items():
            if tuple(self.a[name].shape) != shape:
                raise ValueError(f"{self.root}: {name} has shape {self.a[name].shape}, expected {shape}")
        k = int(self.a["opt_off"][-1])
        for name, shape in (("opt", (k, s.fa)), ("opt_kind", (k,)), ("opt_ptr", (k, s.ptrs)), ("opt_mask", (k,)), ("pi", (k,))):
            if tuple(self.a[name].shape) != shape:
                raise ValueError(f"{self.root}: {name} has shape {self.a[name].shape}, expected {shape}")
        self.n = n

    def __len__(self) -> int:
        return self.n

    def meta(self, name: str) -> np.ndarray:
        return np.asarray(self.a["meta"][:, self.schema.meta_index(name)])

    def game_keys(self) -> np.ndarray:
        """A key per record that is equal for the records of one game (seed and game number)."""
        return self.meta("seed").astype(np.int64) * 1_000_003 + self.meta("game").astype(np.int64)


@dataclass
class Split:
    train: list[tuple[int, np.ndarray]]
    held: list[tuple[int, np.ndarray]]


def split_by_game(shards: list[Shard], held_share: float = 0.05, seed: int = 0) -> Split:
    """Hold out whole games (never single positions: the positions of one game are correlated)."""
    rng = np.random.default_rng(seed)
    keys = np.unique(np.concatenate([s.game_keys() for s in shards])) if shards else np.zeros(0, np.int64)
    held_keys = set(rng.choice(keys, size=max(1, int(round(len(keys) * held_share))), replace=False).tolist()) if len(keys) > 1 else set()
    train, held = [], []
    for i, s in enumerate(shards):
        g = s.game_keys()
        is_held = np.array([k in held_keys for k in g.tolist()], dtype=bool)
        train.append((i, np.nonzero(~is_held)[0]))
        held.append((i, np.nonzero(is_held)[0]))
    return Split(train=train, held=held)


def unpack_adj(adj: np.ndarray, max_sys: int) -> np.ndarray:
    """`[B, S, ADJ_BYTES]` little-endian bits -> `[B, S, S]` bool."""
    bits = np.unpackbits(adj, axis=-1, bitorder="little")
    return bits[..., :max_sys].astype(bool)


def make_batch(shard: Shard, rows: np.ndarray) -> dict[str, torch.Tensor]:
    """The tensors of the records `rows` of one shard (float32 features)."""
    a, s = shard.a, shard.schema
    rows = np.sort(np.asarray(rows, dtype=np.int64))
    b = len(rows)
    off = np.asarray(a["opt_off"], dtype=np.int64)
    starts, ends = off[rows], off[rows + 1]
    ks = ends - starts
    kmax = max(1, int(ks.max()) if b else 1)
    opt = np.zeros((b, kmax, s.fa), np.float32)
    opt_kind = np.zeros((b, kmax), np.int64)
    opt_ptr = np.full((b, kmax, s.ptrs), -1, np.int64)
    opt_mask = np.zeros((b, kmax), bool)
    opt_valid = np.zeros((b, kmax), bool)
    pi = np.zeros((b, kmax), np.float32)
    for i, (lo, hi) in enumerate(zip(starts.tolist(), ends.tolist())):
        k = hi - lo
        opt[i, :k] = a["opt"][lo:hi]
        opt_kind[i, :k] = a["opt_kind"][lo:hi]
        opt_ptr[i, :k] = a["opt_ptr"][lo:hi]
        opt_mask[i, :k] = a["opt_mask"][lo:hi] != 0
        opt_valid[i, :k] = True
        pi[i, :k] = a["pi"][lo:hi]
    sys_id = np.asarray(a["sys_id"][rows], dtype=np.int64)
    obj_id = np.asarray(a["obj_id"][rows], dtype=np.int64)
    t = torch.from_numpy
    return {
        "sys": t(np.asarray(a["sys"][rows], dtype=np.float32)),
        "sys_mask": t(sys_id >= 0),
        "adj": t(unpack_adj(np.asarray(a["adj"][rows]), s.max_sys)),
        "seat": t(np.asarray(a["seat"][rows], dtype=np.float32)),
        "seat_fac": t(np.asarray(a["seat_fac"][rows], dtype=np.int64)),
        "obj": t(np.asarray(a["obj"][rows], dtype=np.float32)),
        "obj_id": t(obj_id),
        "obj_mask": t(obj_id >= 0),
        "glob": t(np.asarray(a["glob"][rows], dtype=np.float32)),
        "ask_kind": t(np.asarray(a["ask_kind"][rows], dtype=np.int64)),
        "opt": t(opt),
        "opt_kind": t(opt_kind),
        "opt_ptr": t(opt_ptr),
        "opt_mask": t(opt_mask),
        "opt_valid": t(opt_valid),
        "pi": t(pi),
        "has_pi": t(pi.sum(axis=1) > 0.5),
        "z": t(np.asarray(a["z"][rows], dtype=np.float32)),
        "vp": t(np.asarray(a["vp"][rows], dtype=np.float32)),
        "tree_kind": t(np.asarray(a["meta"][rows, s.meta_index("tree_kind")], dtype=np.int64)),
        "picked": t(np.asarray(a["meta"][rows, s.meta_index("picked")], dtype=np.int64)),
    }


def batches(shards: list[Shard], index: list[tuple[int, np.ndarray]], batch_size: int, rng: np.random.Generator | None = None):
    """Batches over `index` (pairs of shard number and record rows); shuffled when `rng` is given. A batch never mixes shards
    (each shard is its own memory map)."""
    plan = []
    for si, rows in index:
        rows = np.array(rows, dtype=np.int64)
        if rng is not None:
            rng.shuffle(rows)
        for lo in range(0, len(rows), batch_size):
            plan.append((si, rows[lo:lo + batch_size]))
    if rng is not None:
        order = rng.permutation(len(plan))
        plan = [plan[i] for i in order]
    for si, rows in plan:
        if len(rows):
            yield make_batch(shards[si], rows)


def load_shards(paths: list[str | Path]) -> list[Shard]:
    """Every shard directory under the given paths (a path is a shard or a directory of shards)."""
    found: list[Shard] = []
    for p in paths:
        p = Path(p)
        if (p / "schema.json").exists():
            found.append(Shard(p))
        else:
            for q in sorted(p.iterdir()):
                if (q / "schema.json").exists():
                    found.append(Shard(q))
    if not found:
        raise FileNotFoundError(f"no shard under {paths}")
    first = found[0].schema
    for s in found[1:]:
        if s.schema != first:
            raise ValueError(f"{s.root}: another schema than {found[0].root}")
    return found
