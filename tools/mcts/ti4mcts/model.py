"""The policy + value network (docs/MCTS_BOT_PLAN.md 3.2), CPU sized.

```text
system tokens  [S, FS] --Linear--> [S, d] --2 x block (attention over S + adjacency bias)--> S
seat tokens    [4, FP] ++ faction emb (8) --Linear--> [4, d] (+ seat position)
objective tok. [O, FO] ++ objective emb (16) --Linear--> [O, d]
global token   [FG] ++ ask-kind emb (16) --Linear--> [1, d]
all tokens (+ type emb) --1 block--> pooled h [2d] = (mean of the valid tokens, the actor's seat token)
value:  MLP(h) -> 4 win logits (actor-relative), 4 final VP / 10
policy: per option [h, sys token of ptr0 or 0, obj token of ptr1 or 0, option-kind emb (16), option features] -> MLP -> logit
```

Only these operations are used, so the Rust forward pass (lane L4) is small: Linear, LayerNorm, ReLU, softmax attention with an
additive bias and a key mask, embedding lookup, masked mean, gather. `reference.py` is the same forward pass in NumPy over the
exported weights: the specification of the Rust port, and its parity oracle.
"""

from __future__ import annotations

import math
from dataclasses import asdict, dataclass

import torch
import torch.nn as nn
import torch.nn.functional as F

from .schema import Schema

NEG = -1e9
#: Faction embedding rows: the core's `Faction` index + 1 (row 0 = no seat).
FACTION_ROWS = 35
#: Objective id rows: public index (0..40), secret index + 64 (64..104), + 1 (row 0 = empty).
OBJECTIVE_ROWS = 106


@dataclass
class Config:
    d: int = 64
    heads: int = 4
    ff: int = 128
    sys_blocks: int = 2
    joint_blocks: int = 1
    fac_dim: int = 8
    obj_dim: int = 16
    kind_dim: int = 16
    head_hidden: int = 64

    def to_dict(self) -> dict:
        return asdict(self)


class Block(nn.Module):
    """Pre-LayerNorm transformer block: x + Attn(LN(x)), then x + FF(LN(x)) with ReLU."""

    def __init__(self, d: int, heads: int, ff: int):
        super().__init__()
        self.heads = heads
        self.ln1 = nn.LayerNorm(d)
        self.qkv = nn.Linear(d, 3 * d)
        self.out = nn.Linear(d, d)
        self.ln2 = nn.LayerNorm(d)
        self.ff1 = nn.Linear(d, ff)
        self.ff2 = nn.Linear(ff, d)

    def forward(self, x: torch.Tensor, key_valid: torch.Tensor, bias: torch.Tensor | None = None) -> torch.Tensor:
        b, t, d = x.shape
        h = self.heads
        dh = d // h
        q, k, v = self.qkv(self.ln1(x)).split(d, dim=-1)
        q = q.view(b, t, h, dh).transpose(1, 2)
        k = k.view(b, t, h, dh).transpose(1, 2)
        v = v.view(b, t, h, dh).transpose(1, 2)
        att = (q @ k.transpose(-1, -2)) / math.sqrt(dh)
        if bias is not None:
            att = att + bias
        att = att.masked_fill(~key_valid[:, None, None, :], NEG)
        att = torch.softmax(att, dim=-1)
        y = (att @ v).transpose(1, 2).reshape(b, t, d)
        x = x + self.out(y)
        return x + self.ff2(F.relu(self.ff1(self.ln2(x))))


class Net(nn.Module):
    def __init__(self, schema: Schema, cfg: Config | None = None):
        super().__init__()
        self.schema = schema
        self.cfg = cfg = cfg or Config()
        d = cfg.d
        self.sys_in = nn.Linear(schema.fs, d)
        # Per head: the attention bias of an adjacent pair and of a system to itself.
        self.adj_bias = nn.Parameter(torch.zeros(cfg.heads))
        self.self_bias = nn.Parameter(torch.zeros(cfg.heads))
        self.sys_blocks = nn.ModuleList([Block(d, cfg.heads, cfg.ff) for _ in range(cfg.sys_blocks)])
        self.fac_emb = nn.Embedding(FACTION_ROWS, cfg.fac_dim)
        self.seat_in = nn.Linear(schema.fp + cfg.fac_dim, d)
        self.seat_pos = nn.Parameter(torch.zeros(schema.seats, d))
        self.obj_emb = nn.Embedding(OBJECTIVE_ROWS, cfg.obj_dim)
        self.obj_in = nn.Linear(schema.fo + cfg.obj_dim, d)
        self.ask_emb = nn.Embedding(schema.kind_buckets, cfg.kind_dim)
        self.glob_in = nn.Linear(schema.fg + cfg.kind_dim, d)
        # Token types: system, seat, objective, global.
        self.type_emb = nn.Parameter(torch.zeros(4, d))
        self.joint_blocks = nn.ModuleList([Block(d, cfg.heads, cfg.ff) for _ in range(cfg.joint_blocks)])
        self.ln_out = nn.LayerNorm(d)
        self.v1 = nn.Linear(2 * d, cfg.head_hidden)
        self.v_win = nn.Linear(cfg.head_hidden, schema.seats)
        self.v_vp = nn.Linear(cfg.head_hidden, schema.seats)
        self.kind_emb = nn.Embedding(schema.kind_buckets, cfg.kind_dim)
        self.p1 = nn.Linear(2 * d + d + d + cfg.kind_dim + schema.fa, cfg.head_hidden)
        self.p2 = nn.Linear(cfg.head_hidden, 1)
        nn.init.normal_(self.seat_pos, std=0.02)
        nn.init.normal_(self.type_emb, std=0.02)

    def forward(self, b: dict[str, torch.Tensor]) -> dict[str, torch.Tensor]:
        s, cfg = self.schema, self.cfg
        n = b["sys"].shape[0]
        sys_valid = b["sys_mask"]
        x = self.sys_in(b["sys"])
        eye = torch.eye(s.max_sys, dtype=x.dtype)
        bias = self.adj_bias[None, :, None, None] * b["adj"].to(x.dtype)[:, None] + self.self_bias[None, :, None, None] * eye[None, None]
        for blk in self.sys_blocks:
            x = blk(x, sys_valid, bias)
        fac = self.fac_emb((b["seat_fac"] + 1).clamp(min=0))
        seat = self.seat_in(torch.cat([b["seat"], fac], dim=-1)) + self.seat_pos[None]
        seat_valid = b["seat_fac"] >= 0
        oid = self.obj_emb((b["obj_id"] + 1).clamp(min=0))
        obj = self.obj_in(torch.cat([b["obj"], oid], dim=-1))
        obj_valid = b["obj_mask"]
        glob = self.glob_in(torch.cat([b["glob"], self.ask_emb(b["ask_kind"])], dim=-1))[:, None]
        tokens = torch.cat([x + self.type_emb[0], seat + self.type_emb[1], obj + self.type_emb[2], glob + self.type_emb[3]], dim=1)
        valid = torch.cat([sys_valid, seat_valid, obj_valid, torch.ones(n, 1, dtype=torch.bool)], dim=1)
        for blk in self.joint_blocks:
            tokens = blk(tokens, valid)
        tokens = self.ln_out(tokens)
        w = valid.to(tokens.dtype)[..., None]
        mean = (tokens * w).sum(dim=1) / w.sum(dim=1).clamp(min=1.0)
        me = tokens[:, s.max_sys]
        h = torch.cat([mean, me], dim=-1)
        hv = F.relu(self.v1(h))
        win = self.v_win(hv)
        vp = self.v_vp(hv)
        # Policy.
        k = b["opt"].shape[1]
        sys_tok = tokens[:, : s.max_sys]
        obj_tok = tokens[:, s.max_sys + s.seats : s.max_sys + s.seats + s.max_obj]
        p0 = b["opt_ptr"][..., 0]
        p1 = b["opt_ptr"][..., 1]
        g_sys = torch.gather(sys_tok, 1, p0.clamp(min=0)[..., None].expand(n, k, cfg.d)) * (p0 >= 0)[..., None].to(tokens.dtype)
        g_obj = torch.gather(obj_tok, 1, p1.clamp(min=0)[..., None].expand(n, k, cfg.d)) * (p1 >= 0)[..., None].to(tokens.dtype)
        e = torch.cat([h[:, None].expand(n, k, 2 * cfg.d), g_sys, g_obj, self.kind_emb(b["opt_kind"]), b["opt"]], dim=-1)
        logits = self.p2(F.relu(self.p1(e)))[..., 0]
        return {"win": win, "vp": vp, "logits": logits}


def allowed(b: dict[str, torch.Tensor]) -> torch.Tensor:
    return b["opt_valid"] & b["opt_mask"]


def masked_logits(out: dict, b: dict) -> torch.Tensor:
    return out["logits"].masked_fill(~allowed(b), NEG)


@dataclass
class Losses:
    total: torch.Tensor
    policy: torch.Tensor
    value: torch.Tensor
    vp: torch.Tensor
    agree: float
    agree_n: int


def losses(out: dict, b: dict, smoothing: float = 0.05, w_value: float = 1.0, w_vp: float = 0.1) -> Losses:
    ok = allowed(b)
    logits = masked_logits(out, b)
    logp = torch.log_softmax(logits, dim=-1)
    has = b["has_pi"]
    k_ok = ok.sum(dim=-1, keepdim=True).clamp(min=1).to(logp.dtype)
    target = (1 - smoothing) * b["pi"] + smoothing * ok.to(logp.dtype) / k_ok
    target = target * ok
    if has.any():
        pol = -(target * logp.masked_fill(~ok, 0.0)).sum(dim=-1)[has].mean()
        agree_rows = has
        pred = logits.argmax(dim=-1)
        best = b["pi"].argmax(dim=-1)
        agree = (pred[agree_rows] == best[agree_rows]).float().mean().item()
        agree_n = int(agree_rows.sum().item())
    else:
        pol = logits.sum() * 0.0
        agree, agree_n = 0.0, 0
    val = -(b["z"] * torch.log_softmax(out["win"], dim=-1)).sum(dim=-1).mean()
    vp = F.mse_loss(out["vp"], b["vp"])
    return Losses(total=pol + w_value * val + w_vp * vp, policy=pol, value=val, vp=vp, agree=agree, agree_n=agree_n)


def parameter_count(net: nn.Module) -> int:
    return sum(p.numel() for p in net.parameters())
