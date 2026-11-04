#!/usr/bin/env python3
"""Silent-gap diff (d): the decision subtypes the string engine puts as questions vs the subtypes the core renderers can produce.

    python3 -I tools/engine_core/silent_gaps_d.py [--tsv FILE] [--summary]

Two string-side sources: the observed corpus (`tools/rulebot/d3/subtypes.tsv`, with its frequency) and every literal third argument of
`DecisionContext::new(..)` in crates/ti4-engine/src (production code). Core side: the string literals and `format!` templates of every
`fn subtype` in crates/ti4-engine-next/src/render/*.rs (the closed set `ti4_engine_core::choice::Subtype` renders to). A string subtype that
no core renderer produces cannot be asked by the native path: the question is lent (a bail) or, when the native window never reaches it,
a silent absence. Static: no name resolution; a subtype built by a helper shows as parametric (`~name`) and is listed apart.
"""
import glob
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from lent_sites import mask, scopes, split_args, line_of  # noqa: E402

REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
STR = re.compile(r'"((?:[^"\\]|\\.)*)"')


def prod_text(t):
    i = t.find("#[cfg(test)]")
    return t if i < 0 else t[:i]


def core_set():
    lits, tpls = {}, []
    for f in sorted(glob.glob(os.path.join(REPO, "crates/ti4-engine-next/src/render/*.rs"))):
        t = prod_text(open(f).read())
        m = re.search(r"fn subtype\(.*?\n    }\n", t, re.S)
        if not m or f.endswith("mod.rs"):
            continue
        area = os.path.basename(f)[:-3]
        for s in STR.findall(m.group(0)):
            if "{" in s:
                tpls.append((re.compile("^" + re.escape(s).replace(r"\{\}", ".+").replace(r"\ ", " ") + "$"), s, area))
            elif s:
                lits[s] = area
    return lits, tpls


def wider_sets():
    """Looser evidence than the `fn subtype` bodies: every string literal of the render files (helpers build subtypes too), and of the core crates."""
    render, corecr = {}, {}
    for f in sorted(glob.glob(os.path.join(REPO, "crates/ti4-engine-next/src/render/*.rs"))):
        for s in STR.findall(mask_keep(prod_text(open(f).read()))):
            render.setdefault(s, os.path.basename(f))
    for root in ("crates/ti4-engine-core/src", "crates/ti4-engine-core-factions/src"):
        for f in sorted(glob.glob(os.path.join(REPO, root, "**/*.rs"), recursive=True)):
            if "/generated/" in f or "/tests/" in f or f.endswith("_tests.rs"):
                continue
            for s in STR.findall(prod_text(open(f).read())):
                corecr.setdefault(s, os.path.relpath(f, os.path.join(REPO, "crates")))
    return render, corecr


def mask_keep(t):
    return t


def literal_sites(wanted):
    """subtype -> [(file:line, enclosing fn name as lent_sites.tsv writes it)] for every production occurrence of the literal outside render/."""
    out = {}
    src = os.path.join(REPO, "crates/ti4-engine/src")
    pat = {w: re.compile('"%s"' % re.escape(w)) for w in wanted}
    for f in sorted(glob.glob(os.path.join(src, "**/*.rs"), recursive=True)):
        rel = os.path.relpath(f, src)
        if rel.startswith("render/") or rel.endswith("_tests.rs") or "tests/" in rel:
            continue
        t = open(f).read()
        hits = [(w, m.start()) for w, rx in pat.items() for m in rx.finditer(t)]
        if not hits:
            continue
        mk = mask(t)
        at = scopes(mk)
        for w, off in hits:
            fn, impl, test = at(off)
            if test:
                continue
            name = ("%s::%s" % (impl, fn.name) if impl else fn.name) if fn else "<outside any fn>"
            out.setdefault(w, []).append(("%s:%d" % (rel, line_of(t, off)), name, rel))
    return out


def lent_rows():
    rows = {}
    path = os.path.join(HERE, "lent_sites.tsv")
    L = open(path).read().split("\n")
    hdr = L[0].split("\t")
    for l in L[1:]:
        if l:
            d = dict(zip(hdr, l.split("\t")))
            rows.setdefault((d["site"].split(":")[0], d["fn"]), []).append(d["label"])
    return rows


def corpus():
    L = open(os.path.join(REPO, "tools/rulebot/d3/subtypes.tsv")).read().split("\n")
    hdr = L[0].split("\t")
    out = []
    for l in L[1:]:
        if not l or l.startswith("#"):
            continue
        d = dict(zip(hdr, l.split("\t") + [""] * 20))
        out.append(d)
    return out


def string_sites():
    """subtype literal -> [file:line] of DecisionContext::new in production code of the string engine."""
    sites, dyn = {}, []
    for f in sorted(glob.glob(os.path.join(REPO, "crates/ti4-engine/src/**/*.rs"), recursive=True)):
        rel = os.path.relpath(f, os.path.join(REPO, "crates/ti4-engine/src"))
        if "/render/" in "/" + rel or rel.endswith("_tests.rs") or "tests/" in rel or rel in ("decision_context.rs",):
            continue
        t = open(f).read()
        m = mask(t)
        at = scopes(m)
        for mt in re.finditer(r"\bDecisionContext::new\s*\(", m):
            fn, impl, test = at(mt.start())
            if test:
                continue
            args, _ = split_args(m, mt.end() - 1)
            if len(args) < 3:
                continue
            a = t[args[2][0]:args[2][1]].strip()
            ln = line_of(t, mt.start())
            lit = STR.fullmatch(a)
            if lit:
                sites.setdefault(lit.group(1), []).append("%s:%d" % (rel, ln))
            else:
                dyn.append(("%s:%d" % (rel, ln), a.replace("\n", " ")[:80]))
    return sites, dyn


def main(argv):
    lits, tpls = core_set()

    def core_of(s):
        if s in lits:
            return lits[s]
        for rx, src, area in tpls:
            if rx.match(s):
                return area + " (template " + src + ")"
        return ""

    render_lits, core_lits = wider_sets()

    def level(s):
        c = core_of(s)
        if c:
            return "fn-subtype:" + c
        if s in render_lits:
            return "render-file:" + render_lits[s]
        if s in core_lits:
            return "core-crate:" + core_lits[s]
        return ""

    sites, dyn = string_sites()
    rows = {}
    for d in corpus():
        s = d["subtype"]
        if s.startswith("#"):
            continue
        rows[s] = {"subtype": s, "n": int(d["corpus_n"] or 0), "area": d["area"], "where": ",".join(sites.get(s, [])[:2]), "core": level(s), "src": "corpus"}
    for s, w in sites.items():
        if s in rows:
            continue
        rows[s] = {"subtype": s, "n": 0, "area": "", "where": ",".join(w[:2]), "core": level(s), "src": "source"}
    missing = [r for r in rows.values() if not r["core"] and not r["subtype"].startswith("~")]
    weak = [r for r in rows.values() if r["core"] and not r["core"].startswith("fn-subtype")]
    missing.sort(key=lambda r: (-r["n"], r["subtype"]))
    occ_by = literal_sites([r["subtype"] for r in missing])
    lent = lent_rows()
    for r in rows.values():
        r["fnsite"], r["lent"] = "", ""
        occ = occ_by.get(r["subtype"], [])
        if occ:
            r["fnsite"] = ";".join("%s %s" % (a, b) for a, b, _ in occ[:3])
            labels = sorted({lab for _, name, rel in occ for lab in lent.get((rel, name), [])})
            r["lent"] = ",".join(labels)
    if "--tsv" in argv:
        path = argv[argv.index("--tsv") + 1]
        with open(path, "w") as fh:
            fh.write("subtype\tcorpus_n\tstring_location\tcore_renderer\tsource\tstring_fn\tlent_sites_in_that_fn\n")
            for r in sorted(rows.values(), key=lambda r: (bool(r["core"]), -r["n"], r["subtype"])):
                fh.write("\t".join([r["subtype"], str(r["n"]), r["where"], r["core"], r["src"], r["fnsite"], r["lent"]]) + "\n")
    print("string subtypes: %d (corpus %d, source-only %d); core renderers: %d literals + %d templates; no core renderer: %d (parametric '~' rows left out); only a looser match (render file or core crate literal): %d"
          % (len(rows), sum(1 for r in rows.values() if r["src"] == "corpus"), sum(1 for r in rows.values() if r["src"] == "source"), len(lits), len(tpls), len(missing), len(weak)))
    if "--summary" in argv:
        for r in sorted(weak, key=lambda r: -r["n"]):
            print("weak %6d  %-40s %s" % (r["n"], r["subtype"], r["core"]))
    print("dynamic third arguments (not literals): %d" % len(dyn))
    if "--summary" in argv:
        for r in missing:
            print("%6d  %-40s %s | %s | lent: %s" % (r["n"], r["subtype"], r["where"] or "-", r["fnsite"], r["lent"] or "none"))
        for w, a in dyn:
            print("dyn  %s  %s" % (w, a))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
