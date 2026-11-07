"""Training on a dataset larger than memory (lane L2): the shards of `mcts_dataset` are gzip files of about 100 games each, 100+ MB
once decompressed, so the trainer streams them.

* **Split by shard**: every `held_every`-th shard (in name order) is held out; a shard holds whole games, so this is a split by
  game. At most `held_max` held shards are evaluated.
* **Pools**: an epoch visits the training shards in a random order, `pool` at a time; the batches of a pool are shuffled across
  its shards.
* **Value subsampling per game**: the positions of one game share one outcome, so each game contributes the value loss of at
  most `value_per_game` records (`w_value = min(1, value_per_game / records of the game)`).
* **Rare kinds**: the policy loss of a tree kind is weighted by `clip(sqrt(median count / count), 0.5, 3)` (counts of the
  records with a target in the first pool).
"""

from __future__ import annotations

import json
import time
from pathlib import Path

import numpy as np
import torch

from .data import Shard, make_batch
from .export import write_fixtures, write_weights
from .model import Config, Net, losses, parameter_count

TREE_KINDS = ["draft", "menu", "closing", "activate", "move", "score", "imperial"]
UNIFORM_CE = float(np.log(4.0))


def find_shards(paths: list[str | Path]) -> list[Path]:
    found: list[Path] = []
    for p in paths:
        p = Path(p)
        if (p / "schema.json").exists():
            found.append(p)
        elif p.is_dir():
            found.extend(q for q in sorted(p.iterdir()) if (q / "schema.json").exists())
    if not found:
        raise FileNotFoundError(f"no shard under {paths}")
    return sorted(found)


def split_shards(shards: list[Path], held_every: int = 20, held_max: int = 8) -> tuple[list[Path], list[Path]]:
    held = [s for i, s in enumerate(shards) if i % held_every == held_every // 2][:held_max]
    train = [s for s in shards if s not in held]
    return train, held


def value_weights(shard: Shard, per_game: float) -> np.ndarray:
    keys = shard.game_keys()
    _, inverse, counts = np.unique(keys, return_inverse=True, return_counts=True)
    return np.minimum(1.0, per_game / counts[inverse]).astype(np.float32)


def kind_weights(shard: Shard) -> np.ndarray:
    kinds = shard.meta("tree_kind")
    off = np.asarray(shard.a["opt_off"], dtype=np.int64)
    pi = np.asarray(shard.a["pi"], dtype=np.float32)
    has = np.add.reduceat(pi, off[:-1]) > 0.5 if len(off) > 1 else np.zeros(0, bool)
    counts = np.array([max(1, int(((kinds == k) & has).sum())) for k in range(len(TREE_KINDS))], dtype=np.float64)
    w = np.clip(np.sqrt(np.median(counts) / counts), 0.5, 3.0)
    return w.astype(np.float32)


def add_weights(b: dict, shard: Shard, rows: np.ndarray, kw: np.ndarray) -> dict:
    rows = np.sort(np.asarray(rows, dtype=np.int64))
    b["w_value"] = torch.from_numpy(shard.w_value[rows])
    k = b["tree_kind"].numpy()
    b["w_policy"] = torch.from_numpy(np.where((k >= 0) & (k < len(kw)), kw[np.clip(k, 0, len(kw) - 1)], 1.0).astype(np.float32))
    return b


def load(path: Path, per_game: float) -> Shard:
    s = Shard(path)
    s.w_value = value_weights(s, per_game)
    return s


def pool_batches(shards: list[Shard], batch: int, rng: np.random.Generator, kw: np.ndarray):
    plan = []
    for si, s in enumerate(shards):
        rows = rng.permutation(len(s))
        for lo in range(0, len(rows), batch):
            plan.append((si, rows[lo:lo + batch]))
    for i in rng.permutation(len(plan)):
        si, rows = plan[i]
        yield add_weights(make_batch(shards[si], rows), shards[si], rows, kw)


def faction_prior(shards: list[Shard]) -> dict[int, float]:
    """Win share per faction id over the records of `shards` (the value baseline that knows the factions)."""
    wins: dict[int, float] = {}
    seats: dict[int, float] = {}
    for s in shards:
        z = np.asarray(s.a["z"], dtype=np.float32)
        fac = np.asarray(s.a["seat_fac"], dtype=np.int64)
        for f in np.unique(fac):
            m = fac == f
            wins[int(f)] = wins.get(int(f), 0.0) + float(z[m].sum())
            seats[int(f)] = seats.get(int(f), 0.0) + float(m.sum())
    return {f: wins[f] / max(1.0, seats[f]) for f in wins}


def evaluate(net: Net, held: list[Shard], batch: int, prior: dict[int, float]) -> dict:
    net.eval()
    agree = {k: [0, 0] for k in TREE_KINDS}
    tot = {"value": 0.0, "value_n": 0, "prior": 0.0, "vp": 0.0, "policy": 0.0, "policy_n": 0}
    by_round: dict[int, list[float]] = {}
    chance = [0.0, 0]
    with torch.no_grad():
        for s in held:
            for lo in range(0, len(s), batch):
                rows = np.arange(lo, min(len(s), lo + batch))
                b = make_batch(s, rows)
                out = net(b)
                ok = b["opt_valid"] & b["opt_mask"]
                logits = out["logits"].masked_fill(~ok, -1e9)
                pred = logits.argmax(-1)
                best = b["pi"].argmax(-1)
                has = b["has_pi"]
                logp = torch.log_softmax(logits, -1)
                if has.any():
                    tot["policy"] += float(-(logp.gather(1, best[:, None])[:, 0])[has].sum())
                    tot["policy_n"] += int(has.sum())
                for kind_i, hit, h, kk in zip(b["tree_kind"].tolist(), (pred == best).tolist(), has.tolist(), ok.sum(-1).tolist()):
                    if h and 0 <= kind_i < len(TREE_KINDS):
                        agree[TREE_KINDS[kind_i]][0] += int(hit)
                        agree[TREE_KINDS[kind_i]][1] += 1
                        chance[0] += 1.0 / max(1, kk)
                        chance[1] += 1
                ce = -(b["z"] * torch.log_softmax(out["win"], -1)).sum(-1)
                tot["value"] += float(ce.sum())
                tot["value_n"] += len(rows)
                tot["vp"] += float(((out["vp"] - b["vp"]) ** 2).mean(-1).sum())
                fac = b["seat_fac"].numpy()
                p = np.vectorize(lambda f: prior.get(int(f), 0.25))(fac).astype(np.float64)
                p = p / p.sum(-1, keepdims=True)
                tot["prior"] += float(-(b["z"].numpy() * np.log(np.maximum(p, 1e-9))).sum())
                rnd = s.meta("round")[rows]
                for r, c in zip(rnd.tolist(), ce.tolist()):
                    acc = by_round.setdefault(min(int(r), 9), [0.0, 0])
                    acc[0] += c
                    acc[1] += 1
    n = max(1, tot["value_n"])
    hits = sum(v[0] for v in agree.values())
    cnt = sum(v[1] for v in agree.values())
    return {
        "top1": hits / max(1, cnt),
        "top1_chance": chance[0] / max(1, chance[1]),
        "policy_ce": tot["policy"] / max(1, tot["policy_n"]),
        "top1_by_kind": {k: {"top1": round(v[0] / v[1], 4), "n": v[1]} for k, v in agree.items() if v[1]},
        "value_ce": tot["value"] / n,
        "value_ce_uniform": UNIFORM_CE,
        "value_ce_faction_prior": tot["prior"] / n,
        "vp_mse": tot["vp"] / n,
        "value_ce_by_round": {str(r): round(v[0] / v[1], 4) for r, v in sorted(by_round.items())},
        "records": tot["value_n"],
        "policy_records": cnt,
    }


def train_stream(paths: list[str], out: Path, epochs: int = 3, batch: int = 256, lr: float = 1e-3, weight_decay: float = 1e-4, seed: int = 0,
                 cfg: Config | None = None, pool: int = 6, held_every: int = 20, held_max: int = 8, value_per_game: float = 10.0,
                 max_minutes: float | None = None, max_train_shards: int | None = None, fixtures: int = 200, log=print) -> dict:
    torch.manual_seed(seed)
    rng = np.random.default_rng(seed)
    out.mkdir(parents=True, exist_ok=True)
    all_shards = find_shards(paths)
    train_paths, held_paths = split_shards(all_shards, held_every, held_max)
    if max_train_shards:
        train_paths = train_paths[:max_train_shards]
    held = [load(p, value_per_game) for p in held_paths]
    schema = held[0].schema if held else Shard(train_paths[0]).schema
    first = load(train_paths[0], value_per_game)
    kw = kind_weights(first)
    prior = faction_prior([first])
    net = Net(schema, cfg)
    opt = torch.optim.AdamW(net.parameters(), lr=lr, weight_decay=weight_decay)
    per_shard = len(first)
    steps_total = max(1, epochs * len(train_paths) * ((per_shard + batch - 1) // batch))
    sched = torch.optim.lr_scheduler.OneCycleLR(opt, max_lr=lr, total_steps=steps_total, pct_start=0.03)
    meta = {"train_shards": len(train_paths), "held_shards": [str(p) for p in held_paths], "held_records": sum(len(s) for s in held),
            "parameters": parameter_count(net), "kind_weights": dict(zip(TREE_KINDS, kw.round(3).tolist())), "faction_prior": prior,
            "value_per_game": value_per_game, "epochs": epochs, "batch": batch, "lr": lr}
    log(json.dumps(meta))
    (out / "meta.json").write_text(json.dumps(meta, indent=1))
    started = time.time()
    history = []
    step = 0
    stop = False
    for epoch in range(epochs):
        order = [train_paths[i] for i in rng.permutation(len(train_paths))]
        run = {"loss": 0.0, "policy": 0.0, "value": 0.0, "n": 0, "records": 0}
        for lo in range(0, len(order), pool):
            net.train()
            shards = [load(p, value_per_game) for p in order[lo:lo + pool]]
            for b in pool_batches(shards, batch, rng, kw):
                l = losses(net(b), b)
                opt.zero_grad()
                l.total.backward()
                torch.nn.utils.clip_grad_norm_(net.parameters(), 1.0)
                opt.step()
                if step < steps_total - 1:
                    sched.step()
                step += 1
                run["loss"] += float(l.total.detach())
                run["policy"] += float(l.policy.detach())
                run["value"] += float(l.value.detach())
                run["n"] += 1
                run["records"] += int(b["sys"].shape[0])
                if max_minutes is not None and time.time() - started > max_minutes * 60:
                    stop = True
                    break
            del shards
            el = time.time() - started
            log(f"epoch {epoch + 1} pool {lo // pool + 1}/{(len(order) + pool - 1) // pool}: {run['records']} records, loss {run['loss'] / max(1, run['n']):.4f}, {run['records'] / max(1e-9, el):.0f} records/s overall {el:.0f} s")
            if stop:
                break
        heldr = evaluate(net, held, 512, prior)
        n = max(1, run["n"])
        line = {"epoch": epoch + 1, "train_loss": run["loss"] / n, "train_policy": run["policy"] / n, "train_value": run["value"] / n,
                "train_records": run["records"], "held": heldr, "seconds": round(time.time() - started, 1)}
        history.append(line)
        log(json.dumps(line))
        with open(out / "log.jsonl", "a") as f:
            f.write(json.dumps(line) + "\n")
        torch.save({"state": net.state_dict(), "config": net.cfg.to_dict(), "dims": schema.dims()}, out / "model.pt")
        write_weights(net, out / "weights.t4w")
        if stop:
            break
    manifest = write_fixtures(net, held[0] if held else first, out / "fixtures", n=fixtures, seed=seed)
    return {"history": history, "fixtures": manifest, "parameters": parameter_count(net), "meta": meta}
