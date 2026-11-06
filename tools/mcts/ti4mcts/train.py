"""Imitation + value training (docs/MCTS_BOT_PLAN.md 4.1), CPU only.

```text
python -m ti4mcts.train --shards DIR [DIR...] --out RUN_DIR [--epochs 10] [--batch 256] [--lr 3e-4] [--threads 4]
python -m ti4mcts.train --synthetic 20000 --out RUN_DIR      (a synthetic shard first, until lane L2 exists)
```

Writes `RUN_DIR/model.pt` (the PyTorch state), `RUN_DIR/weights.t4w` (the Rust weights), `RUN_DIR/fixtures/` (the parity
fixtures of lane L4) and `RUN_DIR/log.jsonl` (one line per epoch: losses and top-1 agreement with the target, train and
held-out, the held-out split by tree kind).
"""

from __future__ import annotations

import argparse
import json
import time
from pathlib import Path

import numpy as np
import torch

from .data import Shard, batches, load_shards, split_by_game
from .export import write_fixtures, write_weights
from .model import Config, Net, losses, parameter_count
from .synth import write_synthetic

TREE_KINDS = ["draft", "menu", "closing", "activate", "move", "score", "imperial"]


def evaluate(net: Net, shards: list[Shard], index, batch: int) -> dict:
    net.eval()
    tot = {"policy": 0.0, "value": 0.0, "vp": 0.0, "n": 0, "agree": 0.0, "agree_n": 0}
    by_kind: dict[str, list[int]] = {}
    with torch.no_grad():
        for b in batches(shards, index, batch):
            out = net(b)
            l = losses(out, b)
            n = int(b["sys"].shape[0])
            tot["policy"] += float(l.policy) * max(l.agree_n, 0)
            tot["value"] += float(l.value) * n
            tot["vp"] += float(l.vp) * n
            tot["n"] += n
            tot["agree"] += l.agree * l.agree_n
            tot["agree_n"] += l.agree_n
            logits = out["logits"].masked_fill(~(b["opt_valid"] & b["opt_mask"]), -1e9)
            pred = logits.argmax(-1)
            best = b["pi"].argmax(-1)
            for kind_i, ok, has in zip(b["tree_kind"].tolist(), (pred == best).tolist(), b["has_pi"].tolist()):
                if has:
                    name = TREE_KINDS[kind_i] if 0 <= kind_i < len(TREE_KINDS) else "other"
                    c = by_kind.setdefault(name, [0, 0])
                    c[0] += int(ok)
                    c[1] += 1
    n = max(1, tot["n"])
    return {
        "policy": tot["policy"] / max(1, tot["agree_n"]),
        "value": tot["value"] / n,
        "vp": tot["vp"] / n,
        "agree": tot["agree"] / max(1, tot["agree_n"]),
        "records": tot["n"],
        "by_kind": {k: round(v[0] / max(1, v[1]), 4) for k, v in sorted(by_kind.items())},
    }


def chance_agreement(shards: list[Shard], index) -> float:
    """Top-1 agreement of a uniform pick among the allowed options (the baseline)."""
    acc, n = 0.0, 0
    for si, rows in index:
        s = shards[si]
        off = np.asarray(s.a["opt_off"], dtype=np.int64)
        mask = np.asarray(s.a["opt_mask"]) != 0
        pi = np.asarray(s.a["pi"], dtype=np.float32)
        for r in rows.tolist():
            lo, hi = off[r], off[r + 1]
            if pi[lo:hi].sum() > 0.5:
                acc += 1.0 / max(1, int(mask[lo:hi].sum()))
                n += 1
    return acc / max(1, n)


def train(shard_paths: list[str], out: Path, epochs: int = 10, batch: int = 256, lr: float = 3e-4, weight_decay: float = 1e-4,
          seed: int = 0, cfg: Config | None = None, held_share: float = 0.05, fixtures: int = 200, max_minutes: float | None = None,
          log=print) -> dict:
    torch.manual_seed(seed)
    out.mkdir(parents=True, exist_ok=True)
    shards = load_shards(shard_paths)
    schema = shards[0].schema
    split = split_by_game(shards, held_share=held_share, seed=seed)
    net = Net(schema, cfg)
    opt = torch.optim.AdamW(net.parameters(), lr=lr, weight_decay=weight_decay)
    n_train = sum(len(r) for _, r in split.train)
    steps_total = max(1, epochs * ((n_train + batch - 1) // batch))
    sched = torch.optim.lr_scheduler.OneCycleLR(opt, max_lr=lr, total_steps=steps_total, pct_start=0.05)
    rng = np.random.default_rng(seed)
    log(f"records: train {n_train}, held {sum(len(r) for _, r in split.held)}; parameters {parameter_count(net)}; chance agreement {chance_agreement(shards, split.held):.3f}")
    started = time.time()
    history = []
    step = 0
    for epoch in range(epochs):
        net.train()
        run = {"loss": 0.0, "agree": 0.0, "n": 0}
        for b in batches(shards, split.train, batch, rng):
            l = losses(net(b), b)
            opt.zero_grad()
            l.total.backward()
            torch.nn.utils.clip_grad_norm_(net.parameters(), 1.0)
            opt.step()
            if step < steps_total - 1:
                sched.step()
            step += 1
            run["loss"] += float(l.total.detach())
            run["agree"] += l.agree
            run["n"] += 1
            if max_minutes is not None and time.time() - started > max_minutes * 60:
                break
        held = evaluate(net, shards, split.held, batch)
        line = {"epoch": epoch + 1, "train_loss": run["loss"] / max(1, run["n"]), "train_agree": run["agree"] / max(1, run["n"]), "held": held, "seconds": round(time.time() - started, 1)}
        history.append(line)
        log(json.dumps(line))
        with open(out / "log.jsonl", "a") as f:
            f.write(json.dumps(line) + "\n")
        if max_minutes is not None and time.time() - started > max_minutes * 60:
            break
    torch.save({"state": net.state_dict(), "config": net.cfg.to_dict(), "dims": schema.dims()}, out / "model.pt")
    write_weights(net, out / "weights.t4w")
    held_shard = shards[split.held[0][0]] if split.held and len(split.held[0][1]) else shards[0]
    manifest = write_fixtures(net, held_shard, out / "fixtures", n=fixtures, seed=seed)
    return {"history": history, "fixtures": manifest, "parameters": parameter_count(net)}


def main(argv: list[str] | None = None) -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--shards", nargs="*", default=[])
    ap.add_argument("--synthetic", type=int, default=0, help="write a synthetic shard of this many records into OUT/synthetic and train on it")
    ap.add_argument("--out", required=True)
    ap.add_argument("--epochs", type=int, default=10)
    ap.add_argument("--batch", type=int, default=256)
    ap.add_argument("--lr", type=float, default=3e-4)
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--fixtures", type=int, default=200)
    ap.add_argument("--max-minutes", type=float, default=None)
    args = ap.parse_args(argv)
    torch.set_num_threads(args.threads)
    out = Path(args.out)
    paths = list(args.shards)
    if args.synthetic:
        paths.append(str(write_synthetic(out / "synthetic", args.synthetic, seed=args.seed)))
    if not paths:
        ap.error("give --shards or --synthetic")
    result = train(paths, out, epochs=args.epochs, batch=args.batch, lr=args.lr, seed=args.seed, fixtures=args.fixtures, max_minutes=args.max_minutes)
    print(json.dumps({"parameters": result["parameters"], "final": result["history"][-1] if result["history"] else None}))


if __name__ == "__main__":
    main()
