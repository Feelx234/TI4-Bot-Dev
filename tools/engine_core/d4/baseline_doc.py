#!/usr/bin/env python3
"""Builds docs/D4_MUTATION_BASELINE.md's tables and the committed survivor lists from the summaries a baseline run left.

  baseline_doc.py RESULTS_DIR [--survivors-dir tools/engine_core/d4/mutation_survivors] [--tables OUT.md]

RESULTS_DIR holds `<module>.summary.json` (tools/engine_core/mutation_ratchet.sh). Prints the markdown tables to stdout (or
--tables). Survivor lists are written compactly (`file:line:col [fn] kind  orig => mutated`, paths relative to the crate src).
"""
import argparse
import glob
import json
import os
import sys

ap = argparse.ArgumentParser()
ap.add_argument("results")
ap.add_argument("--survivors-dir")
ap.add_argument("--tables")
a = ap.parse_args()

sums = [json.load(open(f)) for f in sorted(glob.glob(os.path.join(a.results, "*.summary.json")))]
out = []
out.append("| module | points | killed | survived | unviable | score | body kill | timeouts | wall s |")
out.append("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
for s in sums:
    b = s["body"]
    out.append(f"| {s['module']} | {s['points']} | {s['killed']} | {s['survived']} | {s['unviable']} | {s['score']}% | {b['killed']}/{b['points']} ({b['score']}%) | {s['timeouts']} | {s['wall_s']} |")
tp = sum(s["points"] for s in sums)
tk = sum(s["killed"] for s in sums)
out.append(f"| **total** | {tp} | {tk} | {tp - tk} | {sum(s['unviable'] for s in sums)} | {100 * tk // max(tp, 1)}% | | {sum(s['timeouts'] for s in sums)} | {sum(s['wall_s'] for s in sums)} |")
out.append("")
out.append("Weakest files (more than 20 survivors, by survivors):")
out.append("")
out.append("| file | points | survived | score | body survivors |")
out.append("|---|---:|---:|---:|---:|")
rows = []
for s in sums:
    for f, v in s["files"].items():
        if v["survived"] > 20:
            rows.append((v["survived"], f.split("/src/")[-1] if "factions" not in f else "factions/" + f.split("/src/")[-1], v))
for n, f, v in sorted(rows, reverse=True)[:40]:
    out.append(f"| {f} | {v['points']} | {n} | {v['score']}% | {v['body_survived']} |")
text = "\n".join(out) + "\n"
if a.tables:
    open(a.tables, "w").write(text)
else:
    sys.stdout.write(text)
if a.survivors_dir:
    os.makedirs(a.survivors_dir, exist_ok=True)
    for s in sums:
        name = s["module"].replace("/", "_")
        with open(os.path.join(a.survivors_dir, name + ".txt"), "w") as f:
            f.write(f"# survivors of {s['module']}: {s['survived']} of {s['points']} points ({s['score']}% killed); paths relative to crates/<crate>/src\n")
            for x in s["survivors"]:
                p = x["file"].split("/src/", 1)[-1]
                f.write(f"{p}:{x['line']}:{x['col']} [{x['fn'] or '-'}] {x['kind']}  {x['orig'][:110]} => {x['mutated'][:60]}\n")
