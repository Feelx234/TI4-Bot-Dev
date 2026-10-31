#!/usr/bin/env python3
"""Coverage report of the typed core without line coverage (docs/todo/D4-core-tests.md 6.1-6.3, package D4-5).

  coverage.py [--root REPO] [--soak PATH ...] [--dual-report FILE ...] [--covers FILE ...] [--exempt FILE]
              [--out docs/ENGINE_CORE_COVERAGE.md] [--json OUT.json] [--save-census OUT.json]
              [--check [--quick]] [--stage-min 80] [--bail-share 20]
  coverage.py selftest

Inputs
  --soak PATH       the bail census of a soak/golden dual run: the soak's `state.json`, a slice `result.json`, a directory of
                    slices (every `result.json` below it), or a census snapshot written by --save-census. All carry
                    `windows: {label: [compared, bailed]}` ("label [bail: reason]" rows hold the reasons).
  --dual-report F   the text of a `dual_run` report (lines `    label: N compared, M bailed`).
  --covers F        `target/d4-covers.txt` written by the kit's covers!/covers_card!/covers_slot! and the stage trace of its
                    Rig: one line `kind<TAB>label<TAB>test`, kind = window | stage | card | slot. Default: `$D4_COVERS_FILE`,
                    `$CARGO_TARGET_DIR/d4-covers.txt`, `target/d4-covers.txt` (those that exist).
  --exempt F        `tools/engine_core/d4/covers_exempt.txt`: `label<TAB>reason<TAB>ISSUE-ID` per window that may stay uncovered.

What it reports (docs/ENGINE_CORE_COVERAGE.md)
  1 the window inventory (`impl Window for X`, its Stage variants, hook slots, card plays, WindowKind) joined with the covers
    file: covered windows, stage variants visited, per area;
  2 the bail census: silent windows (compared 0), windows bailing more than --bail-share percent, the lent arms (`d1::step
    [bail: arm]`), dual window labels in the source that the census never saw;
  3 the bail registry: every `*-bail-NN` row of docs/ENGINE_KNOWN_ISSUES.md against the tests of the workspace (a row that names
    no test is "documented, untested"; a named test that does not exist; a `bail_*` test that no row names is "undocumented");
  4 the worklist ordered by risk = lines of the window x (1 + bail share of its area) for what is not covered.
--check fails (exit 1) on: a window neither covered nor exempt (the stage variants of an exempt window do not count toward its area), an area below --stage-min percent of stage variants visited, a silent
window without a KNOWN_ISSUES id in the exempt file. --check --quick prints only the covers part (verify_package.sh) and exits 0.
"""
import argparse
import glob
import json
import os
import re
import sys
import tempfile

CORE = "crates/ti4-engine-core/src"
FACTIONS = "crates/ti4-engine-core-factions/src"
CRATE_PREFIX = {"ti4_engine_core_factions::": "factions::", "ti4_engine_core::": "", "crate::": ""}

# which D4-2 package owns which window area (docs/todo/D4-core-tests.md 8)
PACKAGES = [
    (r"^rules::(production|trade|diplomacy|strategy|objective|scoring|agenda)", "D4-2a"),
    (r"^rules::(tactical|explore|relic|leader)|^driver|^movement|^query::(reach|transit)", "D4-2b"),
    (r"^rules::(action_cards|reaction|abilities)|^kernel", "D4-2c"),
    (r"^rules::(combat|invasion)", "D4-2d (after int-5)"),
    (r"^factions", "D4-2e"),
]

# census label prefix -> window area (for the join of bail shares with the inventory)
LABEL_AREA = {
    "combat": "rules::combat", "invasion": "rules::invasion", "production": "rules::production",
    "ProductionWindow": "rules::production", "relics": "rules::relic_windows", "exploration": "rules::explore_windows",
    "leaders": "rules::leader_windows", "agenda_effects": "rules::agenda", "laws": "rules::agenda", "agenda": "rules::agenda",
    "strategy_cards": "rules::strategy_windows", "strategy": "rules::strategy_windows", "trade": "rules::trade_window",
    "objectives": "rules::objective_windows", "tactical": "rules::tactical", "action_cards": "rules::action_cards",
    "d1": "driver", "turn": "driver", "diplomacy": "rules::diplomacy_window",
}


def norm(path):
    """`ti4_engine_core::rules::x::Name<'a>` -> `rules::x::Name`; factions keep a `factions::` prefix."""
    path = re.sub(r"<[^<>]*>", "", path).strip()
    for k, v in CRATE_PREFIX.items():
        if path.startswith(k):
            path = v + path[len(k):]
    return path


def package_of(area):
    for rx, p in PACKAGES:
        if re.search(rx, area):
            return p
    return "?"


# ---- inventory ---------------------------------------------------------------------------------------------------

def src_files(root):
    for base in (CORE, FACTIONS):
        for r, ds, fs in os.walk(os.path.join(root, base)):
            ds[:] = sorted(d for d in ds if d != "generated")
            for f in sorted(fs):
                if f.endswith(".rs"):
                    yield os.path.join(r, f), base


def mod_path(rel_to_src, base):
    parts = rel_to_src[:-3].split(os.sep)
    if parts[-1] in ("mod", "lib"):
        parts = parts[:-1]
    prefix = ["factions"] if base == FACTIONS else []
    return "::".join(prefix + parts)


def balanced(text, start, open_ch="{", close_ch="}"):
    """Index just past the block that opens at text[start] == open_ch (comments and strings are not special-cased: good enough
    for the declarations scanned here)."""
    depth = 0
    for i in range(start, len(text)):
        c = text[i]
        if c == open_ch:
            depth += 1
        elif c == close_ch:
            depth -= 1
            if depth == 0:
                return i + 1
    return len(text)


def enum_variants(text, name):
    m = re.search(r"\benum\s+" + re.escape(name) + r"\b[^{;]*\{", text)
    if not m:
        return None
    body = text[m.end() - 1: balanced(text, m.end() - 1)][1:-1]
    body = re.sub(r"//[^\n]*", "", body)
    body = re.sub(r"/\*.*?\*/", "", body, flags=re.S)
    body = re.sub(r"#\[[^\]]*\]", "", body)
    out, depth, cur = [], 0, ""
    for c in body + ",":
        if c in "({[":
            depth += 1
        elif c in ")}]":
            depth -= 1
        if c == "," and depth == 0:
            mm = re.match(r"\s*([A-Za-z_]\w*)", cur)
            if mm:
                out.append(mm.group(1))
            cur = ""
        else:
            cur += c
    return out


WINDOW_RX = re.compile(r"^[ \t]*impl(?:<[^>{]*>)?[ \t]+(?:[\w]+::)*Window[ \t]+for[ \t]+(\w+)[^{]*\{", re.M)


def inventory(root):
    windows, enums_by_file, texts = {}, {}, {}
    for path, base in src_files(root):
        rel = os.path.relpath(path, os.path.join(root, base))
        text = open(path, encoding="utf-8", errors="replace").read()
        texts[path] = (text, mod_path(rel, base), base)
    all_text = {p: t for p, (t, _, _) in texts.items()}
    for path, (text, mp, base) in texts.items():
        test_blocks = [(t.start(), balanced(text, t.end() - 1)) for t in re.finditer(r"#\[cfg\(test\)\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{", text)]
        for m in WINDOW_RX.finditer(text):
            if any(a <= m.start() < b for a, b in test_blocks):
                continue  # a window of a test module is a fixture, not part of the core
            name = m.group(1)
            end = balanced(text, m.end() - 1)
            block = text[m.start():end]
            sm = re.search(r"type\s+Stage\s*=\s*(\w+)\s*;", block)
            variants = None
            if sm:
                variants = enum_variants(text, sm.group(1))
                if variants is None:  # the enum lives in another file of the crate
                    for other in all_text.values():
                        variants = enum_variants(other, sm.group(1))
                        if variants is not None:
                            break
            key = (mp + "::" if mp else "") + name
            windows[key] = {"name": name, "path": key, "file": os.path.relpath(path, root), "line": text.count("\n", 0, m.start()) + 1,
                            "lines": block.count("\n") + 1, "stage_type": sm.group(1) if sm else None, "stages": variants,
                            "area": area_of(key)}
    return {"windows": windows, "slots": slot_inventory(root), "cards": card_inventory(root), "kinds": kind_inventory(root),
            "dual_labels": dual_labels(root)}


def area_of(path):
    parts = path.split("::")
    if parts[0] == "factions":
        return "factions"
    if parts[0] == "rules" and len(parts) > 2:
        return "rules::" + parts[1]
    return "::".join(parts[:2]) if len(parts) > 2 else parts[0]


def slot_inventory(root):
    """`group.slot` of every slot declared in a `hook_group! { Group { kind name(..) -> T; } }` of kernel/hooks (group = the file
    stem, `general` for mod.rs), plus the `pub struct XHooks` fields written by hand, and `ability.field` of AbilityTable."""
    out = []
    d = os.path.join(root, CORE, "kernel", "hooks")
    for f in sorted(glob.glob(os.path.join(d, "*.rs"))):
        stem = os.path.basename(f)[:-3]
        group = "general" if stem == "mod" else stem
        text = open(f, encoding="utf-8", errors="replace").read()
        for m in re.finditer(r"hook_group!\s*\{", text):
            body = text[m.end() - 1: balanced(text, m.end() - 1)]
            for sm in re.finditer(r"^\s+(?:concat|sum|any|all|first|last|min|max|fold|\w+)\s+(\w+)\s*\(", body, re.M):
                out.append(f"{group}.{sm.group(1)}")
        for m in re.finditer(r"pub struct (\w*Hooks)\b[^{;]*\{", text):
            body = text[m.end() - 1: balanced(text, m.end() - 1)]
            for fm in re.finditer(r"^\s*pub\s+(\w+)\s*:", body, re.M):
                out.append(f"{group}.{fm.group(1)}")
    ab = os.path.join(root, CORE, "kernel", "ability.rs")
    if os.path.exists(ab):
        text = open(ab, encoding="utf-8", errors="replace").read()
        m = re.search(r"pub struct AbilityTable\b[^{;]*\{", text)
        if m:
            body = text[m.end() - 1: balanced(text, m.end() - 1)]
            out += [f"ability.{x}" for x in re.findall(r"^\s*pub\s+(\w+)\s*:", body, re.M)]
    return sorted(set(out))


def card_inventory(root):
    """Action cards with a registered CardPlay whose `covers` names the card: `c == ActionCard::X` closures and `ActionCard::X`
    inside a covers function. Cards a function predicate covers by data are not found: the covers file lists them by name."""
    out = set()
    d = os.path.join(root, FACTIONS, "cards")
    for f in sorted(glob.glob(os.path.join(d, "*.rs"))):
        text = open(f, encoding="utf-8", errors="replace").read()
        out.update(re.findall(r"ActionCard::(\w+)", text))
    return sorted(out)


def kind_inventory(root):
    p = os.path.join(root, CORE, "state", "pending.rs")
    if not os.path.exists(p):
        return []
    return enum_variants(open(p, encoding="utf-8", errors="replace").read(), "WindowKind") or []


def dual_labels(root):
    """The string literals `area::name` in the argument list of dual_window!/run_dual_window( call sites of ti4-engine."""
    out = set()
    for path in glob.glob(os.path.join(root, "crates", "ti4-engine", "src", "**", "*.rs"), recursive=True):
        text = open(path, encoding="utf-8", errors="replace").read()
        for m in re.finditer(r"(?:dual_window!|run_dual_window)\(", text):
            head = text[m.end(): m.end() + 400].split("\n")[:4]
            out.update(re.findall(r'"([A-Za-z_][\w]*::[\w]+)"', "\n".join(head)))
    return sorted(out)


# ---- census ------------------------------------------------------------------------------------------------------

DUAL_LINE = re.compile(r"^\s+(.+?): (\d+) compared, (\d+) bailed\s*$")


def add_windows(into, windows):
    for label, v in windows.items():
        t = into.setdefault(label, [0, 0])
        t[0] += v[0]
        t[1] += v[1]


def load_census(paths, reports):
    windows, meta = {}, {"inputs": []}
    for p in paths:
        files = [p]
        if os.path.isdir(p):
            files = sorted(glob.glob(os.path.join(p, "**", "result.json"), recursive=True))
        for f in files:
            doc = json.load(open(f))
            add_windows(windows, doc.get("windows", {}))
            meta["inputs"].append(os.path.basename(os.path.dirname(f)) + "/" + os.path.basename(f) if len(files) > 1 else os.path.basename(f))
            if "run" in doc:
                meta["run"] = doc["run"]
            if "totals" in doc:
                meta["totals"] = doc["totals"]
            if "census" in doc:
                meta["census"] = doc["census"]
            for k in doc.get("silent", {}):
                meta.setdefault("silent", set()).add(re.sub(r"^window ", "", k))
    for p in reports:
        got = {}
        for line in open(p):
            m = DUAL_LINE.match(line)
            if m:
                got[m.group(1)] = [int(m.group(2)), int(m.group(3))]
        add_windows(windows, got)
        meta["inputs"].append(os.path.basename(p))
    return windows, meta


def label_parts(label):
    m = re.match(r"^(.*?) \[bail: (.*)\]$", label)
    return (m.group(1), m.group(2)) if m else (label, None)


def census_view(windows, bail_share, extra_silent=()):
    labels = {}
    reasons, arms = [], {}
    for label, (c, b) in windows.items():
        base, why = label_parts(label)
        if why is None:
            labels[base] = (c, b)
        else:
            reasons.append((b, base, why))
            if base == "d1::step":
                arms[why] = arms.get(why, 0) + b
    # silent = compared 0 in the whole census, or in one slice (the soak's per-slice detection, `state.json` "silent")
    silent = sorted({l for l, (c, b) in labels.items() if c == 0 and b > 0} | set(extra_silent))
    high = sorted(((b / (c + b), l, c, b) for l, (c, b) in labels.items() if c + b > 0 and b / (c + b) * 100 > bail_share and (c or b)),
                  reverse=True)
    return {"labels": labels, "silent": silent, "high": high, "reasons": sorted(reasons, reverse=True), "arms": arms}


# ---- bail registry -----------------------------------------------------------------------------------------------

ROW_ID = re.compile(r"^\s*(?:\||-)\s*\**([A-Za-z0-9]+-bail-\d+)\**\b")


def bail_rows(root):
    p = os.path.join(root, "docs", "ENGINE_KNOWN_ISSUES.md")
    rows = {}
    if not os.path.exists(p):
        return rows
    for line in open(p, encoding="utf-8", errors="replace"):
        m = ROW_ID.match(line)
        if not m or m.group(1) in rows:
            continue
        text = line.strip()
        names = []
        for tm in re.finditer(r"\b[Tt]ests?\b[^`\n]{0,40}((?:`[^`]+`(?:[ ,]*(?:and|,)?[ ,]*)?)+)", text):
            names += re.findall(r"`([^`]+)`", tm.group(1))
        rows[m.group(1)] = {"text": text, "tests": [n for n in names if re.fullmatch(r"[A-Za-z_][\w*]*", n)],
                            "superseded": "superseded" in text.lower()}
    return rows


def workspace_tests(root):
    names = set()
    for path in glob.glob(os.path.join(root, "crates", "**", "*.rs"), recursive=True):
        text = open(path, encoding="utf-8", errors="replace").read()
        names.update(re.findall(r"\bfn\s+([A-Za-z_]\w*)\s*\(", text))
    return names


def census_row_arms(root):
    """The lent arms from the `D1b-census-01` row of KNOWN_ISSUES (20 golden games), for a census that predates D1b-8."""
    p = os.path.join(root, "docs", "ENGINE_KNOWN_ISSUES.md")
    if not os.path.exists(p):
        return {}
    for line in open(p, encoding="utf-8", errors="replace"):
        if line.lstrip("| ").startswith("D1b-census-01"):
            body = line.split(":", 1)[-1]
            return {a: int(n.replace(" ", "")) for a, n in re.findall(r"([a-z_][a-z_.]*) (\d{1,3}(?: \d{3})*)\b", body)}
    return {}


def bail_registry(root):
    rows = bail_rows(root)
    fns = workspace_tests(root)
    untested, missing = [], []
    named = set()
    for rid, r in rows.items():
        if r["superseded"]:
            continue
        if not r["tests"]:
            untested.append(rid)
            continue
        for t in r["tests"]:
            rx = re.compile("^" + re.escape(t).replace(r"\*", ".*") + "$")
            hit = [f for f in fns if rx.match(f)]
            named.update(hit)
            if not hit:
                missing.append((rid, t))
    undocumented = sorted(f for f in fns if f.startswith("bail_") and f not in named)
    return {"rows": len(rows), "untested": sorted(untested), "missing": sorted(missing), "undocumented": undocumented}


# ---- covers ------------------------------------------------------------------------------------------------------

def default_covers():
    out = []
    for p in (os.environ.get("D4_COVERS_FILE"),
              os.path.join(os.environ.get("CARGO_TARGET_DIR", "target"), "d4-covers.txt"), "target/d4-covers.txt"):
        if p and os.path.exists(p) and p not in out:
            out.append(p)
    return out


def load_covers(paths):
    got = {"window": {}, "stage": {}, "card": {}, "slot": {}}
    for p in paths:
        for line in open(p, encoding="utf-8", errors="replace"):
            cols = line.rstrip("\n").split("\t")
            if len(cols) < 2 or cols[0] not in got:
                continue
            got[cols[0]].setdefault(cols[1], set()).add(cols[2] if len(cols) > 2 else "?")
    return got


def stage_name(debug):
    return re.split(r"[\s({]", debug.strip(), maxsplit=1)[0]


def join_covers(inv, covers):
    wins = inv["windows"]
    by_name = {}
    for k, w in wins.items():
        by_name.setdefault(w["name"], []).append(k)

    def resolve(label):
        k = norm(label)
        if k in wins:
            return k
        last = k.split("::")[-1]
        if len(by_name.get(last, [])) == 1:
            return by_name[last][0]
        return None

    covered, visited, unknown = {}, {}, []
    for label, tests in covers["window"].items():
        k = resolve(label)
        if k:
            covered.setdefault(k, set()).update(tests)
        else:
            unknown.append(label)
    for label, tests in covers["stage"].items():
        wpath, _, st = label.rpartition("::")
        k = resolve(wpath)
        if k:
            visited.setdefault(k, set()).add(stage_name(st))
            covered.setdefault(k, set()).update(tests)  # a traced window ran under a test: covered
    return covered, visited, unknown


def read_exempt(path):
    out = {}
    if path and os.path.exists(path):
        for line in open(path):
            if line.strip() and not line.startswith("#"):
                cols = line.rstrip("\n").split("\t")
                out[cols[0]] = {"reason": cols[1] if len(cols) > 1 else "", "issue": cols[2] if len(cols) > 2 else ""}
    return out


# ---- report ------------------------------------------------------------------------------------------------------

def analyse(root, soak, dual_reports, covers_files, exempt_file, bail_share):
    inv = inventory(root)
    windows, meta = load_census(soak, dual_reports)
    cen = census_view(windows, bail_share, meta.get("silent", ()))
    covers = load_covers(covers_files)
    covered, visited, unknown = join_covers(inv, covers)
    exempt = read_exempt(exempt_file)
    areas = {}
    share_by_area = {}
    for label, (c, b) in cen["labels"].items():
        a = LABEL_AREA.get(label.split("::")[0], None)
        if a:
            t = share_by_area.setdefault(a, [0, 0])
            t[0] += c
            t[1] += b
    for k, w in inv["windows"].items():
        a = areas.setdefault(w["area"], {"windows": 0, "covered": 0, "exempt": 0, "stages": 0, "visited": 0, "lines": 0, "uncovered_lines": 0})
        a["windows"] += 1
        a["lines"] += w["lines"]
        if k in covered:
            a["covered"] += 1
        elif k in exempt:
            a["exempt"] += 1
        else:
            a["uncovered_lines"] += w["lines"]
        # An exempt window is not expected to run, so its stage variants are not part of the area's share (a covered window's are).
        if w["stages"] and (k in covered or k not in exempt):
            a["stages"] += len(w["stages"])
            a["visited"] += len(set(w["stages"]) & visited.get(k, set()))
    work = []
    for k, w in inv["windows"].items():
        if k in covered or k in exempt:
            continue
        c, b = share_by_area.get(w["area"], [0, 0])
        share = b / (c + b) if c + b else 0.0
        work.append((w["lines"] * (1 + share), k, w, share))
    work.sort(key=lambda r: (-r[0], r[1]))
    return {"inv": inv, "meta": meta, "census": cen, "covers": covers, "covered": covered, "visited": visited, "unknown": unknown,
            "exempt": exempt, "areas": areas, "work": work, "registry": bail_registry(root), "share_by_area": share_by_area,
            "windows": windows}


def pct(a, b):
    return "-" if not b else f"{100 * a // b}%"


def render(r, root):
    inv, cen, reg = r["inv"], r["census"], r["registry"]
    L = ["# Typed core: coverage report", "",
         "Generated by `tools/engine_core/d4/coverage.py` (package D4-5; design in `docs/todo/D4-core-tests.md` 6.1-6.3). Inputs: "
         + (", ".join(r["meta"]["inputs"]) or "none") + "; covers files: "
         + (", ".join(sorted(set(os.path.basename(p) for p in r["covers_files"]))) or "none (no test registered a `covers!` yet)") + ". "
         "Regenerate: `tools/engine_core/d4/coverage.py --soak <state.json|census snapshot> --out docs/ENGINE_CORE_COVERAGE.md`.", ""]
    run = r["meta"].get("run")
    tot = r["meta"].get("totals")
    if run or tot:
        L.append(f"Census: {tot['games']} games, {tot['slices']} slices, {tot['calls']:,} calls, {tot['compared']:,} comparisons, {tot['differences']} differences" if tot else "Census loaded.")
        if run:
            L[-1] += f" (soak started {run.get('started')}, commit `{run.get('commit')}`, rules_version {run.get('rules_version')})"
        L.append("")
    nw = len(inv["windows"])
    cov = sum(1 for k in inv["windows"] if k in r["covered"])
    ex = sum(1 for k in inv["windows"] if k not in r["covered"] and k in r["exempt"])
    st = sum(len(w["stages"]) for w in inv["windows"].values() if w["stages"])
    vis = sum(len(set(w["stages"]) & r["visited"].get(k, set())) for k, w in inv["windows"].items() if w["stages"])
    L += ["## 1. Summary", "",
          f"* windows (`impl Window for`): **{nw}**; covered by a test: **{cov}** ({pct(cov, nw)}); exempt: {ex}; uncovered: {nw - cov - ex}",
          f"* stage variants: {st} in {sum(1 for w in inv['windows'].values() if w['stages'])} windows; visited by a test: {vis} ({pct(vis, st)})",
          f"* hook slots: {len(inv['slots'])}; covered by `covers_slot!`: {sum(1 for s in inv['slots'] if s in r['covers']['slot'])}",
          f"* action cards named by a card play in the factions crate: {len(inv['cards'])}; covered by `covers_card!`: "
          f"{sum(1 for c in inv['cards'] if c in r['covers']['card'])} ({len(r['covers']['card'])} cards declared in all)",
          f"* `WindowKind` variants: {len(inv['kinds'])}",
          f"* silent windows in the census (compared 0, bailed > 0): **{len(cen['silent'])}**",
          f"* bail rows in KNOWN_ISSUES: {reg['rows']}; documented but untested: **{len(reg['untested'])}**; named test missing: {len(reg['missing'])}; `bail_*` tests no row names: **{len(reg['undocumented'])}**",
          ""]
    L += ["## 2. Per area", "", "| area | package | windows | covered | exempt | stage variants | visited | lines | bail share (census) |", "|---|---|---:|---:|---:|---:|---:|---:|---:|"]
    for a, v in sorted(r["areas"].items()):
        c, b = r["share_by_area"].get(a, [0, 0])
        L.append(f"| {a} | {package_of(a)} | {v['windows']} | {v['covered']} | {v['exempt']} | {v['stages']} | {v['visited']} ({pct(v['visited'], v['stages'])}) | {v['lines']} | {pct(b, c + b) if c + b else '-'} |")
    L.append("")
    L += ["## 3. Silent windows (the census compared nothing)", ""]
    L += [f"* `{l}`  ({cen['labels'][l][1]} bails)" + (f"  exempt: {r['exempt'][l]['issue']}" if l in r["exempt"] else "") for l in cen["silent"]] or ["none"]
    L += ["", f"## 4. Windows bailing more than the threshold", "", "| label | compared | bailed | share |", "|---|---:|---:|---:|"]
    for share, l, c, b in cen["high"][:40]:
        L.append(f"| `{l}` | {c:,} | {b:,} | {int(share * 100)}% |")
    L += ["", "## 5. Lent arms of the step loop (`d1::step [bail: arm]`)", "", "| arm | steps |", "|---|---:|"]
    for arm, n in sorted(cen["arms"].items(), key=lambda x: -x[1]):
        L.append(f"| {arm} | {n:,} |")
    if not cen["arms"]:
        L.append("| (the census has no `d1::step` rows: it predates D1b-8; the figures below are the D1b-census-01 row of KNOWN_ISSUES, 20 golden games) | |")
        for arm, n in sorted(census_row_arms(root).items(), key=lambda x: -x[1]):
            L.append(f"| {arm} | {n:,} |")
    seen = set(cen["labels"])
    never = [l for l in inv["dual_labels"] if l not in seen]
    L += ["", f"## 6. Dual window labels in the source that the census never saw ({len(never)} of {len(inv['dual_labels'])})", ""]
    L += [", ".join(f"`{l}`" for l in never) or "none", ""]
    L += ["## 7. Bail registry (docs/ENGINE_KNOWN_ISSUES.md)", "",
          "**Documented, untested** (the row names no test): " + (", ".join(reg["untested"]) or "none"), "",
          "**Named test does not exist**: " + (", ".join(f"{i} `{t}`" for i, t in reg["missing"]) or "none"), "",
          "**Undocumented** (`bail_*` test that no row names): " + (", ".join(f"`{t}`" for t in reg["undocumented"]) or "none"), ""]
    L += ["## 8. Worklist: uncovered windows by risk (lines x (1 + bail share of the area))", "", "| # | window | file:line | lines | stages | area share | package |", "|---:|---|---|---:|---:|---:|---|"]
    for i, (risk, k, w, share) in enumerate(r["work"][:60], 1):
        L.append(f"| {i} | `{k}` | {w['file']}:{w['line']} | {w['lines']} | {len(w['stages']) if w['stages'] else '?'} | {int(share * 100)}% | {package_of(w['area'])} |")
    if len(r["work"]) > 60:
        L.append(f"| | ... {len(r['work']) - 60} more | | | | | |")
    L += ["", "## 9. Covers declared that match no window", ""]
    L += [", ".join(f"`{u}`" for u in sorted(r["unknown"])) or "none", ""]
    return "\n".join(L) + "\n"


def check(r, stage_min, quick):
    inv = r["inv"]
    bad = []
    unc = [k for k in inv["windows"] if k not in r["covered"] and k not in r["exempt"]]
    if unc:
        bad.append(f"{len(unc)} window(s) neither covered by a test nor exempt (first: {', '.join(unc[:5])})")
    if quick:
        return bad
    for a, v in sorted(r["areas"].items()):
        if v["stages"] and 100 * v["visited"] < stage_min * v["stages"]:
            bad.append(f"area {a}: {pct(v['visited'], v['stages'])} of stage variants visited, below {stage_min}%")
    for l in r["census"]["silent"]:
        if not r["exempt"].get(l, {}).get("issue"):
            bad.append(f"silent window {l} without a KNOWN_ISSUES id in the exempt file")
    return bad


def save_census(r, path):
    doc = {"windows": r["windows"], "census": {"inputs": r["meta"]["inputs"]}}
    for k in ("run", "totals"):
        if k in r["meta"]:
            doc[k] = r["meta"][k]
    if r["meta"].get("silent"):
        doc["silent"] = {"window " + l: {} for l in sorted(r["meta"]["silent"])}
    json.dump(doc, open(path, "w"), indent=0, sort_keys=True)
    open(path, "a").write("\n")


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..")))
    ap.add_argument("--soak", action="append", default=[])
    ap.add_argument("--dual-report", action="append", default=[])
    ap.add_argument("--covers", action="append", default=None)
    ap.add_argument("--exempt", default=None)
    ap.add_argument("--out")
    ap.add_argument("--json")
    ap.add_argument("--save-census")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--quick", action="store_true")
    ap.add_argument("--stage-min", type=int, default=80)
    ap.add_argument("--bail-share", type=int, default=20)
    a = ap.parse_args(argv)
    exempt = a.exempt or os.path.join(a.root, "tools", "engine_core", "d4", "covers_exempt.txt")
    covers = a.covers if a.covers is not None else default_covers()
    r = analyse(a.root, a.soak, a.dual_report, covers, exempt, a.bail_share)
    r["covers_files"] = covers
    if a.out:
        open(a.out, "w").write(render(r, a.root))
    if a.json:
        json.dump({"windows": len(r["inv"]["windows"]), "covered": sorted(r["covered"]), "silent": r["census"]["silent"],
                   "registry": r["registry"], "arms": r["census"]["arms"]}, open(a.json, "w"), indent=1)
    if a.save_census:
        save_census(r, a.save_census)
    if a.check:
        bad = check(r, a.stage_min, a.quick)
        nw = len(r["inv"]["windows"])
        print(f"coverage: {len(r['covered'] & set(r['inv']['windows'])) if isinstance(r['covered'], set) else len([k for k in r['inv']['windows'] if k in r['covered']])}/{nw} windows covered, {len(r['census']['silent'])} silent, "
              f"{len(r['registry']['untested'])} bail rows untested, {len(r['registry']['undocumented'])} undocumented bail tests")
        for b in bad:
            print(("note: " if a.quick else "FAIL: ") + b)
        return 0 if (a.quick or not bad) else 1
    if not (a.out or a.json or a.save_census):
        sys.stdout.write(render(r, a.root))
    return 0


# ---- self test ---------------------------------------------------------------------------------------------------

def selftest():
    import shutil
    d = tempfile.mkdtemp()
    try:
        def w(rel, text):
            p = os.path.join(d, rel)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            open(p, "w").write(text)
        w(CORE + "/rules/invasion/stages.rs",
          "pub enum CStage { Open, Ask(u8), Done }\nimpl Window for CommitWindow {\n    type Stage = CStage;\n    fn x() {}\n}\n"
          "impl<'a> crate::kernel::stage::Window for Other<'a> {\n    type Stage = OStage;\n}\nenum OStage { A, B { x: u8 } }\n")
        w(CORE + "/rules/mod.rs", "// no windows\n")
        w(CORE + "/kernel/hooks/movement.rs", "hook_group! {\n    MovementHooks {\n        /// doc\n        any gravity_drive(s: &CoreState) -> bool;\n        sum other(s: &CoreState) -> i32;\n    }\n}\n")
        w(CORE + "/state/pending.rs", "pub enum WindowKind { Scoring, Tokens }\n")
        w(FACTIONS + "/cards/mod.rs", "t.add_card_play(CardPlay { covers: |c| c == ActionCard::Spy, effect: spy });\n")
        w("crates/ti4-engine/src/x.rs", 'fn f() { crate::dual_window!(\n  crate::dual::Area::X,\n  "combat::settle",\n  state, 1, 2); }\n')
        w("crates/ti4-engine/src/t.rs", "#[test]\nfn bail_documented_one() {}\n#[test]\nfn bail_undocumented_one() {}\nfn bail_tested_by_name() {}\n")
        w("docs/ENGINE_KNOWN_ISSUES.md",
          "| X-bail-01 | something. Test `bail_documented_one` |\n| X-bail-02 | nothing named here |\n"
          "- X-bail-03: Tests `bail_tested_by_name`, `bail_gone` end\n| X-bail-04 | superseded: ported |\n")
        inv = inventory(d)
        assert sorted(inv["windows"]) == ["rules::invasion::stages::CommitWindow", "rules::invasion::stages::Other"], inv["windows"].keys()
        assert inv["windows"]["rules::invasion::stages::CommitWindow"]["stages"] == ["Open", "Ask", "Done"]
        assert inv["windows"]["rules::invasion::stages::Other"]["stages"] == ["A", "B"]
        assert inv["slots"] == ["movement.gravity_drive", "movement.other"], inv["slots"]
        assert inv["cards"] == ["Spy"] and inv["kinds"] == ["Scoring", "Tokens"] and inv["dual_labels"] == ["combat::settle"]
        reg = bail_registry(d)
        assert reg["untested"] == ["X-bail-02"], reg
        assert reg["missing"] == [("X-bail-03", "bail_gone")], reg
        assert reg["undocumented"] == ["bail_undocumented_one"], reg  # planted: no row names it
        # census: planted silent window, parsing of the three input forms
        state = {"windows": {"combat::settle": [10, 0], "production::sling_relay": [0, 3], "production::sling_relay [bail: x]": [0, 3],
                             "d1::step [bail: tactical]": [0, 7], "d1::step": [20, 0], "laws::offer_discard": [1, 9]}}
        sp = os.path.join(d, "state.json")
        json.dump(state, open(sp, "w"))
        txt = os.path.join(d, "dual.txt")
        open(txt, "w").write("dual: 1 calls\n  windows: 2 call sites\n    combat::settle: 5 compared, 1 bailed\n    new::label: 0 compared, 2 bailed\n")
        windows, meta = load_census([sp], [txt])
        assert windows["combat::settle"] == [15, 1] and windows["new::label"] == [0, 2]
        cen = census_view(windows, 20)
        assert cen["silent"] == ["new::label", "production::sling_relay"], cen["silent"]
        assert cen["arms"] == {"tactical": 7} and "laws::offer_discard" in [h[1] for h in cen["high"]]
        # covers: normalisation of a type_name path, stage trace with a payload, unknown labels
        cf = os.path.join(d, "covers.txt")
        open(cf, "w").write("window\tti4_engine_core::rules::invasion::stages::CommitWindow\tt1\n"
                            "stage\tti4_engine_core::rules::invasion::stages::CommitWindow::Ask(3)\tt1\n"
                            "stage\tti4_engine_core::rules::invasion::stages::CommitWindow::Open\tt1\n"
                            "window\tNoSuchWindow\tt2\ncard\tSpy\tt3\nslot\tmovement.other\tt3\n")
        r = analyse(d, [sp], [txt], [cf], os.path.join(d, "none.txt"), 20)
        r["covers_files"] = [cf]
        assert list(r["covered"]) == ["rules::invasion::stages::CommitWindow"], r["covered"]
        assert r["visited"]["rules::invasion::stages::CommitWindow"] == {"Ask", "Open"}
        assert r["unknown"] == ["NoSuchWindow"]
        bad = check(r, 80, False)
        assert any("neither covered" in b for b in bad) and any("silent window new::label" in b for b in bad), bad
        assert any("stage variants visited" in b for b in bad), bad
        text = render(r, d)
        assert "Silent windows" in text and "`production::sling_relay`" in text and "bail_undocumented_one" in text
        # an exemption with an issue id removes the finding; covering the other window too gets the stage ratio up
        ex = os.path.join(d, "ex.txt")
        open(ex, "w").write("rules::invasion::stages::Other\tpending\tD4-found-01\nnew::label\tx\tX-1\nproduction::sling_relay\ty\tX-2\n")
        r2 = analyse(d, [sp], [txt], [cf], ex, 20)
        r2["covers_files"] = [cf]
        bad2 = check(r2, 0, False)
        assert bad2 == [], bad2
        print("selftest ok")
    finally:
        shutil.rmtree(d)


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "selftest":
        selftest()
    else:
        sys.exit(main(sys.argv[1:]))
