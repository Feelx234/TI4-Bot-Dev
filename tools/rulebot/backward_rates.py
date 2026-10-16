#!/usr/bin/env python3
"""Per-objective score rates of a stored `selfplay compare` run: candidate seats against baseline
seats (unconditional: scored in a seat-game / seat-games), for the aliases given.

  tools/rulebot/backward_rates.py results/<key>.jsonl uf baf fwp mew
"""
import json
import math
import sys

path, aliases = sys.argv[1], sys.argv[2:]
cand = {a: 0 for a in aliases}
base = {a: 0 for a in aliases}
n_c = n_b = 0
for line in open(path):
    try:
        g = json.loads(line)
    except ValueError:
        continue
    if "scored" not in g:
        continue
    for seat, scored in enumerate(g["scored"]):
        is_c = g["candidate_seats"][seat]
        if is_c:
            n_c += 1
        else:
            n_b += 1
        for a in aliases:
            if a in scored:
                (cand if is_c else base)[a] += 1
print(f"seat-games: candidate {n_c}, baseline {n_b}")
for a in aliases:
    pc, pb = cand[a] / max(n_c, 1), base[a] / max(n_b, 1)
    se = math.sqrt(pc * (1 - pc) / max(n_c, 1) + pb * (1 - pb) / max(n_b, 1))
    print(f"{a:6s} candidate {cand[a]:4d} ({100*pc:5.2f}%)  baseline {base[a]:4d} ({100*pb:5.2f}%)  diff {100*(pc-pb):+.2f} pp  (+-{196*se:.2f})")
