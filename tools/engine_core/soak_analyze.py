#!/usr/bin/env python3
"""Reads what one soak slice left behind and keeps the soak's state and reports (docs/ENGINE_SOAK.md).

  soak_analyze.py slice SLICE_DIR STATE REPORT_ROOT   analyse SLICE_DIR (meta.json, specs.jsonl, shards/, merged.txt),
                                                      write SLICE_DIR/result.json, fold it into STATE
  soak_analyze.py plan STATE "LANES" SLICE_GAMES SEED_BASE "MASK_NAMES"   the next round, one slice per line
  soak_analyze.py round STATE | meta STATE KEY VALUE  bump the round counter | record a run property for the report
  soak_analyze.py report STATE OUT.md                 write the rolling summary (LATEST.md)

State is JSON: finished slices, next seed per lane, cumulative counters, differences by signature (first example kept),
window totals per label (compared / bailed), silent-bail findings. Plain stdlib python.
"""
import glob
import json
import os
import re
import sys
import time

CATEGORIES = ["State", "Bytes", "Value", "Decision", "DecisionHash", "Event", "Rng", "Cost"]  # only used for display


def load_shards(dirpath):
    games = {}
    files = sorted(glob.glob(os.path.join(dirpath, "shard-*.json")))
    for f in files:
        try:
            doc = json.load(open(f))
        except Exception:  # a shard that failed before writing its file
            continue
        for rec in doc.get("records", []):
            games[rec["index"]] = rec
    return games, len(files)


def signature(d):
    path = re.sub(r"\[\d+\]", "[]", d.get("path", ""))
    path = re.sub(r"\.\d+(?=\.|$)", ".#", path)
    return f'{d["area"]}/{d["category"]} {d["label"]} @ {path}'


def trunc(s, n=240):
    s = str(s)
    return s if len(s) <= n else s[:n] + "..."


def empty_state():
    return {"version": 1, "created": time.strftime("%Y-%m-%dT%H:%M:%S%z"), "finished": [], "next_seed": {}, "round": 0,
            "totals": {"slices": 0, "games": 0, "calls": 0, "compared": 0, "differences": 0, "hash_changes": 0, "failed_slices": 0,
                       "seconds": 0.0},
            "by_mask": {}, "by_lane": {}, "differences": {}, "windows": {}, "areas": {}, "recent": [], "silent": {}, "failures": []}


def analyse_slice(slice_dir, state_path, report_root):
    meta = json.load(open(os.path.join(slice_dir, "meta.json")))
    specs = {}
    sp = os.path.join(slice_dir, "specs.jsonl")
    if os.path.exists(sp):
        for line in open(sp):
            line = line.strip()
            if line.startswith("{"):
                g = json.loads(line)
                specs[g["index"]] = g
    games, nfiles = load_shards(os.path.join(slice_dir, "shards"))
    merged = ""
    mp = os.path.join(slice_dir, "merged.txt")
    if os.path.exists(mp):
        merged = open(mp).read()
    # exit 1 of a shard is "a difference or hash change" (its file is written); anything else is a crash or kill.
    failed_shards = [m[0] for m in re.findall(r"^SHARD (\d+/\d+) FAILED \(exit (\d+)", merged, re.M) if m[1] != "1"]
    refused = "MERGE REFUSED" in merged
    expected = meta["games"]

    res = {"id": meta["id"], "lane": meta["lane"], "mask": meta["mask"], "mask_name": meta["mask_name"],
           "first_seed": meta["first_seed"], "games_planned": expected, "games_played": len(games),
           "seconds": meta.get("seconds", 0), "exit": meta.get("exit"), "failed_shards": failed_shards, "merge_refused": refused,
           "calls": 0, "compared": 0, "areas": {}, "windows": {}, "differences": [], "dropped": 0, "hash_changes": [],
           "silent_windows": [], "silent_areas": [], "config": meta["config"], "dual_run": meta["dual_run"]}
    mask_areas = [a.split("=")[0].strip() for a in meta["mask"].split(",") if a.strip().endswith("=dual")]
    for idx in sorted(games):
        rec = games[idx]
        rep = rec["report"]
        spec = specs.get(idx, {})
        if rec["dual_hash"] != rec["string_hash"]:
            res["hash_changes"].append({"game": idx, "seed": spec.get("seed"), "dual": rec["dual_hash"], "string": rec["string_hash"],
                                        "repro": repro_line(meta, idx, spec)})
        for name, c in rep["areas"].items():
            a = res["areas"].setdefault(name, {"calls": 0, "compared": 0, "differing": 0})
            a["calls"] += c["calls"]
            a["compared"] += sum(c["compared"])
            a["differing"] += sum(c["differing"])
            res["calls"] += c["calls"]
            res["compared"] += sum(c["compared"])
        for label, w in rep["windows"].items():
            t = res["windows"].setdefault(label, [0, 0])
            t[0] += w["compared"]
            t[1] += w["bailed"]
        res["dropped"] += rep.get("dropped", 0)
        for d in rep["differences"]:
            res["differences"].append({"game": idx, "seed": spec.get("seed"), "map": spec.get("map"), "players": spec.get("players"),
                                       "factions": spec.get("factions"), "rules_version": spec.get("rules_version", "newest"),
                                       "thunders_edge": spec.get("thunders_edge", False), "area": d["area"], "category": d["category"],
                                       "label": d["label"], "path": d["path"], "string": trunc(d["string_side"]),
                                       "core": trunc(d["core_side"]), "repro": repro_line(meta, idx, spec)})
    # Silent bails: a window that ran and never compared; an area on dual that made no call at all in a real slice.
    # ("label [bail: reason]" rows are the bail reasons of a label, compared 0 by construction: not silent.)
    res["silent_windows"] = sorted(l for l, (c, b) in res["windows"].items() if c == 0 and b > 0 and " [bail:" not in l)
    if len(games) >= 8:
        res["silent_areas"] = sorted(a for a in mask_areas if a != "all" and res["areas"].get(a, {}).get("calls", 0) == 0)
    # Fold into the state.
    st = json.load(open(state_path)) if os.path.exists(state_path) else empty_state()
    ok = not failed_shards and not refused and len(games) == expected
    if meta["id"] not in st["finished"]:
        st["finished"].append(meta["id"])
    st["next_seed"][meta["lane"]] = max(st["next_seed"].get(meta["lane"], 0), meta["first_seed"] + expected)
    t = st["totals"]
    t["slices"] += 1
    t["games"] += len(games)
    t["calls"] += res["calls"]
    t["compared"] += res["compared"]
    t["differences"] += len(res["differences"]) + res["dropped"]
    t["hash_changes"] += len(res["hash_changes"])
    t["seconds"] += float(res["seconds"] or 0)
    if not ok:
        t["failed_slices"] += 1
        st["failures"].append({"id": meta["id"], "failed_shards": failed_shards, "merge_refused": refused,
                               "games_played": len(games), "games_planned": expected, "dir": slice_dir})
        st["failures"] = st["failures"][-20:]
    for key, bucket in (("by_mask", meta["mask_name"]), ("by_lane", meta["lane"])):
        b = st[key].setdefault(bucket, {"slices": 0, "games": 0, "calls": 0, "compared": 0, "differences": 0})
        b["slices"] += 1
        b["games"] += len(games)
        b["calls"] += res["calls"]
        b["compared"] += res["compared"]
        b["differences"] += len(res["differences"]) + res["dropped"]
    for name, a in res["areas"].items():
        t2 = st["areas"].setdefault(name, {"calls": 0, "compared": 0, "differing": 0})
        for k in t2:
            t2[k] += a[k]
    for label, (c, b) in res["windows"].items():
        w = st["windows"].setdefault(label, [0, 0])
        w[0] += c
        w[1] += b
    for d in res["differences"]:
        sig = signature(d)
        e = st["differences"].setdefault(sig, {"count": 0, "first": d, "slices": []})
        e["count"] += 1
        if meta["id"] not in e["slices"] and len(e["slices"]) < 10:
            e["slices"].append(meta["id"])
    for h in res["hash_changes"]:
        e = st["differences"].setdefault("HASH CHANGED (dual vs string decision hash)", {"count": 0, "first": h, "slices": []})
        e["count"] += 1
        if meta["id"] not in e["slices"] and len(e["slices"]) < 10:
            e["slices"].append(meta["id"])
    for l in res["silent_windows"]:
        s = st["silent"].setdefault("window " + l, {"slices": 0, "first": meta["id"]})
        s["slices"] += 1
    for a in res["silent_areas"]:
        s = st["silent"].setdefault("area " + a, {"slices": 0, "first": meta["id"]})
        s["slices"] += 1
    st["recent"].append({"id": meta["id"], "ok": ok, "games": len(games), "calls": res["calls"], "compared": res["compared"],
                         "differences": len(res["differences"]) + res["dropped"], "hash_changes": len(res["hash_changes"]),
                         "seconds": round(float(res["seconds"] or 0)), "silent_windows": len(res["silent_windows"]),
                         "when": time.strftime("%H:%M:%S")})
    st["recent"] = st["recent"][-40:]
    json.dump(res, open(os.path.join(slice_dir, "result.json"), "w"), indent=1)
    tmp = state_path + ".tmp"
    json.dump(st, open(tmp, "w"), indent=1)
    os.replace(tmp, state_path)
    # The slice's own short summary line (stdout, for the log).
    print(f'{meta["id"]}: games {len(games)}/{expected} calls {res["calls"]} compared {res["compared"]} '
          f'differences {len(res["differences"]) + res["dropped"]} hash_changes {len(res["hash_changes"])} '
          f'silent_windows {len(res["silent_windows"])} {"OK" if ok else "INCOMPLETE"}')
    return 0 if ok else 1


def repro_line(meta, idx, spec):
    seed = spec.get("seed")
    flag = f"--seed {seed}" if seed is not None else f"--game {idx}"
    return f'{meta["dual_run"]} {meta["config"]} {meta["games"]} "{meta["mask"]}" {flag}'


def write_report(state_path, out):
    st = json.load(open(state_path))
    t = st["totals"]
    L = []
    L.append("# Soak summary")
    L.append("")
    L.append(f'Updated {time.strftime("%Y-%m-%d %H:%M:%S %Z")} (state created {st["created"]}). '
             f'{t["slices"]} slices, {t["games"]} games, {t["calls"]:,} calls, {t["compared"]:,} comparisons, '
             f'**{t["differences"]} differences, {t["hash_changes"]} hash changes**, {t["failed_slices"]} incomplete slices, '
             f'{t["seconds"] / 3600:.1f} h of slice time.')
    run = st.get("run", {})
    if run:
        L.append("Run: " + ", ".join(f"{k} `{v}`" for k, v in run.items()))
        L.append("")
    verdict = "CLEAN so far" if not st["differences"] and not st["failures"] else "ATTENTION"
    L.append(f"Verdict: **{verdict}**" + ("" if verdict.startswith("CLEAN") else f' ({len(st["differences"])} distinct difference signatures, {len(st["failures"])} failures)'))
    L.append("")
    if st["differences"]:
        L.append("## Differences (grouped by area and call site; first example each)")
        L.append("")
        groups = {}
        for sig, e in st["differences"].items():
            f = e["first"]
            key = f"{f['area']} {f['label']}" if "area" in f else sig
            groups.setdefault(key, []).append((sig, e))
        for key, items in sorted(groups.items(), key=lambda kv: -sum(e["count"] for _, e in kv[1])):
            total = sum(e["count"] for _, e in items)
            f0 = items[0][1]["first"]
            L.append(f"### {key}: {total} differences, {len(items)} distinct leaf paths")
            if "area" in f0:
                L.append(f'First example: game seed {f0.get("seed")}, map {f0.get("map")}, {f0.get("players")}p, rules {f0.get("rules_version")}, '
                         f'TE {f0.get("thunders_edge")}, factions {",".join(f0.get("factions") or [])}; slices {", ".join(items[0][1]["slices"][:3])}')
                L.append(f'Repro: `tools/engine_core/soak_repro.sh --line \'{f0.get("repro")}\'`')
            else:
                L.append(f'Repro: `tools/engine_core/soak_repro.sh --line \'{f0.get("repro")}\' --hash`')
            for sig, e in sorted(items, key=lambda it: -it[1]["count"])[:6]:
                f = e["first"]
                if "area" in f:
                    L.append(f'- `{f["category"]} @ {f["path"] or "(whole value)"}` x{e["count"]}: string = {trunc(f["string"], 160)} | core = {trunc(f["core"], 160)}')
            L.append("")
    if st["failures"]:
        L.append("## Incomplete slices")
        L.append("")
        for f in st["failures"]:
            L.append(f'- {f["id"]}: games {f["games_played"]}/{f["games_planned"]}, failed shards {f["failed_shards"]}, '
                     f'merge refused {f["merge_refused"]}; logs in `{f["dir"]}`')
        L.append("")
    if st["silent"]:
        L.append("## Silent bails (window ran, core compared nothing / area on dual with no call)")
        L.append("")
        for k, s in sorted(st["silent"].items()):
            L.append(f'- {k}: in {s["slices"]} slices (first {s["first"]})')
        L.append("")
    L.append("## By mask and lane")
    L.append("")
    L.append("| group | slices | games | calls | comparisons | differences |")
    L.append("|---|---|---|---|---|---|")
    for key in ("by_mask", "by_lane"):
        for name, b in sorted(st[key].items()):
            L.append(f'| {key[3:]} {name} | {b["slices"]} | {b["games"]} | {b["calls"]:,} | {b["compared"]:,} | {b["differences"]} |')
    L.append("")
    L.append("## Bails per window label (compared / bailed over the whole soak; bail share, worst first, labels that bailed at least once)")
    L.append("")
    rows = [(l, c, b) for l, (c, b) in st["windows"].items() if b > 0 and " [bail:" not in l]
    rows.sort(key=lambda r: -(r[2] / max(1, r[1] + r[2])))
    L.append("| label | compared | bailed | bail share |")
    L.append("|---|---|---|---|")
    for l, c, b in rows[:40]:
        L.append(f"| {l} | {c:,} | {b:,} | {100 * b / max(1, c + b):.0f}% |")
    if len(rows) > 40:
        L.append(f"| ... {len(rows) - 40} more labels | | | |")
    L.append("")
    reasons = sorted(((l, b) for l, (c, b) in st["windows"].items() if " [bail:" in l), key=lambda r: -r[1])
    if reasons:
        L.append("")
        L.append("## Bail reasons (top 30 by count, whole soak)")
        L.append("")
        for l, b in reasons[:30]:
            L.append(f"- {b:,} x {l}")
    L.append("")
    L.append("## Next seeds per lane")
    L.append("")
    L.append(", ".join(f"{k}: {v}" for k, v in sorted(st["next_seed"].items())) or "(none yet)")
    L.append("")
    L.append("## Recent slices")
    L.append("")
    L.append("| time | slice | games | calls | comparisons | diffs | hash | silent windows | s | ok |")
    L.append("|---|---|---|---|---|---|---|---|---|---|")
    for r in reversed(st["recent"][-25:]):
        L.append(f'| {r["when"]} | {r["id"]} | {r["games"]} | {r["calls"]:,} | {r["compared"]:,} | {r["differences"]} | '
                 f'{r["hash_changes"]} | {r["silent_windows"]} | {r["seconds"]} | {"y" if r["ok"] else "NO"} |')
    L.append("")
    L.append("Repro of a difference: `tools/engine_core/soak_repro.sh --line '<repro line>'` (or `--record result.json N`), see docs/ENGINE_SOAK.md.")
    tmp = out + ".tmp"
    open(tmp, "w").write("\n".join(L) + "\n")
    os.replace(tmp, out)


def plan(state_path, lanes, slice_games, seed_base, masks):
    """One round: `lane first_seed games mask_name` lines (a lane's games = its share of SLICE_GAMES, at least 2)."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import soak_corpus
    st = json.load(open(state_path)) if os.path.exists(state_path) else empty_state()
    weights = {l: soak_corpus.LANES[l][1] for l in lanes}
    total = sum(weights.values())
    for i, lane in enumerate(lanes):
        mname = masks[(st["round"] + i) % len(masks)]
        # `all=dual` runs every area's comparison at every call (1.3 M calls per game instead of ~300): fewer games per slice.
        scale = float(dict(kv.split("=") for kv in os.environ.get("SOAK_MASK_SCALE", "all=0.3").split(",") if kv).get(mname, 1))
        games = max(2, round(slice_games * weights[lane] / total * scale))
        first = st["next_seed"].get(lane, seed_base)
        print(lane, max(first, seed_base), games, mname)


def bump(state_path, key, value=None):
    st = json.load(open(state_path)) if os.path.exists(state_path) else empty_state()
    if key == "round":
        st["round"] += 1
    else:
        st.setdefault("run", {})[key] = value
    tmp = state_path + ".tmp"
    json.dump(st, open(tmp, "w"), indent=1)
    os.replace(tmp, state_path)


def main(argv):
    if len(argv) >= 6 and argv[0] == "plan":
        plan(argv[1], argv[2].split(), int(argv[3]), int(argv[4]), argv[5].split())
        return 0
    if len(argv) >= 3 and argv[0] == "round":
        bump(argv[1], "round")
        return 0
    if len(argv) >= 4 and argv[0] == "meta":
        bump(argv[1], argv[2], argv[3])
        return 0
    if len(argv) >= 4 and argv[0] == "slice":
        return analyse_slice(argv[1], argv[2], argv[3])
    if len(argv) >= 3 and argv[0] == "report":
        write_report(argv[1], argv[2])
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
