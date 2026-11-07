"""Counts of a dataset directory of `mcts_dataset` shards (for runs stopped before their summary was written).

    /root/mcts-venv/bin/python tools/mcts/dataset_stats.py DIR [--out stats.json]
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import numpy as np  # noqa: E402

from ti4mcts.data import read_array  # noqa: E402
from ti4mcts.stream import TREE_KINDS, find_shards  # noqa: E402


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("dir")
    ap.add_argument("--out")
    a = ap.parse_args()
    shards = find_shards([a.dir])
    games, records, bytes_ = set(), 0, 0
    kinds = np.zeros(len(TREE_KINDS), np.int64)
    smart = noise = 0
    for s in shards:
        meta = read_array(s / "meta")
        names = json.loads((s / "schema.json").read_text())["meta"]
        col = {n: i for i, n in enumerate(names)}
        games.update(meta[:, col["seed"]].tolist())
        records += len(meta)
        kinds += np.bincount(meta[:, col["tree_kind"]].clip(0), minlength=len(TREE_KINDS))[: len(TREE_KINDS)]
        smart += int(meta[:, col["smart"]].sum())
        noise += int(meta[:, col["noise"]].sum())
        bytes_ += sum(f.stat().st_size for f in s.iterdir())
    out = {"shards": len(shards), "games": len(games), "records": records, "records_per_game": records / max(1, len(games)),
           "records_by_kind": dict(zip(TREE_KINDS, kinds.tolist())), "with_policy_target": smart, "noise_played": noise,
           "bytes": bytes_, "bytes_per_record": bytes_ / max(1, records)}
    text = json.dumps(out, indent=1)
    if a.out:
        Path(a.out).write_text(text)
    print(text)


if __name__ == "__main__":
    main()
