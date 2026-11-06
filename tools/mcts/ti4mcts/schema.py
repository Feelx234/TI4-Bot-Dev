"""The feature schema written by `ti4-mcts-features` (`crates/ti4-mcts-features/src/schema.rs`).

The trainer never computes features: it reads the shards Rust wrote and refuses a schema it was not built for.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path

#: The feature version this trainer understands (`encode::FEATURE_VERSION` on the Rust side).
FEATURE_VERSION = 1


@dataclass(frozen=True)
class Schema:
    feature_version: int
    max_sys: int
    seats: int
    max_obj: int
    ptrs: int
    adj_bytes: int
    kind_buckets: int
    fs: int
    fp: int
    fo: int
    fg: int
    fa: int
    meta: tuple[str, ...]
    names: dict = field(default_factory=dict, compare=False)

    @classmethod
    def from_dict(cls, d: dict) -> "Schema":
        return cls(
            feature_version=int(d["feature_version"]),
            max_sys=int(d["max_sys"]),
            seats=int(d["seats"]),
            max_obj=int(d["max_obj"]),
            ptrs=int(d["ptrs"]),
            adj_bytes=int(d["adj_bytes"]),
            kind_buckets=int(d["kind_buckets"]),
            fs=int(d["fs"]),
            fp=int(d["fp"]),
            fo=int(d["fo"]),
            fg=int(d["fg"]),
            fa=int(d["fa"]),
            meta=tuple(d["meta"]),
            names=d.get("names", {}),
        )

    @classmethod
    def load(cls, shard_dir: str | Path) -> "Schema":
        s = cls.from_dict(json.loads((Path(shard_dir) / "schema.json").read_text()))
        s.check()
        return s

    def check(self) -> None:
        if self.feature_version != FEATURE_VERSION:
            raise ValueError(f"feature version {self.feature_version}: this trainer reads version {FEATURE_VERSION}")
        for block, width in (("sys", self.fs), ("seat", self.fp), ("obj", self.fo), ("glob", self.fg), ("opt", self.fa)):
            names = self.names.get(block) or []
            if names and len(names) != width:
                raise ValueError(f"schema: {len(names)} {block} names for width {width}")
        if self.adj_bytes * 8 < self.max_sys:
            raise ValueError("schema: the adjacency rows are too short")

    def dims(self) -> dict:
        """The dimensions a model is built for (stored in the exported weights)."""
        return {k: getattr(self, k) for k in ("feature_version", "max_sys", "seats", "max_obj", "ptrs", "kind_buckets", "fs", "fp", "fo", "fg", "fa")}

    def meta_index(self, name: str) -> int:
        return self.meta.index(name)

    def to_dict(self) -> dict:
        d = self.dims()
        d.update({"adj_bytes": self.adj_bytes, "meta": list(self.meta), "names": self.names})
        return d
