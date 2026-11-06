"""The forward pass in plain NumPy over an exported weights file, one record at a time: the specification the Rust port (lane
L4) follows, and the oracle its parity test is checked against (`tests/test_export.py` checks it against PyTorch).

Every step is written as the Rust code will do it: no batching, no padding, the options of one decision as a matrix.
"""

from __future__ import annotations

import numpy as np

NEG = -1e9
EPS = 1e-5


def linear(w: dict, name: str, x: np.ndarray) -> np.ndarray:
    return x @ w[f"{name}.weight"].T + w[f"{name}.bias"]


def layer_norm(w: dict, name: str, x: np.ndarray) -> np.ndarray:
    mu = x.mean(axis=-1, keepdims=True)
    var = ((x - mu) ** 2).mean(axis=-1, keepdims=True)
    return (x - mu) / np.sqrt(var + EPS) * w[f"{name}.weight"] + w[f"{name}.bias"]


def relu(x: np.ndarray) -> np.ndarray:
    return np.maximum(x, 0.0)


def block(w: dict, name: str, x: np.ndarray, key_valid: np.ndarray, heads: int, bias: np.ndarray | None = None) -> np.ndarray:
    t, d = x.shape
    dh = d // heads
    qkv = linear(w, f"{name}.qkv", layer_norm(w, f"{name}.ln1", x))
    q, k, v = qkv[:, :d], qkv[:, d:2 * d], qkv[:, 2 * d:]
    y = np.zeros_like(x)
    for h in range(heads):
        sl = slice(h * dh, (h + 1) * dh)
        att = (q[:, sl] @ k[:, sl].T) / np.sqrt(dh)
        if bias is not None:
            att = att + bias[h]
        att = np.where(key_valid[None, :], att, NEG)
        att = att - att.max(axis=-1, keepdims=True)
        p = np.exp(att)
        p /= p.sum(axis=-1, keepdims=True)
        y[:, sl] = p @ v[:, sl]
    x = x + linear(w, f"{name}.out", y)
    return x + linear(w, f"{name}.ff2", relu(linear(w, f"{name}.ff1", layer_norm(w, f"{name}.ln2", x))))


def forward(header: dict, w: dict, rec: dict) -> dict:
    """One record: `rec` holds the record's arrays (`sys [S, FS]`, `sys_id [S]`, `adj [S, S]` bool, `seat [4, FP]`, `seat_fac [4]`,
    `obj [O, FO]`, `obj_id [O]`, `glob [FG]`, `ask_kind` int, `opt [K, FA]`, `opt_kind [K]`, `opt_ptr [K, 3]`), float32 features.
    Returns `win [4]`, `vp [4]`, `logits [K]` (raw)."""
    cfg, dims = header["config"], header["dims"]
    heads, d = cfg["heads"], cfg["d"]
    S, P, O = dims["max_sys"], dims["seats"], dims["max_obj"]
    f32 = np.float32
    sys_valid = rec["sys_id"] >= 0
    x = linear(w, "sys_in", rec["sys"].astype(f32))
    adj = rec["adj"].astype(f32)
    eye = np.eye(S, dtype=f32)
    bias = w["adj_bias"][:, None, None] * adj[None] + w["self_bias"][:, None, None] * eye[None]
    for i in range(cfg["sys_blocks"]):
        x = block(w, f"sys_blocks.{i}", x, sys_valid, heads, bias)
    fac = w["fac_emb.weight"][np.maximum(rec["seat_fac"].astype(np.int64) + 1, 0)]
    seat = linear(w, "seat_in", np.concatenate([rec["seat"].astype(f32), fac], axis=-1)) + w["seat_pos"]
    seat_valid = rec["seat_fac"] >= 0
    oid = w["obj_emb.weight"][np.maximum(rec["obj_id"].astype(np.int64) + 1, 0)]
    obj = linear(w, "obj_in", np.concatenate([rec["obj"].astype(f32), oid], axis=-1))
    obj_valid = rec["obj_id"] >= 0
    glob = linear(w, "glob_in", np.concatenate([rec["glob"].astype(f32), w["ask_emb.weight"][int(rec["ask_kind"])]]))[None]
    te = w["type_emb"]
    tokens = np.concatenate([x + te[0], seat + te[1], obj + te[2], glob + te[3]], axis=0)
    valid = np.concatenate([sys_valid, seat_valid, obj_valid, [True]])
    for i in range(cfg["joint_blocks"]):
        tokens = block(w, f"joint_blocks.{i}", tokens, valid, heads)
    tokens = layer_norm(w, "ln_out", tokens)
    mean = tokens[valid].sum(axis=0) / max(1.0, float(valid.sum()))
    h = np.concatenate([mean, tokens[S]])
    hv = relu(linear(w, "v1", h))
    win = linear(w, "v_win", hv)
    vp = linear(w, "v_vp", hv)
    k = rec["opt"].shape[0]
    rows = []
    for j in range(k):
        p0, p1 = int(rec["opt_ptr"][j, 0]), int(rec["opt_ptr"][j, 1])
        g_sys = tokens[p0] if p0 >= 0 else np.zeros(d, f32)
        g_obj = tokens[S + P + p1] if p1 >= 0 else np.zeros(d, f32)
        rows.append(np.concatenate([h, g_sys, g_obj, w["kind_emb.weight"][int(rec["opt_kind"][j])], rec["opt"][j].astype(f32)]))
    e = np.stack(rows) if rows else np.zeros((0, 4 * d + cfg["kind_dim"] + dims["fa"]), f32)
    logits = linear(w, "p2", relu(linear(w, "p1", e)))[:, 0]
    return {"win": win, "vp": vp, "logits": logits}


def record(shard, i: int) -> dict:
    """Record `i` of a `data.Shard` as the arrays [`forward`] takes."""
    from .data import unpack_adj

    a, s = shard.a, shard.schema
    lo, hi = int(a["opt_off"][i]), int(a["opt_off"][i + 1])
    return {
        "sys": np.asarray(a["sys"][i], np.float32), "sys_id": np.asarray(a["sys_id"][i]),
        "adj": unpack_adj(np.asarray(a["adj"][i : i + 1]), s.max_sys)[0],
        "seat": np.asarray(a["seat"][i], np.float32), "seat_fac": np.asarray(a["seat_fac"][i]),
        "obj": np.asarray(a["obj"][i], np.float32), "obj_id": np.asarray(a["obj_id"][i]),
        "glob": np.asarray(a["glob"][i], np.float32), "ask_kind": int(a["ask_kind"][i]),
        "opt": np.asarray(a["opt"][lo:hi], np.float32), "opt_kind": np.asarray(a["opt_kind"][lo:hi]), "opt_ptr": np.asarray(a["opt_ptr"][lo:hi]),
    }
