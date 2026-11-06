"""Export for the Rust forward pass (lane L4): a flat weights file and the parity fixtures.

Weights file (`.t4w`, format 1), little endian:

```text
b"T4MW"  u32 format (1)  u32 header length H  header (JSON, UTF-8, padded with spaces so that 12 + H is a multiple of 16)
f32 data: every tensor in header order, C order
```

The header: `{"format": 1, "config": {...}, "dims": {...}, "tensors": [{"name", "shape", "offset"}]}` (`offset` in floats
from the start of the data). Names are the PyTorch `state_dict` names (`sys_blocks.0.qkv.weight`, ...); a Linear weight is
`[out, in]` (y = x W^T + b); LayerNorm eps is 1e-5.

Fixtures (a directory): `inputs/` (a plain `.npy` shard of N records, exactly the f16-rounded features Rust reads),
`weights.t4w`, `out_win.npy` `[N, 4]`, `out_vp.npy` `[N, 4]`, `out_logits.npy` `[sum K]` (raw logits, before the mask), all f32,
and `manifest.json` (tolerance, counts).
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

import numpy as np
import torch

from .data import Shard, make_batch
from .model import Config, Net
from .schema import Schema
from .synth import subset, write_shard

MAGIC = b"T4MW"
FORMAT = 1


def write_weights(net: Net, path: str | Path) -> dict:
    tensors, blobs, offset = [], [], 0
    for name, t in net.state_dict().items():
        a = t.detach().cpu().to(torch.float32).numpy().astype("<f4", copy=False)
        tensors.append({"name": name, "shape": list(a.shape), "offset": offset})
        blobs.append(np.ascontiguousarray(a).tobytes())
        offset += a.size
    header = {"format": FORMAT, "config": net.cfg.to_dict(), "dims": net.schema.dims(), "tensors": tensors}
    text = json.dumps(header).encode()
    pad = (16 - (12 + len(text)) % 16) % 16
    text += b" " * pad
    with open(path, "wb") as f:
        f.write(MAGIC)
        f.write(struct.pack("<II", FORMAT, len(text)))
        f.write(text)
        for b in blobs:
            f.write(b)
    return header


def read_weights(path: str | Path) -> tuple[dict, dict[str, np.ndarray]]:
    raw = Path(path).read_bytes()
    if raw[:4] != MAGIC:
        raise ValueError(f"{path}: not a T4MW file")
    fmt, n = struct.unpack("<II", raw[4:12])
    if fmt != FORMAT:
        raise ValueError(f"{path}: format {fmt}, expected {FORMAT}")
    header = json.loads(raw[12:12 + n].decode())
    data = np.frombuffer(raw, dtype="<f4", offset=12 + n)
    out = {}
    for t in header["tensors"]:
        size = int(np.prod(t["shape"])) if t["shape"] else 1
        out[t["name"]] = data[t["offset"]:t["offset"] + size].reshape(t["shape"])
    return header, out


def load_net(path: str | Path, schema: Schema) -> Net:
    header, weights = read_weights(path)
    net = Net(schema, Config(**header["config"]))
    net.load_state_dict({k: torch.from_numpy(np.array(v)) for k, v in weights.items()})
    return net.eval()


def write_fixtures(net: Net, shard: Shard, out_dir: str | Path, n: int = 200, seed: int = 0) -> dict:
    """`n` records of `shard` (a fixed random choice), their outputs, the weights."""
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(seed)
    rows = np.sort(rng.choice(len(shard), size=min(n, len(shard)), replace=False))
    sub = subset({k: np.asarray(v) for k, v in shard.a.items()}, rows.tolist())
    write_shard(out_dir / "inputs", sub, shard.schema, compress=False)
    inputs = Shard(out_dir / "inputs")
    net = net.eval()
    with torch.no_grad():
        b = make_batch(inputs, np.arange(len(inputs)))
        out = net(b)
    win = out["win"].numpy().astype("<f4")
    vp = out["vp"].numpy().astype("<f4")
    valid = b["opt_valid"].numpy()
    logits = out["logits"].numpy()[valid].astype("<f4")
    np.save(out_dir / "out_win.npy", win)
    np.save(out_dir / "out_vp.npy", vp)
    np.save(out_dir / "out_logits.npy", logits)
    write_weights(net, out_dir / "weights.t4w")
    manifest = {"records": int(len(inputs)), "options": int(logits.shape[0]), "tolerance": 1e-4, "feature_version": shard.schema.feature_version,
                "weights": "weights.t4w", "outputs": ["out_win.npy", "out_vp.npy", "out_logits.npy"], "inputs": "inputs"}
    (out_dir / "manifest.json").write_text(json.dumps(manifest, indent=1))
    return manifest
