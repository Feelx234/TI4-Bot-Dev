#!/usr/bin/env python3
"""D3-0 census of the string touchpoints of the rule bot (docs/todo/D3-bots.md section 1.2) and the subtype table.

    python3 -I tools/rulebot/d3/census.py            # rewrite census.txt and subtypes.tsv next to this script
    python3 -I tools/rulebot/d3/census.py --check    # regenerate in memory, exit 1 when a committed file differs

Production code only: each file is cut at its first `#[cfg(test)]`. Classes (regexes below, a lower bound: they count
lines, not statements):

  A  subtype / context dispatch          B  option id equality (`o.id == "done"`, `by_id("keep")`)
  C  option id parsing                   D  kind compare and `is_decline`
  E  prompt text                         F  payload reads        G  label reads        H  details reads
  I  ids built from text (`PlayerId::new`, `SystemId::new`, `PlanetId::new`)

Deterministic: the same tree and the same corpus give the same text. Needs the `zstd` binary for the corpus
classification (subtypes.tsv); without it the corpus columns are left empty and `--check` skips that file.
"""
import collections
import glob
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
BOT = os.path.join(REPO, "crates", "ti4-rule-bot", "src")
RENDER = os.path.join(REPO, "crates", "ti4-engine", "src", "render")
CORPUS = os.path.join(REPO, "tools", "engine_core", "decision_corpus")
BASELINE = os.path.join(REPO, "tools", "rulebot", "baseline.json")

DOC = {"A": 39, "B": 48, "C": 46, "D": 93, "E": 6, "F": 10, "G": 8, "H": 9, "I": 47}

# -- classes ---------------------------------------------------------------------------------------------------------

CLASSES = [
    ("A", "subtype / context dispatch", [
        r"\bsubtype\s*(==|!=)\s*Some\(",
        r"\bsubtype\s*==\s*\"",
        r"\bmatches!\(\s*subtype\b",
        r"\bsub\s+if\b",
        r"\bsub\.(starts_with|ends_with)\(",
        r"\bsub\s*==\s*\"",
        r"\bcontext\.(round|trigger)\b",
        r"\bc\.(round|trigger)\b",
        r"\.subtype\s*(==|!=)",
        r"\.subtype\.(starts_with|ends_with|as_str)\(",
        r"\bmatch\s+subtype\b",
    ]),
    ("B", "option id equality", [
        r"\bid\s*(==|!=)",
        r"\.id\s*(==|!=)",
        r"\bid\.as_str\(\)\s*(==|!=)",
        r"\bby_id\(",
        r"\bmatches!\(\s*\w+\.id\.as_str\(\)",
    ]),
    ("C", "option id parse", [
        r"\bid\.(starts_with|strip_prefix|split_once|rsplit|split|ends_with|strip_suffix|rsplit_once|splitn)\(",
        r"\bid\.contains\(",
        r"\bid\.as_str\(\)\.(starts_with|strip_prefix|split_once|rsplit|split|ends_with|strip_suffix|contains)\(",
        r"\b(rest|tail|head)\.(split_once|rsplit|split)\(",
    ]),
    ("D", "kind compare / is_decline", [
        r"\bkind\s*(==|!=)\s*\"",
        r"\bkind\.as_str\(\)\s*(==|!=)",
        r"\bmatches!\(\s*\w+\.kind\.as_str\(\)",
        r"\bis_decline\b",
    ]),
    ("E", "prompt text", [
        r"\bprompt\.(contains|starts_with|strip_prefix|ends_with)\(",
        r"\bprompt\s*(==|!=)",
        r"strip_prefix\(\s*\"transaction with ",
    ]),
    ("F", "payload reads", [r"\.payload\b"]),
    ("G", "label reads", [r"\.label\b"]),
    ("H", "details reads", [r"\.details\b"]),
    ("I", "ids from text", [r"\b\w+Id::new\("]),
]
COMPILED = [(c, d, [re.compile(p) for p in ps]) for c, d, ps in CLASSES]


def production(path):
    text = open(path, encoding="utf-8").read()
    cut = text.find("#[cfg(test)]")
    if cut >= 0:
        text = text[:cut]
    return text.splitlines()


def files():
    out = sorted(glob.glob(os.path.join(BOT, "*.rs")) + glob.glob(os.path.join(BOT, "*", "*.rs")))
    return [(os.path.relpath(f, BOT), f) for f in out]


def fn_spans(lines):
    """(first line index, name) of every `fn` item, to attribute a hit to its function."""
    spans = []
    for i, l in enumerate(lines):
        m = re.match(r"\s*(pub(\([a-z]+\))?\s+)?(const\s+)?fn\s+([a-z_0-9]+)", l)
        if m:
            spans.append((i, m.group(4)))
    return spans


MATCH_OPEN = re.compile(r"\bmatch\s+(\w*subtype\w*|sub|choice\.context\.as_ref\(\)\.map\(\|c\| c\.subtype\.as_str\(\)\))\s*\{")
ARM = re.compile(r'\s*(?:Some\()?((?:"[A-Za-z_0-9:]+"\s*\|?\s*)+)\)?\s*(?:if\b.*)?=>|\s*sub\s+if\b')


def subtype_arms(lines):
    """Indices of the arm lines of every `match subtype { .. }` block (a multi-text arm is one site)."""
    arms = {}
    in_match = None
    depth = 0
    for i, l in enumerate(lines):
        code = l.split("//")[0]
        if MATCH_OPEN.search(code):
            in_match = depth
        before = depth
        depth += code.count("{") - code.count("}")
        if in_match is not None and before == in_match + 1 and ARM.match(code):
            arms[i] = code
        if in_match is not None and depth <= in_match:
            in_match = None
    return arms


def census():
    total = collections.Counter()
    by_file = collections.defaultdict(collections.Counter)
    by_fn = collections.defaultdict(collections.Counter)
    prod_lines = 0
    test_lines = 0
    for rel, f in files():
        lines = production(f)
        prod_lines += len(lines)
        test_lines += len(open(f, encoding="utf-8").read().splitlines()) - len(lines)
        spans = fn_spans(lines)
        arms = subtype_arms(lines)
        for i, l in enumerate(lines):
            code = l.split("//")[0]
            for c, _, pats in COMPILED:
                if (c == "A" and i in arms) or any(p.search(code) for p in pats):
                    total[c] += 1
                    by_file[rel][c] += 1
                    name = "-"
                    for start, n in spans:
                        if start <= i:
                            name = n
                        else:
                            break
                    by_fn[(rel, name)][c] += 1
    return total, by_file, by_fn, prod_lines, test_lines


def render_census(total, by_file, by_fn, prod_lines, test_lines):
    out = []
    out.append("# D3-0 census of the string touchpoints of ti4-rule-bot (tools/rulebot/d3/census.py)")
    out.append("# production code only (cut at the first #[cfg(test)] of each file); a lower bound, lines not statements")
    out.append(f"files {len(files())}  production lines {prod_lines}  test lines {test_lines}")
    out.append("")
    out.append("## totals")
    for c, d, _ in CLASSES:
        out.append(f"{c}  {total[c]:5d}  {d}")
    out.append("")
    out.append("## against the numbers of docs/todo/D3-bots.md 1.2 (measured there on core-integration-3 with statement-level counting)")
    for c, d, _ in CLASSES:
        out.append(f"{c}  lines {total[c]:4d}  doc {DOC[c]:4d}" + ("  (A: every dispatch arm counts, the doc counted statements)" if c == "A" else ""))
    out.append(f"A+B+C+D(kind)+E lines (the 'about 120 parse sites' of ENGINE_CORE_STATE.md, line count): "
               f"{sum(total[c] for c in 'ABCDE')}")
    out.append("")
    out.append("## per file")
    head = "file".ljust(34) + "".join(c.rjust(6) for c, _, _ in CLASSES)
    out.append(head)
    for rel in sorted(by_file):
        out.append(rel.ljust(34) + "".join(str(by_file[rel][c]).rjust(6) for c, _, _ in CLASSES))
    out.append("")
    out.append("## per function (A..E only, at least 3 hits)")
    rows = []
    for (rel, name), cnt in by_fn.items():
        n = sum(cnt[c] for c in "ABCDE")
        if n >= 3:
            rows.append((-n, rel, name, cnt))
    for negn, rel, name, cnt in sorted(rows):
        parts = " ".join(f"{c}{cnt[c]}" for c in "ABCDE" if cnt[c])
        out.append(f"{-negn:4d}  {rel}::{name}  {parts}")
    return "\n".join(out) + "\n"


# -- subtype table -----------------------------------------------------------------------------------------------------

LIT = re.compile(r'"([A-Za-z_0-9:]+)"')
# Predicates on the subtype text that are code, not text (docs/todo/D3-bots.md D3-0 item 2).
PRED = re.compile(r'\b(?:sub|subtype|s|c\.subtype|context\.subtype)\.(starts_with|ends_with)\(\s*"([^"]+)"')
EQ = re.compile(r'\b(?:sub|subtype)\s*(==|!=)\s*(?:Some\()?"([^"]+)"')


def bot_sites():
    """subtype text -> first `file:line` that dispatches on it; and the parametric predicates with their sites."""
    sites = {}
    preds = collections.OrderedDict()
    for rel, f in files():
        lines = production(f)
        arms = subtype_arms(lines)
        for i, l in enumerate(lines):
            code = l.split("//")[0]
            for m in PRED.finditer(code):
                preds.setdefault((m.group(1), m.group(2)), f"{rel}:{i + 1}")
            if i in arms:
                arm = ARM.match(code)
                if arm and arm.group(1):
                    for m in LIT.finditer(arm.group(1)):
                        sites.setdefault(m.group(1), f"{rel}:{i + 1}")
            if "subtype" in code or re.search(r"\bsub\b", code):
                if re.search(r"(==|!=)\s*(Some\()?\"|matches!|\.contains\(", code):
                    for m in EQ.finditer(code):
                        sites.setdefault(m.group(2), f"{rel}:{i + 1}")
                    if "matches!" in code or ".contains(" in code:
                        for m in LIT.finditer(code):
                            sites.setdefault(m.group(1), f"{rel}:{i + 1}")
    return sites, preds


def typed_names():
    """subtype text -> 'area::Variant' from the `fn subtype` of every renderer; parametric ones are not listed."""
    names = {}
    for f in sorted(glob.glob(os.path.join(RENDER, "*.rs"))):
        area = os.path.basename(f)[:-3]
        text = open(f, encoding="utf-8").read()
        m = re.search(r"fn subtype\([^)]*\)\s*->\s*String\s*\{(.*?)\n    \}", text, re.S)
        if not m:
            continue
        for arm in re.finditer(r'Subtype::([A-Za-z0-9_]+)[^=]*=>\s*"([^"]+)"\.to_owned\(\)', m.group(1)):
            names.setdefault(arm.group(2), f"{area}::{arm.group(1)}")
    return names


UNINHABITED = ["tactical", "technology", "tokens", "diplomacy", "legendary", "promissory"]

# subtype text -> (area, owner package of docs/todo/D3-bots.md 2.3); first match wins, order matters.
GROUPS = [
    (r"^(activate_system|movement_step|load_cargo|warfare_free_tactical|remove_over_supply|.*over_fleet_supply|.*_remove)$",
     "tactical", "D3-3"),
    (r"^(action_menu|end_turn|legendary_end_of_turn|~action)$", "turn", "D3-3"),
    (r"^(produce_unit|produce_units|pay_resources|pay_influence|place_structure|construction_choose_ability|"
     r"gain_command_token|buy_token_with_influence|.*redistribute.*|self_assembly_mech|~produce|~pay)", "production", "D3-4"),
    (r"^(draft_strategy_card|strategy_secondary|research_technology|starting_technology|assimilate|~strategy|"
     r".*_pick_technology|.*_choose_technology|flayesh_technology)", "strategy", "D3-5"),
    (r"^(functioning_base_choose_reward)$", "explore", "D3-4"),
    (r"^(letnev_commander_trade_good)$", "combat", "D3-6"),
    (r"^(remove_custodians|trade_choose_replenish)$", "agenda/objectives/trade", "D3-7"),
    (r"^(umbat_player|malleon_player|malleon_payment)$", "reaction/agents", "D3-8"),
    (r"^(announce_retreat|retreat_to|commit_ground_forces|sustain_damage|assign_casualty|ready_planet|"
     r"magen_defense_hit|munitions_.*|ambush_roll|pillage_take)", "combat", "D3-6"),
    (r"^(cast_vote|vote_.*|politics_.*|predict_agenda_outcome|.*rider.*|score_objective|.*_score_objective|"
     r"score_secret_objective|return_over_secret_hand_limit|propose_transaction|answer_transaction|transaction.*)",
     "agenda/objectives/trade", "D3-7"),
    (r"^(reaction_.*|play_reaction.*|ospha_.*|mercer_take|doctor_sucaban_.*|zeu_.*|l1z1x_agent_swap|stillness_target|"
     r"suffi_an|.*agent.*)", "reaction/agents", "D3-8"),
]


def group_of(text):
    for pat, area, pkg in GROUPS:
        if re.match(pat, text):
            return area, pkg
    return "", ""


def corpus_subtypes():
    """subtype -> decisions in the recorded corpus (the 114 654 decisions); {} when zstd is missing."""
    counts = collections.Counter()
    sources = collections.defaultdict(set)
    try:
        for f in sorted(glob.glob(os.path.join(CORPUS, "*.jsonl.zst"))):
            src = os.path.basename(f).split(".")[0]
            proc = subprocess.run(["zstd", "-dc", "-q", f], capture_output=True, text=True, check=True)
            for l in proc.stdout.splitlines():
                ch = json.loads(l)["choice"]
                ctx = ch.get("context")
                if ctx and ctx.get("subtype"):
                    sub = ctx["subtype"]
                else:
                    opts = ch.get("options") or []
                    sub = "~" + (opts[0]["kind"] if opts else "none")
                counts[sub] += 1
                sources[sub].add(src)
    except (OSError, subprocess.CalledProcessError):
        return {}, {}
    return counts, sources


def baseline_stats():
    try:
        d = json.load(open(BASELINE, encoding="utf-8"))
        return d["combined"]["by_subtype"]
    except (OSError, KeyError, ValueError):
        return {}


def predicate_of(text, preds):
    """The parametric predicates of the bot that accept `text`."""
    hits = []
    for (kind, arg), site in preds.items():
        if (kind == "starts_with" and text.startswith(arg)) or (kind == "ends_with" and text.endswith(arg)):
            hits.append(f"{kind}({arg})")
    return hits


def subtype_table():
    sites, preds = bot_sites()
    names = typed_names()
    counts, sources = corpus_subtypes()
    stats = baseline_stats()
    texts = sorted(set(counts) | set(sites) | set(stats))
    rows = ["\t".join(["subtype", "bot_site", "predicates", "typed_subtype", "area", "owner",
                       "corpus_n", "corpus_sources", "baseline_calls", "baseline_smart", "baseline_ns"])]
    for t in texts:
        area, pkg = group_of(t)
        pr = predicate_of(t, preds)
        if not area:
            for p in pr:
                a2, k2 = group_of(re.sub(r"^.*\((.*)\)$", r"\1", p))
                if p.startswith("starts_with(reaction_") or p.startswith("starts_with(play_reaction"):
                    area, pkg = "reaction/agents", "D3-8"
                elif "_technology" in p:
                    area, pkg = "strategy", "D3-5"
        typed = names.get(t, "")
        if not typed and area in ("tactical",) and t in sites:
            typed = "uninhabited (core-a2b)"
        if not typed and area == "turn":
            typed = "uninhabited (D1b-4)"
        s = stats.get(t, {})
        used = "yes" if (t in sites or pr) else "no-opinion"
        rows.append("\t".join([
            t, sites.get(t, ""), ";".join(pr), typed or ("" if used == "yes" else "-"), area or ("-" if used != "yes" else "?"),
            pkg or ("router" if used != "yes" else "?"),
            str(counts.get(t, "")), ",".join(sorted(sources.get(t, []))),
            str(s.get("n", "")), str(s.get("smart", "")), str(s.get("ns", "")),
        ]))
    rows.append("")
    rows.append("# parametric predicates (code, not text): predicate -> site")
    for (kind, arg), site in preds.items():
        rows.append(f"# {kind}({arg})\t{site}")
    return "\n".join(rows) + "\n", len(texts), len(counts)


def main():
    check = "--check" in sys.argv[1:]
    total, by_file, by_fn, prod_lines, test_lines = census()
    census_txt = render_census(total, by_file, by_fn, prod_lines, test_lines)
    table, n_texts, n_corpus = subtype_table()
    want = {"census.txt": census_txt}
    if n_corpus:
        want["subtypes.tsv"] = table
    bad = 0
    for name, text in want.items():
        path = os.path.join(HERE, name)
        if check:
            have = open(path, encoding="utf-8").read() if os.path.exists(path) else ""
            if have != text:
                print(f"{name}: differs from the regenerated text", file=sys.stderr)
                bad += 1
        else:
            with open(path, "w", encoding="utf-8") as fh:
                fh.write(text)
    if not check:
        print(census_txt.split("## per file")[0])
        print(f"subtypes.tsv: {n_texts} rows, {n_corpus} distinct subtype texts in the corpus")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
