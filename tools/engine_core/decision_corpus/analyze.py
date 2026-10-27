#!/usr/bin/env python3
"""Derive the option grammar tables of docs/DECISION_FAMILIES.md from the recorded corpus and the engine sources.

    python3 tools/engine_core/decision_corpus/analyze.py [CORPUS_DIR] > tables.md

Reads every <source>.jsonl.zst of the corpus (needs the `zstd` binary), groups every offered option by
`kind`, and prints per kind: how often it is offered, the id grammar (separator, segment shapes), the
payload keys with their JSON number types, label and prompt templates, the order rule, the context
subtypes and the source files whose literals build it. Also prints the site inventory of the engine
(builder calls outside test modules, per file). Deterministic: the same corpus gives the same text.
"""
import collections
import glob
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CORPUS = next((a for a in sys.argv[1:] if not a.startswith("--")), HERE)
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
SEP = re.compile(r"[|:>]")


def lines():
    for f in sorted(glob.glob(os.path.join(CORPUS, "*.jsonl.zst"))):
        out = subprocess.run(["zstd", "-dc", "-q", f], capture_output=True, text=True, check=True).stdout
        src = os.path.basename(f).split(".")[0]
        for l in out.splitlines():
            yield src, json.loads(l)


def jtype(v):
    if isinstance(v, bool):
        return "bool"
    if isinstance(v, int):
        return "int"
    if isinstance(v, float):
        return "float"
    if v is None:
        return "null"
    if isinstance(v, str):
        return "str"
    return "array" if isinstance(v, list) else "object"


def norm_num(s):
    return re.sub(r"-?\d+(\.\d+)?", "#", s)


def seg_class(v):
    if re.fullmatch(r"-?\d+", v):
        return "int"
    if v == "":
        return "empty"
    return "name"


class Kind:
    def __init__(self):
        self.n = 0
        self.choices = set()
        self.ids = collections.Counter()
        self.segvals = collections.defaultdict(collections.Counter)  # (nseg, pos) -> values
        self.shapes = collections.Counter()
        self.payload = collections.defaultdict(collections.Counter)  # key -> type counter
        self.payload_sets = collections.Counter()
        self.labels = collections.Counter()
        self.label_ex = {}
        self.prompts = collections.Counter()
        self.prompt_ex = {}
        self.subtypes = collections.Counter()
        self.sources = collections.Counter()
        self.order = collections.Counter()
        self.preview = 0
        self.extra = collections.Counter()  # other option fields
        self.free_form = collections.Counter()
        self.sizes = []
        self.examples = []


def label_template(label, idsegs):
    t = label
    for s in sorted({x for x in idsegs if len(x) > 1 and not x.isdigit()}, key=len, reverse=True):
        t = re.sub(r"(?<![A-Za-z0-9_])" + re.escape(s) + r"(?![A-Za-z0-9_])", "<id>", t)
    return norm_num(t)


def prompt_template(p):
    """Fold the variable parts of a prompt: event names, snake_case ids, a trailing seat letter, numbers."""
    p = re.sub(r"\b[A-Z][A-Z_]{3,}\b", "<EVENT>", p)
    p = re.sub(r"\b[a-z][a-z0-9]*_[a-z0-9_]+\b", "<id>", p)
    p = re.sub(r"\b(vote|for|to) [a-h]$", r"\1 <seat>", p)
    p = re.sub(r"\bl1z1x\b", "<id>", p)
    return norm_num(p)


def sorted_by(seq, key):
    try:
        ks = [key(x) for x in seq]
    except Exception:
        return False
    return all(a <= b for a, b in zip(ks, ks[1:]))


def num_key(i):
    m = re.match(r"-?\d+", i)
    if not m:
        raise ValueError
    return (int(m.group(0)), i)


def main():
    kinds = collections.defaultdict(Kind)
    choices = 0
    decisions_by_source = collections.Counter()
    prompt_total = collections.Counter()
    ctx_fields = collections.Counter()
    top_keys = collections.Counter()
    details_keys = collections.Counter()
    decline_pos = collections.Counter()
    for src, d in lines():
        c = d["choice"]
        decisions_by_source[src] += 1
        choices += 1
        for k in c:
            top_keys[k] += 1
        for k in c.get("details", {}):
            details_keys[k] += 1
        for k in (c.get("context") or {}):
            ctx_fields[k] += 1
        opts = c["options"]
        ids = [o["id"] for o in opts]
        sub = (c.get("context") or {}).get("subtype", "")
        pt = prompt_template(c["prompt"])
        prompt_total[pt] += 1
        by_kind = collections.defaultdict(list)
        for i, o in enumerate(opts):
            by_kind[o["kind"]].append((i, o))
        for kind, group in by_kind.items():
            kd = kinds[kind]
            kd.n += len(group)
            key = (src, d["g"], d["step"], d["k"])
            kd.choices.add(key)
            kd.sources[src] += 1
            kd.prompts[pt] += 1
            kd.prompt_ex.setdefault(pt, c["prompt"])
            kd.subtypes[sub] += 1
            if c.get("free_form"):
                kd.free_form[c["free_form"]] += 1
            kd.sizes.append(len(opts))
            gids = [o["id"] for _, o in group]
            pos = [i for i, _ in group]
            contiguous = pos == list(range(pos[0], pos[0] + len(pos)))
            if len(group) > 1:
                if sorted_by(gids, lambda x: x):
                    kd.order["ids ascending (string)"] += 1
                elif sorted_by(gids, num_key):
                    kd.order["ids ascending (leading integer)"] += 1
                elif sorted_by(gids, lambda x: x) is False and sorted_by(list(reversed(gids)), lambda x: x):
                    kd.order["ids descending"] += 1
                else:
                    kd.order["insertion order"] += 1
                kd.order["contiguous" if contiguous else "interleaved with other kinds"] += 1
            for i, o in group:
                kd.ids[o["id"]] += 1
                segs = SEP.split(o["id"])
                sepsig = "".join(ch for ch in o["id"] if ch in "|:>")
                kd.shapes[(len(segs), sepsig)] += 1
                for p, s in enumerate(segs):
                    kd.segvals[(len(segs), sepsig, p)][s] += 1
                kd.payload_sets[tuple(sorted(o["payload"]))] += 1
                for pk, pv in o["payload"].items():
                    kd.payload[pk][jtype(pv)] += 1
                lt = label_template(o["label"], segs)
                kd.labels[lt] += 1
                kd.label_ex.setdefault(lt, o["label"])
                for ek in o:
                    if ek not in ("id", "kind", "label", "payload"):
                        kd.extra[ek] += 1
                if len(kd.examples) < 3 and o["id"] not in [e[0] for e in kd.examples]:
                    kd.examples.append((o["id"], o["label"], o["payload"]))
                if kind == "decline":
                    decline_pos["last" if i == len(opts) - 1 else ("first" if i == 0 else "middle")] += 1
    print_report(kinds, choices, decisions_by_source, prompt_total, ctx_fields, top_keys, details_keys, decline_pos)


def seg_desc(vals, total):
    if all(seg_class(v) == "int" for v in vals):
        lo, hi = min(int(v) for v in vals), max(int(v) for v in vals)
        return f"int {lo}..{hi}"
    if len(vals) <= 6:
        return "{" + ",".join(sorted(vals)) + "}"
    cls = {seg_class(v) for v in vals}
    return f"{len(vals)} distinct ({'/'.join(sorted(cls))})" + (" e.g. " + ", ".join(sorted(vals)[:3]) if len(vals) < 40 else "")


def id_grammar(kd):
    out = []
    for (n, sig), cnt in kd.shapes.most_common(4):
        parts = []
        for p in range(n):
            vals = kd.segvals[(n, sig, p)]
            parts.append(seg_desc(vals, cnt))
        seps = [""] + list(sig) if len(sig) == n - 1 else [""] * n
        text = ""
        for p, s in zip(parts, seps):
            text += (s if s else ("|" if text else "")) + "`" + p + "`"
        out.append(f"{text} x{cnt}")
    more = len(kd.shapes) - 4
    return "; ".join(out) + (f"; +{more} more shapes" if more > 0 else "")


AREAS = {
    "generic (decline, done, yes/no, numbers)": "decline end_turn answer count",
    "strategy phase and action menu": "strategy_card strategy action pool redistribute",
    "tactical action (activate, move, load, transit)": "activate move load transit ship system",
    "combat and invasion": "commit commit_infantry sustain casualty ground_casualty retreat retreat_to reroll_die extra_die ambush_roll remove unit infantry cruiser rescue_from produce_hit destroy_own_ship",
    "production, payment and research": "produce build pay place place_unit produce_unit salvage production production_discount production_agent research research_waiver research_waiver_payment starting_technology technology ready ready_technology",
    "transactions, trade and economy": "open_transaction transaction offer replenish economy pillage give_note promissory_note",
    "agenda and voting": "vote vote_planet vote_trade_goods vote_predictive tiebreak prediction law elect agenda repeal follower recombine tribute speaker player",
    "objectives, relics, exploration, cards": "score objective discard custodians component component_action action_card own_hand_card take_revealed_card relic fragment explore planet legendary ability",
    "leaders": "leader agent hero commander breakthrough",
}


SPACED = {"strategy card": "strategy phase and action menu", "promissory note": "transactions, trade and economy"}


def area_of(kind):
    if kind in SPACED:
        return SPACED[kind]
    for area, ks in AREAS.items():
        if kind in ks.split():
            return area
    if kind.startswith("leader_"):
        return "leaders"
    return "faction mechanics (one family per faction ability)"


def esc(s):
    return s.replace("|", "\\|").replace("\n", " ")


def source_files():
    """kind literal -> files of ti4-engine (non-test code) that mention it as a string literal."""
    files = {}
    for f in glob.glob(os.path.join(REPO, "crates", "ti4-engine", "src", "**", "*.rs"), recursive=True):
        s = open(f).read()
        i = s.find("#[cfg(test)]")
        files[os.path.relpath(f, os.path.join(REPO, "crates", "ti4-engine", "src"))] = s[:i] if i >= 0 else s
    return files


def kind_files(kind, files):
    pat = re.compile(r'"' + re.escape(kind) + r'"')
    hits = []
    for name, s in files.items():
        for m in pat.finditer(s):
            ctx = s[max(0, m.start() - 160): m.start()]
            if "ChoiceOption" in ctx or "labelled" in ctx or "::new(" in ctx or "kind" in ctx:
                hits.append(name)
                break
    return sorted(hits)


def print_report(kinds, choices, by_source, prompt_total, ctx_fields, top_keys, details_keys, decline_pos):
    files = source_files()
    total_opts = sum(k.n for k in kinds.values())
    print("<!-- generated by tools/engine_core/decision_corpus/analyze.py: do not edit by hand -->\n")
    print("### Corpus totals\n")
    print("| Source | Decisions |\n|---|---:|")
    for s, n in sorted(by_source.items()):
        print(f"| {s} | {n} |")
    print(f"| **all** | **{choices}** |\n")
    print(f"Offered options: {total_opts}; distinct option kinds: {len(kinds)}; distinct prompt templates (digits folded): {len(prompt_total)}.\n")
    print("Fields present on a rendered `Choice`: " + ", ".join(f"`{k}` ({n})" for k, n in sorted(top_keys.items())) + ".\n")
    print("Context fields present: " + ", ".join(f"`{k}` ({n})" for k, n in sorted(ctx_fields.items())) + ".\n")
    print("`details` keys: " + ", ".join(f"`{k}` ({n})" for k, n in sorted(details_keys.items())) + ".\n")
    print("Position of `decline` within a decision: " + ", ".join(f"{k} {n}" for k, n in decline_pos.most_common()) + ".\n")

    print("### Kind index\n")
    print("| Kind | Options | Decisions | Id grammar (separator-split segments) |\n|---|---:|---:|---|")
    for kind, kd in sorted(kinds.items(), key=lambda kv: -kv[1].n):
        print(f"| `{kind}` | {kd.n} | {len(kd.choices)} | {esc(id_grammar(kd))} |")
    print()

    print("### Per-kind grammar\n")
    print("Each block: payload keys with JSON number types (`int` is serialised without a fraction, `float` with one: they are not interchangeable in the wire bytes), label templates (ids folded to `<id>`, numbers to `#`), prompt templates, order rule, subtypes, sources.\n")
    for kind, kd in sorted(kinds.items(), key=lambda kv: -kv[1].n):
        print(f"#### `{kind}`\n")
        print(f"- offered {kd.n} times in {len(kd.choices)} decisions; per decision {min(kd.sizes)}..{max(kd.sizes)} options in the whole choice")
        print(f"- ids: {id_grammar(kd)}")
        print("  examples: " + "; ".join(f"`{e[0]}` / \"{e[1]}\"" for e in kd.examples))
        if kd.payload:
            sets = "; ".join(f"{{{','.join(s) or '-'}}} x{n}" for s, n in kd.payload_sets.most_common(4))
            types = ", ".join(f"`{k}`: " + "/".join(f"{t} {n}" for t, n in tc.most_common()) for k, tc in sorted(kd.payload.items()))
            print(f"- payload: {types}; key sets: {sets}")
        else:
            print("- payload: none")
        if kd.extra:
            print("- other option fields: " + ", ".join(f"`{k}` x{n}" for k, n in kd.extra.items()))
        lab = kd.labels.most_common(3)
        print(f"- label: {len(kd.labels)} template(s): " + "; ".join(f"\"{esc(kd.label_ex[t])}\" -> `{esc(t)}` x{n}" for t, n in lab))
        pr = kd.prompts.most_common(3)
        print(f"- prompt: {len(kd.prompts)} template(s): " + "; ".join(f"\"{esc(kd.prompt_ex[t][:110])}\" x{n}" for t, n in pr))
        if kd.order:
            print("- order: " + ", ".join(f"{k} x{n}" for k, n in kd.order.most_common()))
        else:
            print("- order: never more than one of this kind in a decision")
        if kd.free_form:
            print("- free_form prefixes: " + ", ".join(f"`{k}`" for k in kd.free_form))
        print("- subtypes: " + ", ".join(f"`{k or '(none)'}` x{n}" for k, n in kd.subtypes.most_common(5)))
        print("- sources: " + ", ".join(f"{k} {n}" for k, n in sorted(kd.sources.items())))
        kf = kind_files(kind, files)
        print("- built in: " + (", ".join(f"`{f}`" for f in kf[:6]) + (f" (+{len(kf) - 6})" if len(kf) > 6 else "") if kf else "not found as a literal (built from a variable or in host/bot code)"))
        print()

    # Cross-kind payload summary: the number-type traps
    ptypes = collections.defaultdict(lambda: collections.defaultdict(collections.Counter))
    for kind, kd in kinds.items():
        for pk, tc in kd.payload.items():
            for t, n in tc.items():
                ptypes[pk][kind][t] += n
    print("### Payload keys by type (all kinds)\n")
    print("A key whose values are `float` anywhere, or are `int` in one kind and `float`/`str` in another, needs its number type pinned by the family.\n")
    print("| Key | Types (kinds) |\n|---|---|")
    for pk in sorted(ptypes):
        tt = collections.defaultdict(list)
        for kind, tc in ptypes[pk].items():
            for t in tc:
                tt[t].append(kind)
        cell = "; ".join(f"{t}: " + ", ".join(f"`{k}`" for k in sorted(v)[:8]) + (f" (+{len(v) - 8})" if len(v) > 8 else "") for t, v in sorted(tt.items()))
        print(f"| `{pk}` | {esc(cell)} |")
    print()
    common = sum(n for _, n in prompt_total.most_common() if n >= 10)
    print(f"Prompt templates seen at least 10 times: {sum(1 for n in prompt_total.values() if n >= 10)}, covering {100 * common / choices:.1f}% of the decisions; the long tail ({sum(1 for n in prompt_total.values() if n < 10)} templates) is card, planet and faction names inside prose.\n")
    print("### Proposed typed areas (each kind assigned to one `Ask` area)\n")
    print("A kind never changes area, so one `choice/<area>.rs` owns all of its options. `strategy card` and `promissory note` (kinds with a space) are separate, older kinds from `strategy_card` / `promissory_note`: byte-identical output keeps both spellings.\n")
    print("| Area | Kinds (options offered) |\n|---|---|")
    grouped = collections.defaultdict(list)
    for kind in kinds:
        grouped[area_of(kind)].append(kind)
    for area in [*AREAS, "faction mechanics (one family per faction ability)"]:
        ks = sorted(grouped.get(area, []), key=lambda k: -kinds[k].n)
        if ks:
            print(f"| {area} | " + ", ".join(f"`{k}` ({kinds[k].n})" for k in ks) + " |")
    print()
    print("### Families by module (kinds whose literal is built in a file)\n")
    print("| Module (ti4-engine/src) | Option-builder sites | Kinds built there |\n|---|---:|---|")
    modkinds = collections.defaultdict(set)
    for kind in kinds:
        for f in kind_files(kind, files):
            modkinds[f].add(kind)
    site = {}
    for name, src in files.items():
        site[name] = len(re.findall(r"ChoiceOption::(?:new|labelled)\(|\.with\(\"|\.with_planet(?:_located)?\(|\boffer_[a-z_]+\(", src))
    for f, ks in sorted(modkinds.items(), key=lambda kv: -site.get(kv[0], 0)):
        print(f"| `{f}` | {site.get(f, 0)} | " + ", ".join(f"`{k}`" for k in sorted(ks)) + " |")
    print()
    print("### Prompt templates by frequency (top 60)\n")
    print("| Decisions | Template (digits folded) |\n|---:|---|")
    for p, n in prompt_total.most_common(60):
        print(f"| {n} | {esc(p[:150])} |")
    print()

    # Site inventory
    print("### Option-building site inventory (ti4-engine, outside `#[cfg(test)]`)\n")
    print("Sites = `ChoiceOption::new|labelled` calls + `.with(\"..\")` payload setters + `offer_*` detail builders + `Choice` constructors (`Choice::new`/`Choice {`). Counted per file.\n")
    rows = []
    for name, s in files.items():
        a = len(re.findall(r"ChoiceOption::(?:new|labelled)\(", s))
        b = len(re.findall(r"\.with\(\"|\.with_planet(?:_located)?\(", s))
        c = len(re.findall(r"\boffer_[a-z_]+\(", s))
        d = len(re.findall(r"\bChoice::new\(|\bChoice \{", s))
        if a + b + c + d:
            rows.append((name, a, b, c, d))
    rows.sort(key=lambda r: -(r[1] + r[2] + r[3]))
    print("| File | option ctors | payload setters | offer_* | Choice ctors |\n|---|---:|---:|---:|---:|")
    for r in rows:
        print(f"| `{r[0]}` | {r[1]} | {r[2]} | {r[3]} | {r[4]} |")
    print(f"| **total** | **{sum(r[1] for r in rows)}** | **{sum(r[2] for r in rows)}** | **{sum(r[3] for r in rows)}** | **{sum(r[4] for r in rows)}** |")
    print(f"\nOption-builder sites (ctors + setters + offer_*): **{sum(r[1] + r[2] + r[3] for r in rows)}**.")


if __name__ == "__main__":
    if "--write-doc" in sys.argv:
        sys.argv.remove("--write-doc")
        doc = os.path.join(REPO, "docs", "DECISION_FAMILIES.md")
        head = open(doc).read().split("<!-- generated by")[0].rstrip("\n") + "\n\n"
        import io
        buf = io.StringIO()
        old, sys.stdout = sys.stdout, buf
        try:
            main()
        finally:
            sys.stdout = old
        open(doc, "w").write(head + buf.getvalue())
    else:
        main()
