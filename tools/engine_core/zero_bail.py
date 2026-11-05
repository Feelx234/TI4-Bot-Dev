#!/usr/bin/env python3
"""The zero-bail acceptance check (user rule: a whole game plays in the new engine without a bail, on the agreed content: everything
except Thunder's Edge and the dropped factions/features).

    tools/engine_core/zero_bail.py [--enforce] [--classes FILE] [--native LOG]... [--bench JSON]... [--require-bench N]
                                   [--root DIR] [--no-evidence] REPORT.txt...

What it checks, and what each input is:

1. NATIVE EVIDENCE (the rule itself). `--native LOG` is the output of `native_play` (whole games on the core alone, every gap counted;
   its last line says `ZERO GAP` or not); `--bench JSON` is the result file of a rule-bot bench run on the native wrapper
   (`selfplay run --out`, steps bench-*): every game must end in a `victory`, `problems` must be empty. `--require-bench N` (default
   6 with --enforce) wants N distinct bench configs among the files. These are the numbers that say "a whole game plays natively".

2. THE DUAL'S COUNTERS. REPORT.txt are the texts `dual_run` / `dual_sharded.sh` print for runs with every area on Dual (the golden 20,
   the lent set, ...). The dual plays the STRING engine and lends windows to the core; a counted bail (`<label> [bail: <why>]`) or gate
   (`<label> [gate: <why>]`: a position the call site runs bare) means the dual's adapter did not compare that window, NOT that the
   core stopped. Every cause must therefore be classified in `zero_bail_classes.tsv` (the first row whose label and why match):
     census   a body of the string driver that the core runs natively (step arms, reconcile hooks, turn-end hooks);
     adapter  a lent window the dual's adapter declines that the native path plays (lockstep tests as evidence);
     gap      a real gap of the native path: fails in enforce mode.
   A cause no row matches ("unclassified") and a bail with no recorded reason ("silent") fail in enforce mode: a new cause is never
   hidden. Each row names its evidence as `path::token`; the evidence is checked against the tree (a renamed test breaks the step).
   The `d1::pending` rows (a step with nothing open to compare) are not lends and are left out.

Without --enforce the report is printed and the exit status is 0; with --enforce (end_of_d1.sh, ZERO_BAIL=enforce) any failure of 1 or 2
exits 1.
"""
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from lent_census import NO_REASON, parse  # noqa: E402

CLASSES = ("census", "adapter", "gap")
DEFAULT_CLASSES = os.path.join(HERE, "zero_bail_classes.tsv")
DEFAULT_ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))


def load_classes(path):
    """The rows of the class table: (class, label regex, why regex, reason, [(file, token)])."""
    rows = []
    with open(path, encoding="utf-8") as fh:
        lines = fh.read().split("\n")
    for n, line in enumerate(lines, 1):
        line = line.rstrip("\n")
        if not line.strip() or line.startswith("#"):
            continue
        cols = line.split("\t")
        if len(cols) != 5:
            raise ValueError(f"{path}:{n}: {len(cols)} columns, want 5 (class, label, why, reason, evidence)")
        cls, label, why, reason, evidence = cols
        if cls not in CLASSES:
            raise ValueError(f"{path}:{n}: unknown class {cls!r}")
        items = []
        for item in evidence.split(" ; "):
            item = item.strip()
            if not item:
                continue
            if "::" not in item:
                raise ValueError(f"{path}:{n}: evidence {item!r} is not path::token")
            f, _, token = item.partition("::")
            items.append((f, token))
        if cls != "gap" and not items:
            raise ValueError(f"{path}:{n}: a {cls} row needs evidence")
        rows.append((cls, re.compile(label), re.compile(why), reason, items))
    return rows


REPORT_HEAD = re.compile(r"^games \d+ \| areas on dual: .*\| hashes equal: (true|false)")
REPORT_END = re.compile(r"^dual: \d+ calls, \d+ comparisons, \d+ differences")


def report_problem(path):
    """Why `path` is no complete dual report (the run died before the merge printed it), or None. A log of a dead run counts no cause,
    so without this check it would pass for a clean corpus (run A read the log of the panicked dual-lent step as a report)."""
    head = end = False
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            for line in fh:
                head = head or bool(REPORT_HEAD.match(line))
                end = end or bool(REPORT_END.match(line))
    except OSError as e:
        return f"{path}: cannot be read ({e.strerror})"
    if not (head and end):
        return f"{path}: not a dual report (no `games N | areas on dual ...` header and `dual: N calls ...` line: the step that wrote it died)"
    return None


def causes_of(files):
    """`{(label, kind, why): count}` over the dual reports (the d1::pending rows are not lends)."""
    causes = {}
    for path in files:
        _, reasons = parse(path)
        for label, rs in reasons.items():
            if label == "d1::pending" or label.startswith("d1::pending."):
                continue
            for why, n in rs:
                kind = "gate" if why.startswith("gate:") else "bail"
                text = why[len("gate: "):] if kind == "gate" else why
                causes[(label, kind, text)] = causes.get((label, kind, text), 0) + n
    return causes


def classify(label, why, rows):
    """`(class, row index)`: 'silent' for a bail without a recorded reason, 'unclassified' for a cause no row matches."""
    if why == NO_REASON:
        return "silent", None
    for i, (cls, lrx, wrx, _, _) in enumerate(rows):
        if lrx.fullmatch(label) and wrx.fullmatch(why):
            return cls, i
    return "unclassified", None


def verify_evidence(rows, root):
    """The `path::token` items that do not hold in the tree: a list of messages."""
    bad = []
    cache = {}
    for cls, _, _, reason, items in rows:
        for f, token in items:
            full = os.path.join(root, f)
            if full not in cache:
                try:
                    with open(full, encoding="utf-8", errors="replace") as fh:
                        cache[full] = fh.read()
                except OSError:
                    cache[full] = None
            text = cache[full]
            if text is None:
                bad.append(f"evidence file missing: {f}")
            elif token not in text:
                bad.append(f"evidence token not in {f}: {token!r}")
    return sorted(set(bad))


NATIVE_LAST = re.compile(r"^native_play: (\d+) games \| (\d+) finished with zero gaps \| (\d+) gaps\b.*")


def native_verdict(path):
    """`(ok, text)` for the output of native_play: every game finished with zero gaps and the line says ZERO GAP."""
    last = None
    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            if NATIVE_LAST.match(line):
                last = line.strip()
    if last is None:
        return False, f"{path}: no `native_play: N games | ...` summary line (the run died or is not a native_play log)"
    m = NATIVE_LAST.match(last)
    games, finished, gaps = int(m.group(1)), int(m.group(2)), int(m.group(3))
    ok = games > 0 and games == finished and gaps == 0 and last.endswith("ZERO GAP")
    return ok, f"{os.path.basename(path)}: {games} games, {finished} finished with zero gaps, {gaps} gaps" + ("" if ok else "  <-- NOT ZERO GAP")


def bench_verdict(path):
    """`(ok, text, config)` for a selfplay result: every game a victory, no problem."""
    with open(path, encoding="utf-8") as fh:
        d = json.load(fh)
    games = d.get("games", [])
    ends = {}
    for g in games:
        ends[g.get("termination")] = ends.get(g.get("termination"), 0) + 1
    problems = d.get("problems", [])
    ok = bool(games) and set(ends) == {"victory"} and not problems
    ends_text = ", ".join(f"{n} {k}" for k, n in sorted(ends.items(), key=lambda kv: str(kv[0])))
    return ok, f"{os.path.basename(os.path.dirname(path))}/{os.path.basename(path)}: {len(games)} games ({ends_text}), {len(problems)} problems" + ("" if ok else "  <-- NOT A WHOLE-GAME RUN"), os.path.basename(path)


def main(argv):
    enforce = "--enforce" in argv
    classes_path, root = DEFAULT_CLASSES, DEFAULT_ROOT
    natives, benches, files = [], [], []
    require_bench = None
    evidence = True
    it = iter(argv)
    for a in it:
        if a == "--enforce":
            continue
        if a == "--no-evidence":
            evidence = False
        elif a in ("--classes", "--root", "--native", "--bench", "--require-bench"):
            v = next(it, None)
            if v is None:
                print(f"zero-bail: {a} needs a value", file=sys.stderr)
                return 2
            if a == "--classes":
                classes_path = v
            elif a == "--root":
                root = v
            elif a == "--native":
                natives.append(v)
            elif a == "--bench":
                benches.append(v)
            else:
                require_bench = int(v)
        elif a.startswith("--"):
            print(f"zero-bail: unknown option {a}", file=sys.stderr)
            return 2
        else:
            files.append(a)
    if not files and not natives and not benches:
        print(__doc__)
        return 2
    if require_bench is None:
        require_bench = 6 if enforce else 0
    rows = load_classes(classes_path)
    failures = []

    # 1. the native evidence
    print("zero-bail: native evidence (whole games on the core alone)")
    if enforce and not natives:
        failures.append("no --native log given: the native playthrough is the rule itself")
    for path in natives:
        ok, text = native_verdict(path)
        print(f"  {'ok  ' if ok else 'FAIL'} {text}")
        if not ok:
            failures.append(text)
    configs = set()
    for path in benches:
        ok, text, config = bench_verdict(path)
        print(f"  {'ok  ' if ok else 'FAIL'} {text}")
        if ok:
            configs.add(config)
        else:
            failures.append(text)
    if len(configs) < require_bench:
        failures.append(f"bench results: {len(configs)} distinct configs ({', '.join(sorted(configs)) or 'none'}), {require_bench} wanted (run the bench steps first)")
        print(f"  FAIL {failures[-1]}")
    elif benches:
        print(f"  ok   {len(configs)} bench configs: {', '.join(sorted(configs))}")

    # 2. the dual's counters
    for path in files:
        problem = report_problem(path)
        if problem:
            failures.append(problem)
            print(f"  FAIL {problem}")
    causes = causes_of(files)
    by_class = {}
    for (label, kind, why), n in causes.items():
        cls, idx = classify(label, why, rows)
        by_class.setdefault(cls, []).append((n, kind, label, why, idx))
    total = sum(causes.values())
    sums = {c: sum(n for n, *_ in by_class.get(c, [])) for c in ("census", "adapter", "gap", "unclassified", "silent")}
    print(f"zero-bail: {total} dual bails/gates in {len(causes)} causes over {len(files)} report(s): "
          + ", ".join(f"{c} {sums[c]}" for c in ("census", "adapter", "gap", "unclassified", "silent")))
    for cls in ("gap", "unclassified", "silent", "census", "adapter"):
        for n, kind, label, why, idx in sorted(by_class.get(cls, []), key=lambda r: -r[0]):
            print(f"{n:9}  {cls:12} {kind:4}  {label}  --  {why}")
    for cls in ("gap", "unclassified", "silent"):
        for n, kind, label, why, idx in by_class.get(cls, []):
            failures.append(f"{cls}: {n} x {label} [{kind}: {why}]")

    # 3. the evidence the class table cites
    if evidence:
        bad = verify_evidence(rows, root)
        for b in bad:
            print(f"  FAIL {b}")
        failures.extend(bad)
        if not bad:
            print(f"zero-bail: the evidence of the {len(rows)} class rows holds in the tree")

    if failures and enforce:
        print(f"zero-bail: FAILED ({len(failures)} problems: a whole game must play natively without a bail, and every dual cause must be classified)")
        return 1
    if failures:
        print(f"zero-bail: {len(failures)} problems (report mode: exit 0)")
    else:
        print("zero-bail: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
