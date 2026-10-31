#!/usr/bin/env python3
"""The lent-site inventory (package D2-5r R3, docs/todo/D2-remaining.md 3.1): every place of the string engine where the core
may decline and the string code runs, found in the source text. The probe of a dual run (`TI4_BAIL_PROBE`) sees only the bails of
`dual_window!` windows; this tool lists ALL sites, so the flip has a work list and `bail_table --check` has something to check against.

    python3 -I tools/engine_core/lent_sites.py [SRC_DIR] [--tsv FILE] [--census REPORT.txt] [--games N] [--summary]
    python3 -I tools/engine_core/lent_sites.py --check [--tsv FILE]     exit 1 when the committed TSV is not what the source gives
    python3 -I tools/engine_core/lent_sites.py --selftest

SRC_DIR defaults to `crates/ti4-engine/src`. The TSV (default `tools/engine_core/lent_sites.tsv`, committed) has one row per
site, test code left out (counted in the summary):

    site         file:line of the macro or call
    macro        dual_window | dual_typed | typed_ability | dual | dual_write | dual_hook | cb_dual | cc_dual | dual_query | run_dual_ask |
                 note_gate | run_dual_window | run_dual_typed | run_dual | run_dual_query | run_dual_hook
                 (dual_write = `dual::write(..)` and the per-file `dual_write(..)` wrappers; typed_ability = a `dual_typed!` through
                 the `typed_ability!` wrapper macro of faction_abilities.rs; cb_dual and cc_dual = `dual_hook!` wrappers of core_factions;
                 the run_dual_* names = direct calls of the functions the macros expand to; dual.rs itself is the machinery, not a site)
    kind         what the site is for the flip:
                   window    a `dual_window!` (or a direct `run_dual_window(..)`): the core opens a window; on `None` the string window runs (a bail)
                   fallback  `dual_typed!`, `typed_ability!`, `dual!`, `dual_write`: on `None` (or always, for `dual!` on a position the
                             core half does not carry) the string code answers
                   hook      `dual_hook!`: one string seam answer next to the core's (a string slot)
                   question  `run_dual_ask`: the question half of a stage window
                   gate      `note_gate`: a position the call site runs bare, no window opened (the `[gate: ..]` rows)
                   query     `dual_query!`: a read compared by value; no `None`, both halves exist (listed for completeness)
    area         the `Area` of the call (blank when it is not a literal)
    label        the label (the hook slot for dual_hook); `<expr: ...>` when it is not a string literal, with dynamic=y
    dynamic      n for a literal; const (a path to a `&str` const, resolved), alt (`if p { "a" } else { "b" }`: label is `a|b`),
                 format (a format! template, label is the template) or y (anything else: a variable or call; read these by hand, trap T-E)
    fn           the enclosing function, `Type::name` inside an impl
    receiver     free | self (a method on a window struct or on Game: no free function to wrap) | closure-only for a site outside any fn
    string_body  y when the string closure calls a `*_string` function (a body that is already separate, so a wrapper is a one-liner)
    string_call  the first function the string closure calls
    macro_line   the line the call starts at (site is the same file)
    calls, bailed, gate_hits   (with --census) the compared+bailed calls, the bails, and the `[gate: ..]` runs of this label in the
                 report of a dual run; `calls` is per the whole run: pass --games N to print per-game numbers in --summary

Rows are sorted by file, line. The scan is text based (a small Rust lexer that blanks comments and string contents; no name
resolution). Sites inside `macro_rules!` bodies (label `$label`) and in `#[cfg(test)]`/`#[test]` items are not sites.
"""
import os
import re
import sys
import unittest
from collections import Counter, defaultdict

MACROS = [
    # (name, regex of the call, label arg, string arg, area arg, kind)
    ("dual_window", r"\bdual_window!\s*\(", 1, 3, 0, "window"),
    ("dual_typed", r"\bdual_typed!\s*\(", 1, 3, 0, "fallback"),
    ("typed_ability", r"\btyped_ability!\s*\(", 0, 5, None, "fallback"),
    ("dual", r"\bdual!\s*\(", 1, 3, 0, "fallback"),
    ("dual_write", r"(?<!fn )\bdual::write\s*\(", 2, 3, 0, "fallback"),
    ("dual_write", r"(?<!fn )(?<![\w:])dual_write\s*\(|(?<!fn )\b(?:exploration|objectives|leaders)::dual_write\s*\(", 1, 2, None, "fallback"),
    ("dual_hook", r"\bdual_hook!\s*\(", 1, 2, None, "hook"),
    ("dual_query", r"\bdual_query!\s*\(", 1, 2, 0, "query"),
    ("run_dual_ask", r"\brun_dual_ask\s*\(", 1, None, 0, "question"),
    ("note_gate", r"(?<!fn )\bnote_gate\s*\(", 1, None, 0, "gate"),
    # the wrapper macros of the hook sites and the direct calls of the run_dual_* functions the macros expand to
    ("cb_dual", r"\bcb_dual!\s*\(", 1, 2, None, "hook"),
    ("cc_dual", r"\bcc_dual!\s*\(", 1, 2, None, "hook"),
    ("run_dual_window", r"(?<!fn )\brun_dual_window\s*\(", 1, 3, 0, "window"),
    ("run_dual_typed", r"(?<!fn )\brun_dual_typed\s*\(", 1, 3, 0, "fallback"),
    ("run_dual", r"(?<!fn )\brun_dual\s*\(", 1, 3, 0, "fallback"),
    ("run_dual_query", r"(?<!fn )\brun_dual_query\s*\(", 1, 2, 0, "query"),
    ("run_dual_hook", r"(?<!fn )\brun_dual_hook\s*\(", 1, 2, None, "hook"),
]
# the machinery itself: its macros and run_dual_* functions call each other with variable labels
INFRA_FILES = {"dual.rs"}
TRIVIAL_CALLS = {"Run::new", "dual::Run::new", "crate::dual::Run::new", "Some", "None", "Ok", "Err", "Box::new", "vec", "format", "matches", "Vec::new", "String::new", "if", "match", "while", "for", "assert"}
# the per-file wrappers fix the area of a `dual_write` call
WRAPPER_AREA_RE = re.compile(r"fn\s+dual_write\b.*?crate::dual::write\(\s*crate::dual::Area::(\w+)", re.S)


# --- the lexer: comments and string contents blanked, same offsets -----------------------------------------

def mask(text):
    out = list(text)
    n = len(text)
    i = 0

    def blank(a, b):
        for k in range(a, b):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        c = text[i]
        if text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            blank(i, j)
            i = j
        elif text.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth += 1
                    j += 2
                elif text.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            blank(i, j)
            i = j
        elif c == "r" and re.match(r"r#*\"", text[i:i + 12]) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            m = re.match(r"r(#*)\"", text[i:])
            close = "\"" + m.group(1)
            j = text.find(close, i + len(m.group(0)))
            j = n if j < 0 else j + len(close)
            blank(i + len(m.group(0)), j - len(close))
            i = j
        elif c == "\"":
            j = i + 1
            while j < n and text[j] != "\"":
                j += 2 if text[j] == "\\" else 1
            blank(i + 1, j)
            i = j + 1
        elif c == "'":
            # a char literal ('x', '\n', '\u{..}') or a lifetime ('a)
            m = re.match(r"'(\\.[^']*|[^\\'])'", text[i:i + 14])
            if m:
                blank(i + 1, i + len(m.group(0)) - 1)
                i += len(m.group(0))
            else:
                i += 1
        else:
            i += 1
    return "".join(out)


# --- items: functions, impls, tests ---------------------------------------------------------------------------

class Frame:
    __slots__ = ("kind", "name", "has_self", "impl_ty", "test", "params")

    def __init__(self, kind, name, has_self, impl_ty, test, params=""):
        self.kind, self.name, self.has_self, self.impl_ty, self.test, self.params = kind, name, has_self, impl_ty, test, params


def balanced_end(m, start, open_c, close_c):
    depth = 0
    for k in range(start, len(m)):
        if m[k] == open_c:
            depth += 1
        elif m[k] == close_c:
            depth -= 1
            if depth == 0:
                return k
    return len(m) - 1


def impl_type(header):
    h = header.strip()
    h = h[len("impl"):].strip()
    if h.startswith("<"):
        h = h[balanced_end(h, 0, "<", ">") + 1:].strip()
    if re.search(r"\bfor\b", h):
        h = re.split(r"\bfor\b", h, maxsplit=1)[1]
    h = re.split(r"\bwhere\b", h)[0]
    h = re.sub(r"<.*", "", h, flags=re.S)
    return h.strip().split("::")[-1]


def scopes(m):
    """For every offset of the masked text, the stack of frames: returns a function offset -> (fn Frame or None, impl type, test)."""
    pending = {}   # offset of `{` -> Frame
    test_attr = False
    skip_until = 0
    for mt in re.finditer(r"#\s*\[\s*(?:cfg\s*\(\s*test\s*\)|test)\s*\]|\b(fn|impl|mod|trait)\b", m):
        s = mt.group(0)
        if s.startswith("#"):
            test_attr = True
            continue
        kw = mt.group(1)
        if mt.start() < skip_until:
            continue   # `impl Fn(..)` or `fn(..)` inside the signature of the item before
        # the signature runs to the first `{` or `;` outside parens
        k, depth = mt.end(), 0
        while k < len(m):
            if m[k] in "([":
                depth += 1
            elif m[k] in ")]":
                depth -= 1
            elif depth == 0 and m[k] in "{;":
                break
            k += 1
        if k >= len(m) or m[k] == ";":
            test_attr = False
            continue
        sig = m[mt.end():k]
        skip_until = k
        if kw == "fn":
            name = re.match(r"\s*(\w+)", sig)
            params = ""
            p = sig.find("(")
            if p >= 0:
                params = sig[p:balanced_end(sig, p, "(", ")") + 1]
            pending[k] = Frame("fn", name.group(1) if name else "?", bool(re.search(r"\bself\b", params)), None, test_attr, params)
        elif kw == "impl" or kw == "trait":
            ty = impl_type("impl" + sig) if kw == "impl" else (re.match(r"\s*(\w+)", sig).group(1) if re.match(r"\s*(\w+)", sig) else "?")
            pending[k] = Frame(kw, ty, False, ty, test_attr)
        else:
            pending[k] = Frame("mod", re.match(r"\s*(\w+)", sig).group(1) if re.match(r"\s*(\w+)", sig) else "?", False, None, test_attr)
        test_attr = False
    # sweep braces, recording intervals of the frames
    intervals = []   # (start, end, Frame, parent_test)
    stack = []
    for mt in re.finditer(r"[{}]", m):
        i = mt.start()
        if mt.group(0) == "{":
            stack.append((i, pending.get(i)))
        elif stack:
            start, frame = stack.pop()
            if frame is not None:
                intervals.append((start, i, frame))
    intervals.sort(key=lambda t: (t[0], -t[1]))

    def at(off):
        inside = [iv for iv in intervals if iv[0] <= off <= iv[1]]
        fn = next((f for _, _, f in reversed(inside) if f.kind == "fn"), None)
        impl = next((f.impl_ty for _, _, f in reversed(inside) if f.kind == "impl"), None)
        test = any(f.test for _, _, f in inside)
        return fn, impl, test

    return at


# --- the call scanner ---------------------------------------------------------------------------------------------

def split_args(m, open_at):
    """The top-level arguments of the call whose `(` is at open_at: list of (start, end) and the offset of the closing `)`."""
    args, depth, start = [], 0, open_at + 1
    i = open_at
    n = len(m)
    while i < n:
        c = m[i]
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
            if depth == 0:
                args.append((start, i))
                return args, i
        elif c == "," and depth == 1:
            args.append((start, i))
            start = i + 1
        elif c == "|" and depth == 1:
            before = m[start:i].strip()
            if before in ("", "move") and not m.startswith("||", i):
                # closure parameters: skip to the closing `|` so their commas do not split the argument
                j = m.find("|", i + 1)
                if j > 0:
                    i = j
        i += 1
    return args, n - 1


def line_of(text, off):
    return text.count("\n", 0, off) + 1


def first_call(arg_masked):
    for mt in re.finditer(r"([A-Za-z_][\w:.]*)\s*(?:::<[^>]*>)?\s*\(", arg_masked):
        name = mt.group(1)
        if name in TRIVIAL_CALLS or name.endswith("!"):
            continue
        return name
    return ""


CONST_RE = re.compile(r"\bconst\s+(\w+)\s*:\s*&(?:'static\s+)?str\s*=\s*\"((?:[^\"\\]|\\.)*)\"\s*;")


def find_consts(src):
    """name -> [(file relative to src, value)] of every `const NAME: &str = "..."` (a label spelled as a path resolves through this)."""
    out = defaultdict(list)
    for root, _, files in os.walk(src):
        for f in files:
            if f.endswith(".rs"):
                rel = os.path.relpath(os.path.join(root, f), src).replace(os.sep, "/")
                with open(os.path.join(root, f), encoding="utf-8") as fh:
                    for mt in CONST_RE.finditer(fh.read()):
                        out[mt.group(1)].append((rel, mt.group(2)))
    return out


def module_of(rel):
    """`a/b.rs` -> `b`; `a/mod.rs` -> `a`."""
    stem = rel[:-3].split("/")
    return stem[-2] if stem[-1] == "mod" and len(stem) > 1 else stem[-1]


def resolve_label(src_text, consts, rel=""):
    """(label, how): a literal, a path to a `&str` const, an `if .. { "a" } else { "b" }` of literals ('a|b'), a format! template."""
    lit = re.fullmatch(r'"((?:[^"\\]|\\.)*)"', src_text, re.S)
    if lit:
        return lit.group(1), "n"
    path = re.fullmatch(r"&?(?:crate::)?((?:\w+::)*)([A-Z][A-Z0-9_]*)", src_text)
    if path:
        # `a::b::NAME` is the const in module `b`; a bare NAME is the one of this file (or the only one there is)
        qualifier = [q for q in path.group(1).split("::") if q]
        found = [(f, v) for f, v in consts.get(path.group(2), [])
                 if (module_of(f) == qualifier[-1] if qualifier else f == rel)]
        if not found and not qualifier and len(consts.get(path.group(2), [])) == 1:
            found = consts[path.group(2)]
        if len(found) == 1:
            return found[0][1], "const"
    alts = re.findall(r'"((?:[^"\\]|\\.)*)"', src_text)
    if alts and re.fullmatch(r'\s*if\b[^"]*(?:\{\s*"[^"]*"\s*\}\s*else\s*)+\{\s*"[^"]*"\s*\}\s*', src_text, re.S):
        return "|".join(alts), "alt"
    fmt = re.fullmatch(r'&?format!\(\s*"((?:[^"\\]|\\.)*)"\s*(?:,.*)?\)', src_text, re.S)
    if fmt:
        return fmt.group(1), "format"
    return "<expr: " + re.sub(r"\s+", " ", src_text)[:60] + ">", "y"


def scan_file(path, text, rel, consts=None):
    if rel in INFRA_FILES:
        return []
    m = mask(text)
    at = scopes(m)
    wrapper_area = None
    wm = WRAPPER_AREA_RE.search(m)
    if wm:
        wrapper_area = wm.group(1)
    rows = []
    for name, rx, label_i, string_i, area_i, kind in MACROS:
        for mt in re.finditer(rx, m):
            open_at = mt.end() - 1
            args, close = split_args(m, open_at)
            argtext = lambda i: (text[args[i][0]:args[i][1]].strip() if i is not None and i < len(args) else "")
            argmask = lambda i: (m[args[i][0]:args[i][1]] if i is not None and i < len(args) else "")
            label_src = argtext(label_i)
            # a macro template: its labels are `$label`
            if "$" in label_src or "$" in argtext(0):
                continue
            fn, impl, test = at(mt.start())
            label, dynamic = resolve_label(label_src, consts or {}, rel)
            if dynamic == "y" and fn is not None and re.fullmatch(r"&?\w+", label_src) and re.search(r"\b%s\s*:" % re.escape(label_src.lstrip("&")), fn.params):
                dynamic = "param"   # a helper that takes the label: its callers carry the labels
            area = ""
            if area_i is not None:
                am = re.search(r"Area::(\w+)", argtext(area_i))
                area = am.group(1) if am else ""
            elif kind == "hook":
                area = "Factions"
            elif name == "dual_write" and wrapper_area and not mt.group(0).startswith("dual::") and "::" not in mt.group(0):
                area = wrapper_area
            elif name == "dual_write":
                q = re.match(r"(\w+)::dual_write", mt.group(0))
                area = {"exploration": "ExplorationRelics", "objectives": "Objectives", "leaders": "LeadersAbilities"}.get(q.group(1), "") if q else ""
            elif name == "typed_ability":
                area = "LeadersAbilities"
            sm = argmask(string_i) if string_i is not None else ""
            call = first_call(sm)
            has_string = "y" if re.search(r"\b\w*_string\w*\s*(?:::<[^>]*>)?\s*\(", sm) else "n"
            if string_i is None:
                has_string, call = "-", ""
            if name == "dual_write" and fn and fn.name == "dual_write":
                continue   # the body of a per-file `dual_write` wrapper: plumbing, its callers are the sites
            fname = ("%s::%s" % (impl, fn.name) if impl else fn.name) if fn else "<outside any fn>"
            receiver = ("self" if fn.has_self else "free") if fn else "closure-only"
            rows.append({
                "path": rel, "line": line_of(text, mt.start()), "macro": name, "kind": kind, "area": area, "label": label,
                "dynamic": dynamic, "fn": fname, "receiver": receiver, "string_body": has_string, "string_call": call, "test": test,
            })
    rows.sort(key=lambda r: (r["path"], r["line"], r["macro"]))
    return rows


def scan(src):
    rows, tests = [], []
    consts = find_consts(src)
    for root, _, files in os.walk(src):
        for f in sorted(files):
            if not f.endswith(".rs"):
                continue
            path = os.path.join(root, f)
            rel = os.path.relpath(path, src).replace(os.sep, "/")
            with open(path, encoding="utf-8") as fh:
                text = fh.read()
            # whole test files (`*_tests.rs`, `tests.rs`) are test code
            is_test_file = f.endswith("_tests.rs") or f == "tests.rs" or "/tests/" in "/" + rel
            for r in scan_file(path, text, rel, consts):
                (tests if (r["test"] or is_test_file) else rows).append(r)
    rows.sort(key=lambda r: (r["path"], r["line"], r["macro"]))
    return rows, tests


# --- census join ---------------------------------------------------------------------------------------------------

LINE = re.compile(r"^\s+(\S+?)(?: \[(bail|gate): (.*)\])?: (\d+) compared, (\d+) bailed\s*$")


def read_census(path):
    windows, gates = {}, defaultdict(int)
    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            m = LINE.match(line.rstrip("\n"))
            if not m:
                continue
            label, kind, _, compared, bailed = m.group(1), m.group(2), m.group(3), int(m.group(4)), int(m.group(5))
            if kind is None:
                windows[label] = (compared, bailed)
            elif kind == "gate":
                gates[label] += bailed
    return windows, gates


COLUMNS = ["site", "macro", "kind", "area", "label", "dynamic", "fn", "receiver", "string_body", "string_call"]


def render(rows, census=None):
    cols = COLUMNS + (["calls", "bailed", "gate_hits"] if census else [])
    out = ["\t".join(cols)]
    for r in rows:
        cells = ["%s:%d" % (r["path"], r["line"]), r["macro"], r["kind"], r["area"], r["label"], r["dynamic"], r["fn"], r["receiver"], r["string_body"], r["string_call"]]
        if census:
            windows, gates = census
            c, b = windows.get(r["label"], (0, 0))
            cells += [str(c + b), str(b), str(gates.get(r["label"], 0))]
        out.append("\t".join(cells))
    return "\n".join(out) + "\n"


def summary(rows, tests, census=None, games=None):
    lines = []
    by_macro = Counter(r["macro"] for r in rows)
    lines.append("sites outside test code: %d (test code left out: %d)" % (len(rows), len(tests)))
    for name, _, _, _, _, kind in MACROS:
        pass
    seen = []
    for name, *_ in MACROS:
        if name in seen:
            continue
        seen.append(name)
        t = sum(1 for r in tests if r["macro"] == name)
        lines.append("  %-14s %4d   (+%d in tests)" % (name, by_macro[name], t))
    windows = [r for r in rows if r["macro"] == "dual_window"]
    lines.append("dual_window sites: %d on `self` methods: %d, with a *_string body: %d, label to read by hand: %d" % (
        len(windows), sum(1 for r in windows if r["receiver"] == "self"), sum(1 for r in windows if r["string_body"] == "y"),
        sum(1 for r in windows if r["dynamic"] == "y")))
    labels = Counter(r["label"] for r in rows if r["dynamic"] in ("n", "const") and r["kind"] != "gate")
    dups = sorted(l for l, n in labels.items() if n > 1)
    lines.append("labels used by more than one site: %d" % len(dups))
    if census:
        wins, gates = census
        known = {r["label"] for r in rows}
        lines.append("census labels with no site in the source (stale label or a dynamic one): %s" % sorted((set(wins) | set(gates)) - known))
        per = (lambda n: "%.1f" % (n / games)) if games else str
        tot = sum(c + b for c, b in wins.values())
        lines.append("window calls in the census: %d (%s per game); gate rows: %d" % (tot, per(tot), sum(gates.values())))
    return "\n".join(lines) + "\n"


def main(argv):
    args = list(argv)
    if "--selftest" in args:
        return 0 if unittest.main(argv=[sys.argv[0]], exit=False, verbosity=1).result.wasSuccessful() else 1

    def take(flag):
        if flag in args:
            i = args.index(flag)
            v = args[i + 1]
            del args[i:i + 2]
            return v
        return None

    tsv = take("--tsv") or "tools/engine_core/lent_sites.tsv"
    census_path = take("--census")
    games = take("--games")
    check = "--check" in args
    want_summary = "--summary" in args
    args = [a for a in args if not a.startswith("--")]
    src = args[0] if args else "crates/ti4-engine/src"
    rows, tests = scan(src)
    census = read_census(census_path) if census_path else None
    text = render(rows, census)
    if check:
        try:
            have = open(tsv, encoding="utf-8").read()
        except OSError as e:
            print("lent_sites: %s" % e)
            return 1
        # the committed file may carry census columns; compare the site columns only
        def sites(t):
            return [tuple(l.split("\t")[:len(COLUMNS)]) for l in t.splitlines()]
        if sites(have) != sites(text):
            old, new = set(sites(have)), set(sites(text))
            print("lent_sites: %s is out of date: %d sites in the file, %d in the source" % (tsv, len(old) - 1, len(new) - 1))
            for r in sorted(new - old)[:15]:
                print("  new     %s" % "\t".join(r[:5]))
            for r in sorted(old - new)[:15]:
                print("  removed %s" % "\t".join(r[:5]))
            print("  regenerate: python3 -I tools/engine_core/lent_sites.py --tsv %s" % tsv)
            return 1
        print("lent_sites: %s matches the source (%d sites)" % (tsv, len(rows)))
        return 0
    if want_summary:
        sys.stdout.write(summary(rows, tests, census, int(games) if games else None))
        return 0
    with open(tsv, "w", encoding="utf-8") as fh:
        fh.write(text)
    sys.stdout.write(summary(rows, tests, census, int(games) if games else None))
    print("wrote %s" % tsv)
    return 0


# --- self test ------------------------------------------------------------------------------------------------------

SAMPLE = r'''
use crate::dual::Area;

pub fn free_fn(state: &mut GameState) -> u32 {
    // dual_window!(Area::Combat, "commented::out", s, a, b)
    let s = "dual_typed!(Area::X, \"in::string\", a, b)";
    crate::dual_window!(Area::Invasion, "invasion::commit_one", state, |s| Run::new(commit_one_string(s, 1)), |s, script| None)
}

impl Window {
    fn method(&mut self, state: &mut GameState) {
        crate::dual_window!(
            Area::Driver,
            "combat::roll_round",
            state,
            |s: &mut GameState| Run::new(self.roll(s)),
            |s, script| None
        );
        crate::dual::note_gate(crate::dual::Area::Driver, "combat::settle", why);
        let label = format!("x::{}", 3);
        crate::dual_window!(Area::Driver, &label, state, |s| Run::new(1), |s, _| None);
    }
}

pub fn typed(state: &GameState) -> Vec<u8> {
    crate::dual_typed!(Area::Objectives, "objectives::list", typed_expr(), list_string(state))
}

pub(crate) fn dual_write<R>(state: &mut GameState, label: &str, string: impl Fn(&mut GameState) -> R, core: impl Fn(&mut GameState) -> Option<R>) -> R {
    crate::dual::write(crate::dual::Area::ExplorationRelics, state, label, string, core)
}

pub fn writer(state: &mut GameState) {
    dual_write(state, "relics::gain", |s| s.x(), |s| Some(1));
    crate::dual::write(Area::Technology, state, "technology::exhaust", |s| s.y(), |s| None);
    crate::run_dual_ask(Area::Invasion, "invasion::fight_round_ask", Some(&c), || None);
    crate::dual_hook!(HookGroup::Seam, "hit_shift", string(), core());
}

macro_rules! typed_ability {
    ($label:literal, $state:expr) => {{ crate::dual_typed!(Area::X, $label, a, b) }};
}

pub fn ability() {
    typed_ability!("faction_abilities::of_player", state, content, SRC, |v| v.abilities_of(p), of_player_string(a))
}

#[cfg(test)]
mod tests {
    #[test]
    fn t() {
        crate::dual_window!(Area::Driver, "tests::only", s, a, b);
    }
}
'''


def find_consts_text(text, rel="x.rs"):
    out = defaultdict(list)
    for mt in CONST_RE.finditer(text):
        out[mt.group(1)].append((rel, mt.group(2)))
    out["LABEL"].append(("cannon_dual.rs", "cannon::head"))
    out["LABEL"].append(("other.rs", "other::label"))
    return out


class SelfTest(unittest.TestCase):
    def rows(self):
        return scan_file("x.rs", SAMPLE, "x.rs")

    def test_comments_strings_templates_and_tests_are_not_sites(self):
        labels = [r["label"] for r in self.rows() if not r["test"]]
        self.assertNotIn("commented::out", labels)
        self.assertNotIn("in::string", labels)
        self.assertNotIn("tests::only", labels)
        self.assertTrue(any(r["label"] == "tests::only" and r["test"] for r in self.rows()))
        self.assertFalse(any(l.startswith("$") for l in labels))

    def test_window_sites_with_receiver_and_string_body(self):
        by = {r["label"]: r for r in self.rows()}
        free = by["invasion::commit_one"]
        self.assertEqual((free["macro"], free["kind"], free["area"], free["receiver"], free["fn"]), ("dual_window", "window", "Invasion", "free", "free_fn"))
        self.assertEqual((free["string_body"], free["string_call"]), ("y", "commit_one_string"))
        meth = by["combat::roll_round"]
        self.assertEqual((meth["receiver"], meth["fn"], meth["string_body"]), ("self", "Window::method", "n"))
        self.assertEqual(meth["string_call"], "self.roll")

    def test_dynamic_labels_are_flagged(self):
        dyn = [r for r in self.rows() if r["dynamic"] == "y"]
        self.assertEqual(len(dyn), 1)
        self.assertTrue(dyn[0]["label"].startswith("<expr: &label"))
        helper = scan_file("x.rs", "fn dual_w<R>(state: &mut S, label: &str) -> R { crate::dual::write(Area::X, state, label, a, b) }\nfn dual_write_x(state: &mut S, label: &str) -> R { dual_write(state, label, a, b) }", "x.rs")
        self.assertEqual([(r["fn"], r["dynamic"]) for r in helper], [("dual_w", "param"), ("dual_write_x", "param")])

    def test_const_alt_and_format_labels_are_resolved(self):
        text = """const LABEL: &str = "same::file";
fn f(p: bool){ dual_window!(Area::Driver, crate::cannon_dual::LABEL, s, |s| a_string(s), |s, w| None);
 dual_window!(Area::Driver, LABEL, s, |s| 1, |s, w| None);
 dual_window!(Area::Driver, if p { "a::primary" } else { "a::secondary" }, s, |s| 1, |s, w| None);
 run_dual_window(Area::Driver, &format!("explore::ask:{card}"), s, |s| 1, |s, w| None); }"""
        rows = scan_file("x.rs", text, "x.rs", find_consts_text(text))
        self.assertEqual([(r["label"], r["dynamic"]) for r in rows], [("cannon::head", "const"), ("same::file", "const"), ("a::primary|a::secondary", "alt"), ("explore::ask:{card}", "format")])

    def test_other_macros(self):
        by = {r["label"]: r for r in self.rows()}
        self.assertEqual((by["objectives::list"]["macro"], by["objectives::list"]["string_body"]), ("dual_typed", "y"))
        self.assertEqual((by["relics::gain"]["macro"], by["relics::gain"]["area"]), ("dual_write", "ExplorationRelics"))
        self.assertEqual((by["technology::exhaust"]["macro"], by["technology::exhaust"]["area"]), ("dual_write", "Technology"))
        self.assertEqual(by["invasion::fight_round_ask"]["kind"], "question")
        self.assertEqual((by["hit_shift"]["macro"], by["hit_shift"]["kind"]), ("dual_hook", "hook"))
        self.assertEqual(by["combat::settle"]["kind"], "gate")
        self.assertEqual((by["faction_abilities::of_player"]["macro"], by["faction_abilities::of_player"]["string_body"]), ("typed_ability", "y"))

    def test_the_definition_of_dual_write_is_not_a_site(self):
        sites = [r for r in self.rows() if r["macro"] == "dual_write" and r["fn"] == "dual_write"]
        self.assertEqual(sites, [])   # `crate::dual::write` inside the wrapper is its plumbing; the callers are the sites
        self.assertEqual(sum(1 for r in self.rows() if r["macro"] == "dual_write"), 2)

    def test_closure_params_with_commas_do_not_split_arguments(self):
        rows = scan_file("x.rs", "fn f(){ dual_window!(Area::Driver, \"a::b\", s, |x, y| Run::new(z_string(x)), |s, w| None) }", "x.rs")
        self.assertEqual((rows[0]["label"], rows[0]["string_body"]), ("a::b", "y"))

    def test_render_and_check_roundtrip(self):
        rows = [r for r in self.rows() if not r["test"]]
        text = render(rows)
        self.assertEqual(text.splitlines()[0].split("\t"), COLUMNS)
        self.assertEqual(len(text.splitlines()), len(rows) + 1)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
