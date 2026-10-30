#!/usr/bin/env python3
"""Summaries of `tools/engine_core/mutants.sh` reports for the D4 mutation baseline and the ratchet.

  mutation_report.py summarize --module NAME --report R.json [--report R2.json ...] [--wall S] --out SUMMARY.json [--survivors FILE]
  mutation_report.py check     --floors tools/engine_core/mutation_floor.json --summary SUMMARY.json [...]
  mutation_report.py raise     --floors FILE --summary SUMMARY.json [...]       # only ever raises
  mutation_report.py table     --summary SUMMARY.json [...]                      # markdown table
  mutation_report.py selftest

A report is the `report.json` of one mutants.sh run: {"baseline_s", "mutants": [{id, kind, file, line, col, fn, desc,
orig, mutated, status: ok|fail|timeout, by}], "unviable": [ids]}. Several reports of the SAME module (the same points,
different test crates) are merged: a mutant is killed when ANY run killed it (identified by file:line:col + mutated
text, not by id: the id depends on the file list of the run).

Score = killed / (killed + survived); `unviable` points (not compilable under the switch) and `// mutants:skip` lines
are not in the denominator; their number is reported. A timeout counts as killed (docs/ENGINE_COMPUTE.md) and is
counted separately as `timeouts`, so an inflated loop-heavy module can be recognised.
"""
import argparse
import json
import os
import sys


def key(m):
    return (m["file"], m["line"], m["col"], m.get("kind", ""), m["mutated"])


def merge(reports):
    """-> (mutants by key with the best status, unviable count of the first report)."""
    best = {}
    rank = {"ok": 0, "timeout": 1, "fail": 2}
    unv = 0
    for i, rep in enumerate(reports):
        if i == 0:
            unv = len(rep.get("unviable", []))
        for m in rep["mutants"]:
            k = key(m)
            cur = best.get(k)
            if cur is None or rank[m["status"]] > rank[cur["status"]]:
                best[k] = m
    return best, unv


def stat(ms):
    killed = sum(1 for m in ms if m["status"] != "ok")
    timeouts = sum(1 for m in ms if m["status"] == "timeout")
    n = len(ms)
    return {"points": n, "killed": killed, "survived": n - killed, "timeouts": timeouts}


def summarize(module, reports, wall):
    best, unv = merge(reports)
    ms = list(best.values())
    files = {}
    for m in ms:
        files.setdefault(m["file"], []).append(m)
    out = {"module": module, "wall_s": wall, "unviable": unv, **stat(ms)}
    out["score"] = pct(out["killed"], out["points"])
    body = [m for m in ms if m.get("kind") == "body"]
    out["body"] = stat(body)
    out["body"]["score"] = pct(out["body"]["killed"], out["body"]["points"])
    kinds = {}
    for m in ms:
        kinds.setdefault(m.get("kind", "?"), []).append(m)
    out["kinds"] = {k: dict(stat(v), score=pct(sum(1 for m in v if m["status"] != "ok"), len(v))) for k, v in sorted(kinds.items())}
    out["files"] = {f: dict(stat(v), score=pct(sum(1 for m in v if m["status"] != "ok"), len(v)),
                            body_survived=sum(1 for m in v if m.get("kind") == "body" and m["status"] == "ok"))
                    for f, v in sorted(files.items())}
    out["survivors"] = sorted(
        ({"file": m["file"], "line": m["line"], "col": m["col"], "kind": m.get("kind", "?"), "fn": m["fn"],
          "orig": m["orig"], "mutated": m["mutated"]} for m in ms if m["status"] == "ok"),
        key=lambda s: (s["file"], s["line"], s["col"]))
    return out


def pct(k, n):
    """Integer percent rounded DOWN (a floor must never claim more than was measured); 100 for no points."""
    return 100 if n == 0 else (100 * k) // n


def load_summaries(paths):
    return [json.load(open(p)) for p in paths]


def cmd_summarize(a):
    reports = [json.load(open(p)) for p in a.report]
    s = summarize(a.module, reports, a.wall)
    json.dump(s, open(a.out, "w"), indent=1)
    if a.survivors:
        with open(a.survivors, "w") as f:
            f.write(f"# survivors of {a.module}: {s['survived']} of {s['points']} points ({s['score']}% killed)\n")
            for x in s["survivors"]:
                f.write(f"{x['file']}:{x['line']}:{x['col']}\t[{x['fn'] or '-'}]\t{x['kind']}\t{x['orig']}  =>  {x['mutated']}\n")
    print(f"{a.module}: {s['points']} points, {s['killed']} killed, {s['survived']} survived, {s['unviable']} unviable, "
          f"score {s['score']}%, body {s['body']['killed']}/{s['body']['points']} ({s['body']['score']}%), "
          f"timeouts {s['timeouts']}, wall {s['wall_s']}s")


def cmd_check(a):
    floors = json.load(open(a.floors))
    bad = 0
    for s in load_summaries(a.summary):
        f = floors.get(s["module"])
        if f is None:
            print(f"NOFLOOR {s['module']}: {s['score']}% (no floor in {a.floors}: add one with `raise`)")
            continue
        if s["score"] < f:
            bad += 1
            print(f"BELOW   {s['module']}: {s['score']}% < floor {f}% ({s['survived']} survivors)")
            for x in s["survivors"][: a.show]:
                print(f"  SURVIVED {x['file']}:{x['line']}:{x['col']} [{x['fn'] or '-'}] {x['orig']}  =>  {x['mutated']}")
            if s["survived"] > a.show:
                print(f"  ... {s['survived'] - a.show} more (full list: the survivors file of the run)")
        else:
            print(f"ok      {s['module']}: {s['score']}% >= floor {f}% ({s['survived']} survivors)")
    sys.exit(1 if bad else 0)


def cmd_raise(a):
    floors = json.load(open(a.floors)) if os.path.exists(a.floors) else {}
    changed = False
    for s in load_summaries(a.summary):
        old = floors.get(s["module"])
        new = s["score"]
        if old is None or new > old:
            floors[s["module"]] = new
            changed = True
            print(f"floor {s['module']}: {old} -> {new}")
    if changed:
        json.dump(dict(sorted(floors.items())), open(a.floors, "w"), indent=1)
        open(a.floors, "a").write("\n")


def cmd_table(a):
    print("| module | points | killed | survived | unviable | score | body kill | timeouts | wall s |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
    for s in load_summaries(a.summary):
        b = s["body"]
        print(f"| {s['module']} | {s['points']} | {s['killed']} | {s['survived']} | {s['unviable']} | {s['score']}% | "
              f"{b['killed']}/{b['points']} ({b['score']}%) | {s['timeouts']} | {s['wall_s']} |")


def selftest():
    def m(i, line, st, kind="cmp", mut="x"):
        return {"id": i, "kind": kind, "file": "a.rs", "line": line, "col": 1, "fn": "f", "desc": "d", "orig": "o", "mutated": mut, "status": st, "by": ""}
    r1 = {"mutants": [m(1, 1, "ok"), m(2, 2, "fail"), m(3, 3, "ok", "body"), m(4, 4, "timeout")], "unviable": [9]}
    r2 = {"mutants": [m(11, 1, "fail"), m(12, 2, "ok"), m(13, 3, "ok", "body"), m(14, 4, "timeout")], "unviable": []}
    s = summarize("t", [r1], 3)
    assert (s["points"], s["killed"], s["survived"], s["unviable"], s["score"], s["timeouts"]) == (4, 2, 2, 1, 50, 1), s
    assert s["body"] == {"points": 1, "killed": 0, "survived": 1, "timeouts": 0, "score": 0}, s["body"]
    s2 = summarize("t", [r1, r2], 3)
    assert (s2["killed"], s2["survived"]) == (3, 1), s2  # the union kills line 1 and keeps line 3
    assert pct(2, 3) == 66 and pct(0, 0) == 100
    print("selftest ok")


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("summarize")
    s.add_argument("--module", required=True)
    s.add_argument("--report", action="append", required=True)
    s.add_argument("--wall", type=int, default=0)
    s.add_argument("--out", required=True)
    s.add_argument("--survivors")
    c = sub.add_parser("check")
    c.add_argument("--floors", required=True)
    c.add_argument("--summary", action="append", required=True)
    c.add_argument("--show", type=int, default=40)
    r = sub.add_parser("raise")
    r.add_argument("--floors", required=True)
    r.add_argument("--summary", action="append", required=True)
    t = sub.add_parser("table")
    t.add_argument("--summary", action="append", required=True)
    sub.add_parser("selftest")
    a = ap.parse_args()
    if a.cmd == "selftest":
        return selftest()
    {"summarize": cmd_summarize, "check": cmd_check, "raise": cmd_raise, "table": cmd_table}[a.cmd](a)


if __name__ == "__main__":
    main()
