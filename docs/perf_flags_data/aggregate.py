#!/usr/bin/env python3
"""results.jsonl -> the tables of docs/PERF_FLAGS.md (markdown on stdout) and a chain check."""
import json, re, statistics as st, sys, collections
LOGS = sys.argv[1].rsplit("/", 1)[0] + "/logs"
rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
V = collections.OrderedDict()
for r in rows:
    V.setdefault(r["variant"], []).append(r)
def med(xs): return st.median(xs) if xs else float("nan")
def chain_of(out, v=None, w=None, rep=None):
    if w == "native-quick":
        try:
            t = open(f"{LOGS}/{v}.{w}.{rep}.out").read()
            m = re.search(r"fingerprint ([0-9a-f]{16})", t)
            return m.group(1) if m else "NOFP"
        except OSError:
            return "NOLOG"
    m = re.search(r'"chain":\\?"([0-9a-f]+)', out)
    if m: return m.group(1)
    return out[-60:]
WORKS = ["core-random-4-1", "core-random-4-2", "core-random-4-3", "legacy-random-4-1", "core-record-4-1", "core-replay-4-1", "native-quick"]
info = {}
for v, rs in V.items():
    d = {}
    for r in rs:
        k = r["kind"]
        if k in ("cold_build", "rebuild_noincr", "rebuild_incr", "rebuild_incr_first", "size", "server_cold", "server_rebuild", "wasm_build"):
            d[k] = r
    runs = collections.defaultdict(list)
    for r in rs:
        if r["kind"] == "run": runs[r["work"]].append(r)
    d["runs"] = runs
    info[v] = d
chains = collections.defaultdict(dict)
for v, d in info.items():
    for w, rr in d["runs"].items():
        cs = {chain_of(r["out"], v, w, r["rep"]) for r in rr}
        chains[w][v] = cs
bad = []
for w, m in chains.items():
    allc = set().union(*m.values())
    if len(allc) > 1: bad.append((w, {v: sorted(c) for v, c in m.items()}))
print("CHAIN CHECK:", "ALL IDENTICAL" if not bad else f"MISMATCH {bad}", file=sys.stderr)
for w, m in chains.items(): print(w, sorted(set().union(*m.values())), file=sys.stderr)
def ins(d, ws): return sum(med([r["instructions"] for r in d["runs"][w]]) for w in ws if w in d["runs"])
def cyc(d, ws): return sum(med([r["cycles"] for r in d["runs"][w]]) for w in ws if w in d["runs"])
def usr(d, ws): return sum(med([r["user"] for r in d["runs"][w]]) for w in ws if w in d["runs"])
base = info.get("gate")
R3 = WORKS[:3]
print("| variant | cold build wall s | cold CPU s | rebuild wall s (no incr) | rebuild wall s (incr) | perf_ab MB | instr G (3 random games) | rel | cycles G | rel | user s (3 games) | RSS MB (max) |")
print("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
for v, d in info.items():
    if "cold_build" not in d: continue
    cb = d["cold_build"]; rn = d.get("rebuild_noincr") or {"wall": float("nan")}; ri = d.get("rebuild_incr"); sz = d.get("size") or {"perf_ab": 0}
    i = ins(d, R3) / 1e9; c = cyc(d, R3) / 1e9
    bi = ins(base, R3) / 1e9; bc = cyc(base, R3) / 1e9
    rss = max([r["rss_kb"] for w in R3 if w in d["runs"] for r in d["runs"][w]] or [0]) / 1024
    print(f"| {v} | {cb['wall']:.0f} | {cb['user']+cb['sys']:.0f} | {rn['wall']:.0f} | {ri['wall'] if ri else float('nan'):.0f} | {sz['perf_ab']/1e6:.1f} | {i:.2f} | {i/bi:.3f} | {c:.2f} | {c/bc:.3f} | {usr(d, R3):.2f} | {rss:.1f} |")
print()
print("| variant | " + " | ".join(WORKS) + " |  (instructions, G; relative to gate)")
print("|---|" + "---:|" * len(WORKS))
for v, d in info.items():
    if not d["runs"]: continue
    cells = []
    for w in WORKS:
        if w in d["runs"]:
            x = med([r["instructions"] for r in d["runs"][w]]) / 1e9
            b = med([r["instructions"] for r in base["runs"][w]]) / 1e9
            cells.append(f"{x:.2f} ({x/b:.3f})")
        else: cells.append("")
    print(f"| {v} | " + " | ".join(cells) + " |")
print()
print("| variant | " + " | ".join(WORKS) + " |  (RSS MB, median)")
print("|---|" + "---:|" * len(WORKS))
for v, d in info.items():
    if not d["runs"]: continue
    print(f"| {v} | " + " | ".join(f"{med([r['rss_kb'] for r in d['runs'][w]])/1024:.1f}" if w in d["runs"] else "" for w in WORKS) + " |")
print()
print("| variant | " + " | ".join(WORKS) + " |  (wall s, median, spread)")
print("|---|" + "---:|" * len(WORKS))
for v, d in info.items():
    if not d["runs"]: continue
    print(f"| {v} | " + " | ".join(f"{med([r['wall'] for r in d['runs'][w]]):.2f} ({min(r['wall'] for r in d['runs'][w]):.2f}-{max(r['wall'] for r in d['runs'][w]):.2f})" if w in d["runs"] else "" for w in WORKS) + " |")
print()
print("server cold / rebuild / size / wasm:")
for v, d in info.items():
    if "server_cold" in d:
        s = d["server_cold"]; r = d.get("server_rebuild", {})
        print(f"server {v}: cold wall {s['wall']:.0f} cpu {s['user']+s['sys']:.0f} size {s['size']/1e6:.1f} MB; rebuild wall {r.get('wall', float('nan')):.0f} cpu {r.get('user', 0)+r.get('sys', 0):.0f}")
    if "wasm_build" in d:
        s = d["wasm_build"]; runs = [r for r in V[v] if r["kind"] == "wasm_run"]
        print(f"wasm {v}: cold wall {s['wall']:.0f} cpu {s['user']+s['sys']:.0f} size {s['size']} run ms {[r['ms'] for r in runs]} out {set(r['out'] for r in runs)}")
