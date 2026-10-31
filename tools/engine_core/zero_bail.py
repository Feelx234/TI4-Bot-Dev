#!/usr/bin/env python3
"""The zero-bail acceptance check (user rule: a whole game plays in the new engine without a bail, on the agreed content: everything
except Thunder's Edge and the dropped factions/features).

    tools/engine_core/zero_bail.py [--enforce] REPORT.txt..

REPORT.txt are the texts `dual_run` / `dual_sharded.sh` print for runs with every area on Dual (the golden 20, the lent set, ...); the
gate binaries run with `unsupported::Policy::Panic` and strict unmapped mode, so a dropped faction or a Thunder's Edge card would already
have panicked. Counts, over all the reports: every counted bail (`<label> [bail: <why>]`) and every gate (`<label> [gate: <why>]`: a
position the call site runs bare), by cause, biggest first; the `d1::pending` rows (a step with nothing open to compare) are not lends and
are left out. Without --enforce the list is the report and the exit status is 0 (the starting list of the lanes still porting); with
--enforce (end_of_d1.sh, ZERO_BAIL=enforce) any cause left is a failure, exit 1.
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lent_census import parse  # noqa: E402


def main(argv):
    enforce = "--enforce" in argv
    files = [a for a in argv if not a.startswith("--")]
    if not files:
        print(__doc__)
        return 2
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
    total = sum(causes.values())
    print(f"zero-bail: {total} bails/gates in {len(causes)} causes over {len(files)} report(s)")
    for (label, kind, text), n in sorted(causes.items(), key=lambda kv: -kv[1]):
        print(f"{n:9}  {kind:4}  {label}  --  {text}")
    if total and enforce:
        print("zero-bail: FAILED (a whole game must play without a bail)")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
