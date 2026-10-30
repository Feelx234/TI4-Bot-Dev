#!/usr/bin/env python3
"""The lent census (package W0 of docs/todo/lent-bodies.md): turn the report of a dual run into a table.

    tools/engine_core/lent_census.py REPORT.txt [--games N] [--md]

REPORT.txt is the text `dual_run` prints (`tools/engine_core/dual_sharded.sh ... > REPORT.txt`, best with
`TI4_NESTED_LOG=1` and the wide mask `all=dual`). It holds, per dual window, a line
`    <label>: <compared> compared, <bailed> bailed` and, for every reason the adapters counted,
`    <label> [bail: <why>]: 0 compared, <n> bailed`; a position the call site runs bare (a gate, no window opened) is
`    <label> [gate: <why>]: 0 compared, <n> bailed`.

Prints, per window: calls (compared + bailed), bailed, the share, then each reason with its runs. Reasons that start
with `gate:` are positions the call site runs bare (no window opened): they are not part of `bailed`.

Exit status 1 when a bail has no reason (the line `NO REASON RECORDED`, `dual::NO_REASON`) or when the reasons of a
window add up to less than its `bailed` (a bail somebody forgot to count): the census must show no silent bail.
"""
import re
import sys

NO_REASON = "NO REASON RECORDED"
LINE = re.compile(r"^\s+(\S+?)(?: \[(bail|gate): (.*)\])?: (\d+) compared, (\d+) bailed\s*$")


def parse(path):
    windows = {}
    reasons = {}
    for line in open(path, encoding="utf-8", errors="replace"):
        m = LINE.match(line.rstrip("\n"))
        if not m:
            continue
        label, kind, why, compared, bailed = m.group(1), m.group(2), m.group(3), int(m.group(4)), int(m.group(5))
        if why is None:
            windows[label] = (compared, bailed)
        elif kind == "gate":
            reasons.setdefault(label, []).append(("gate: " + why, bailed))
        else:
            reasons.setdefault(label, []).append((why, bailed))
    return windows, reasons


def main(argv):
    args = [a for a in argv if not a.startswith("--")]
    if not args:
        print(__doc__)
        return 2
    windows, reasons = parse(args[0])
    md = "--md" in argv
    silent = 0
    short = []
    rows = []
    for label in sorted(set(windows) | set(reasons)):
        compared, bailed = windows.get(label, (0, 0))
        rs = sorted(reasons.get(label, []), key=lambda r: -r[1])
        counted = sum(n for why, n in rs if not why.startswith("gate:"))
        silent += sum(n for why, n in rs if why == NO_REASON)
        if counted < bailed:
            short.append((label, bailed, counted))
        rows.append((label, compared, bailed, rs, counted))
    total_calls = sum(c + b for _, c, b, _, _ in rows)
    total_bail = sum(b for _, _, b, _, _ in rows)
    if md:
        print("| window | calls | bailed | share | reason | runs |")
        print("|---|---:|---:|---:|---|---:|")
    else:
        print(f"{'window':45} {'calls':>8} {'bailed':>7} {'share':>6}")
    for label, compared, bailed, rs, counted in rows:
        calls = compared + bailed
        share = f"{100 * bailed / calls:.1f}%" if calls else "-"
        if bailed == 0 and not rs:
            continue
        if md:
            print(f"| `{label}` | {calls} | {bailed} | {share} | | |")
            for why, n in rs:
                print(f"| | | | | {why} | {n} |")
        else:
            print(f"{label:45} {calls:8} {bailed:7} {share:>6}")
            for why, n in rs:
                print(f"    {n:7}  {why}")
    print(f"\nwindow calls {total_calls}, bailed {total_bail} ({100 * total_bail / max(total_calls, 1):.2f}%), "
          f"bails with no reason {silent}")
    for label, bailed, counted in short:
        print(f"UNCOUNTED: {label}: {bailed} bailed, reasons add up to {counted}")
    return 1 if (silent or short) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
