#!/usr/bin/env python3
"""Static inventory of every `#[test]` of the engine crates and its disposition for the D2-7 legacy move (docs/todo/D4-core-tests.md 7).

    python3 tools/engine_core/d4/test_inventory.py [--tsv FILE] [--summary FILE] [--check]

One row per `#[test]` attribute (so the row count of a crate equals `grep -c '#\\[test\\]'` over its files, which `--check` verifies):
crate, file, test, group, area, generic, dual, game.

Groups (section 7 of the todo):
  G  Game-driven depth tests (drive a `Game` through a scripted `Table`): KEEP, run unchanged against the flipped wrapper
  U  module-internal unit tests of the string engine: MOVE with the module to ti4-engine-legacy
  L  lockstep tests (assert the core half ran, `dual::`): STAY as oracle tests (cfg feature oracle)
  C  codec / kernel_sync tests of the wrapper: STAY in the wrapper
  S  string-only machinery (timing, choice, dual, fingerprint, registry, render, decision_*): with legacy / the bridge
  K  tests of the typed core crates, the factions crate and the bridge (not part of the legacy move)
The classification is by the text of the test body (an upper bound for G, a lower bound for U); `compile triage` after the move reclassifies.
"""
import argparse
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))

CRATES = [
    ("ti4-engine", ["crates/ti4-engine/src", "crates/ti4-engine/tests"]),
    ("ti4-engine-core", ["crates/ti4-engine-core/src", "crates/ti4-engine-core/tests"]),
    ("ti4-engine-core-factions", ["crates/ti4-engine-core-factions/src", "crates/ti4-engine-core-factions/tests"]),
    ("ti4-engine-core-bridge", ["crates/ti4-engine-core-bridge/src", "crates/ti4-engine-core-bridge/tests"]),
]

S_FILES = re.compile(r"(^|/)(timing|choice|dual|dual_window|fingerprint|registry|decision_[a-z_]*|render)(\.rs|/)")
C_FILES = re.compile(r"(^|/)(core_sync|kernel_sync)(\.rs|/)")
G_BODY = re.compile(r"\bGame::|\bGame\b\s*\{|choice::Table|\bTable::|\bScriptedTable\b|\bScripted\b|\.advance\(|\bplay_until\b|\brun_choices\b")
L_BODY = re.compile(r"\bcompared\b|assert_core_ran|core_ran\(|dual::serial|\bserial\(\)")
GENERIC = re.compile(r"fixtures::game\(|\"generic\"|\bgeneric\b")
DUAL = re.compile(r"dual::|\bcompared\b")
GAME = re.compile(r"\bGame::")

TEST_ATTR = re.compile(r"#\[test\]")
FN = re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)")


def rust_files(base):
    for dirpath, dirs, files in os.walk(os.path.join(ROOT, base)):
        dirs[:] = sorted(d for d in dirs if d not in ("target", "generated"))
        for f in sorted(files):
            if f.endswith(".rs"):
                yield os.path.join(dirpath, f)


def fn_body(text, start):
    """Text of the function whose `fn` keyword is at `start`: up to the matching closing brace (strings and comments are not parsed)."""
    open_at = text.find("{", start)
    if open_at < 0:
        return ""
    depth = 0
    for i in range(open_at, min(len(text), open_at + 60000)):
        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return text[open_at : i + 1]
    return text[open_at : open_at + 60000]


def area_of(rel, crate):
    parts = rel.split("/")
    if crate == "ti4-engine":
        if parts[2] == "tests":
            return "tests/" + os.path.splitext(parts[-1])[0]
        rest = parts[3:]
        if len(rest) > 1:
            return rest[0]
        return os.path.splitext(rest[0])[0]
    rest = parts[3:]
    return rest[0] if len(rest) > 1 else os.path.splitext(rest[0])[0]


def group_of(rel, crate, body):
    if crate != "ti4-engine":
        return "K"
    if "/tests/" in rel:
        return "G" if G_BODY.search(body) else "U"
    if C_FILES.search(rel):
        return "C"
    if S_FILES.search(rel):
        return "S"
    if L_BODY.search(body):
        return "L"
    if G_BODY.search(body):
        return "G"
    return "U"


def scan(only=None):
    rows = []
    counts = {}
    for crate, bases in CRATES:
        if only and crate not in only:
            continue
        grep_count = 0
        for base in bases:
            for path in rust_files(base):
                rel = os.path.relpath(path, ROOT).replace(os.sep, "/")
                text = open(path, encoding="utf-8", errors="replace").read()
                grep_count += len(TEST_ATTR.findall(text))
                for m in TEST_ATTR.finditer(text):
                    f = FN.search(text, m.end())
                    if not f:
                        continue
                    body = fn_body(text, f.start())
                    rows.append(
                        (
                            crate,
                            rel,
                            f.group(1),
                            group_of(rel, crate, body),
                            area_of(rel, crate),
                            int(bool(GENERIC.search(body))),
                            int(bool(DUAL.search(body))),
                            int(bool(GAME.search(body))),
                        )
                    )
        counts[crate] = grep_count
    return rows, counts


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tsv", default=os.path.join(ROOT, "tools/engine_core/d4/test_disposition.tsv"))
    ap.add_argument("--summary")
    ap.add_argument("--check", action="store_true", help="fail unless the row counts equal the #[test] counts and the committed TSV is current")
    ap.add_argument("--crate", action="append")
    args = ap.parse_args()
    rows, counts = scan(args.crate)
    rows.sort()
    header = "crate\tfile\ttest\tgroup\tarea\tgeneric\tdual\tgame"
    text = header + "\n" + "\n".join("\t".join(map(str, r)) for r in rows) + "\n"
    per = {}
    for r in rows:
        per[r[0]] = per.get(r[0], 0) + 1
    ok = all(per.get(c, 0) == n for c, n in counts.items())
    if args.check:
        if not ok:
            print("row counts differ from the #[test] counts:", per, counts)
            return 1
        try:
            current = open(args.tsv, encoding="utf-8").read()
        except OSError:
            current = ""
        if current != text:
            print("test_disposition.tsv is stale: run tools/engine_core/d4/test_inventory.py")
            return 1
        print("ok:", per)
        return 0
    with open(args.tsv, "w", encoding="utf-8") as f:
        f.write(text)
    groups = {}
    for r in rows:
        groups[(r[0], r[3])] = groups.get((r[0], r[3]), 0) + 1
    print("rows per crate:", per, "| #[test] per crate:", counts, "| equal:", ok)
    for k in sorted(groups):
        print(k, groups[k])
    print("generic users:", sum(r[5] for r in rows), "| dual users:", sum(r[6] for r in rows), "| Game users:", sum(r[7] for r in rows))
    if args.summary:
        open(args.summary, "w").write(repr((per, counts, groups)))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
