#!/usr/bin/env python3
"""Card usage audit: which fields of each card record are read, by whom, and which card ids the
code names.

Reproducible and read-only. Run from the repository root:

    python3 -I tools/card_audit/card_audit.py            # writes docs/card_audit/*
    python3 -I tools/card_audit/card_audit.py --check    # prints a summary, writes nothing

Inputs
  * crates/ti4-content/content/*.json      the card records (15 card-like categories)
  * crates/*/src, crates/*/examples         Rust sources (production code only, see below)
  * web/src                                 TypeScript sources (non-test)
  * tools/card_audit/curation.json          hand-made judgements: the classification of each
                                            (category, field), notes on how a field is used, and
                                            the per-card issues found by reading the code. Every
                                            curated claim names its evidence; the script checks
                                            that each evidence site still contains its pattern.
  * docs/card_audit/registry_dump.json      optional: the engine's own registries, written by
                                            `cargo run -p ti4-engine --example card_registry_dump`.

Outputs (docs/card_audit/)
  * fields.csv      category x field: records carrying it, bytes in the embedded (minified) JSON,
                    reader sites by component, classification
  * cards.csv       one row per card: id, category, source, fields present, implementation
                    status (registry / literal references), id-reference counts, issues
  * id_refs.json    ids named in code that are not in content (both directions), per category
  * audit.json      everything above in one file, for the artifact page

What "measured" means here: the reader sites are found by pattern (record accessor calls such as
`.text("window")`, the host's `text(record, "window")`, TypeScript property reads of the
catalog's field names). Attribution of a site to a category uses the nearest `ContentType::X` in
the enclosing function, or the field's uniqueness to one category; sites that cannot be
attributed that way are listed as unattributed rather than guessed. The classification column is
curated (the code was read), not computed.
"""

from __future__ import annotations

import argparse
import csv
import json
import os
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONTENT = ROOT / "crates" / "ti4-content" / "content"
OUT = ROOT / "docs" / "card_audit"
CURATION = Path(__file__).resolve().parent / "curation.json"

# category key -> (json file, identity field, ContentType variant)
CATEGORIES = {
    "action_cards": ("action_cards.json", "alias", "ActionCards"),
    "agendas": ("agendas.json", "alias", "Agendas"),
    "promissory_notes": ("promissory_notes.json", "alias", "PromissoryNotes"),
    "relics": ("relics.json", "alias", "Relics"),
    "leaders": ("leaders.json", "id", "Leaders"),
    "breakthroughs": ("breakthroughs.json", "alias", "Breakthroughs"),
    "public_objectives": ("public_objectives.json", "alias", "PublicObjectives"),
    "secret_objectives": ("secret_objectives.json", "alias", "SecretObjectives"),
    "technologies": ("technologies.json", "alias", "Technologies"),
    "abilities": ("abilities.json", "id", "Abilities"),
    "explores": ("explores.json", "id", "Explores"),
    "attachments": ("attachments.json", "id", "Attachments"),
    "strategy_cards": ("strategy_cards.json", "id", "StrategyCards"),
    "galactic_events": ("galactic_events.json", "alias", "GalacticEvents"),
    "genericcards": ("genericcards.json", "alias", "GenericCards"),
}
VARIANT_TO_CAT = {v[2]: k for k, v in CATEGORIES.items()}
# Non-card ContentTypes, so that a site next to `ContentType::Units` is not given to a card.
OTHER_VARIANTS = {
    "Units", "Planets", "Systems", "Factions", "Tokens", "Colors", "CombatModifiers", "Decks",
    "FrankenErrata", "MapTemplates", "Rules", "Sources", "StrategyCardSets",
}

# crate -> component. Components in RUNTIME are the ones that run in a game (server, wasm page,
# self-play); the rest are research/analysis tools and are reported separately.
COMPONENT = {
    "ti4-model": "model",
    "ti4-content": "content",
    "ti4-engine": "engine",
    "ti4-host": "host",
    "ti4-rule-bot": "bot",
    "ti4-policy": "bot",
    "ti4-bot-agent": "bot",
    "ti4-advisor": "bot",
    "ti4-wasm": "wasm",
    "ti4-server": "server",
    "ti4-selfplay": "selfplay",
}
RUNTIME = ["content", "engine", "host", "bot", "wasm", "server", "selfplay", "web"]

ACCESSOR = re.compile(
    r"""(?:\.(?:text|int|float|flag|strings|raw)\(\s*"(?P<a>[A-Za-z0-9_]+)"\s*\)"""
    # the host's `text(record, "k")` / `field(record, "k")` helpers and the `field("k")` /
    # `printed("k")` / `text_of(record, "k")` closures of engine and host
    r"""|\b(?:text|strings|field|printed|text_of)\(\s*(?:\w+\s*,\s*)?"(?P<b>[A-Za-z0-9_]+)"\s*\)"""
    r"""|\.get\(\s*"(?P<c>[A-Za-z0-9_]+)"\s*\))"""
)
CT = re.compile(r"ContentType::([A-Za-z]+)")
FN_START = re.compile(r"^\s*(?:pub(?:\([a-z]+\))?\s+)?(?:const\s+)?(?:async\s+)?fn\s+\w+")
RUST_STR = re.compile(r'r#?"(?:[^"\\]|\\.)*?"#?|"((?:[^"\\]|\\.)*)"')
TS_STR = re.compile(r'"((?:[^"\\]|\\.)*)"|\'((?:[^\'\\]|\\.)*)\'')


def minified_size(value) -> int:
    return len(json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8"))


# ---------------------------------------------------------------------------------------------
# Content


def load_content():
    cards = {}
    for cat, (file, key, _variant) in CATEGORIES.items():
        records = json.loads((CONTENT / file).read_text(encoding="utf-8-sig"))
        cards[cat] = [(r[key], r) for r in records]
    return cards


def all_content_ids():
    """Every identity of every category (cards and non-cards), for the typo check."""
    known = defaultdict(set)
    for path in sorted(CONTENT.glob("*.json")):
        if path.name == "manifest.json":
            continue
        data = json.loads(path.read_text(encoding="utf-8-sig"))
        if not isinstance(data, list):
            continue
        for r in data:
            if not isinstance(r, dict):
                continue
            for k in ("id", "alias", "tileId", "asyncId"):
                if isinstance(r.get(k), str):
                    known[r[k]].add(path.stem)
            for k in ("aliases", "aliasList"):
                for a in r.get(k, []) if isinstance(r.get(k), list) else []:
                    if isinstance(a, str):
                        known[a].add(path.stem)
    return known


# ---------------------------------------------------------------------------------------------
# Sources


def is_rust_test_path(rel: str) -> bool:
    parts = rel.split("/")
    return (
        "tests" in parts
        or "benches" in parts
        or any(p.endswith("_tests") for p in parts)
        or parts[-1] in ("tests.rs", "fixtures.rs")
        or parts[-1].endswith("_tests.rs")
    )


TEST_MOD = re.compile(r"^#\[cfg\(test\)\]\s*\n(?:#\[[^\n]*\]\s*\n)*\s*(?:pub(?:\([a-z]+\))?\s+)?mod\s", re.M)


def production_lines(text: str):
    """The lines before the file's test module.

    Cut at the first `#[cfg(test)]` that introduces a `mod`, not at the first `#[cfg(test)]`:
    a test-only *function* in the middle of a file (`combat.rs:3793`) is followed by more
    production code. Line numbers stay those of the file.
    """
    if re.search(r"^#!\[cfg\(test\)\]", text, re.M):
        return []  # the whole file is a test module (`wiring.rs`)
    m = TEST_MOD.search(text)
    return (text[: m.start()] if m else text).split("\n")


def rust_files():
    for crate_dir in sorted((ROOT / "crates").iterdir()):
        if not crate_dir.is_dir():
            continue
        crate = crate_dir.name
        for sub in ("src", "examples"):
            base = crate_dir / sub
            if not base.exists():
                continue
            for path in sorted(base.rglob("*.rs")):
                rel = str(path.relative_to(ROOT))
                if is_rust_test_path(rel):
                    continue
                if sub == "examples":
                    component = "tool"
                else:
                    component = COMPONENT.get(crate, "tool")
                if rel.endswith("examples/card_registry_dump.rs"):
                    continue
                text = path.read_text(encoding="utf-8", errors="replace")
                yield rel, crate, component, production_lines(text)


def ts_files():
    base = ROOT / "web" / "src"
    for path in sorted(base.rglob("*")):
        if path.suffix not in (".ts", ".tsx"):
            continue
        rel = str(path.relative_to(ROOT))
        name = path.name
        if ".test." in name or "/test/" in rel or "/fixtures/" in rel or name.endswith(".d.ts"):
            continue
        yield rel, path.read_text(encoding="utf-8", errors="replace").split("\n")


def is_comment(line: str) -> bool:
    s = line.lstrip()
    return s.startswith("//") or s.startswith("*") or s.startswith("/*")


# ---------------------------------------------------------------------------------------------
# Field readers (Rust)


def field_sites(cards):
    """(category, field) -> list of sites; plus unattributed sites per field."""
    schema = defaultdict(set)  # field -> categories carrying it
    for cat, recs in cards.items():
        for _id, r in recs:
            for f in r:
                schema[f].add(cat)
    sites = defaultdict(list)
    unattributed = defaultdict(list)
    for rel, crate, component, lines in rust_files():
        fn_start = 0
        for i, line in enumerate(lines):
            if FN_START.match(line):
                fn_start = i
            if is_comment(line):
                continue
            for m in ACCESSOR.finditer(line):
                field = m.group("a") or m.group("b") or m.group("c")
                if field not in schema:
                    continue
                site = {
                    "file": rel,
                    "line": i + 1,
                    "component": component,
                    "code": line.strip()[:160],
                }
                # Nearest ContentType mention: same line, then upward to the enclosing fn, then
                # down a few lines (a closure handed to `.filter` before the category is named).
                variants = CT.findall(line)
                j = i - 1
                while not variants and j >= fn_start:
                    variants = CT.findall(lines[j])
                    j -= 1
                k = i + 1
                while not variants and k < min(len(lines), i + 4):
                    variants = CT.findall(lines[k])
                    k += 1
                cats = [VARIANT_TO_CAT[v] for v in variants if v in VARIANT_TO_CAT]
                others = [v for v in variants if v in OTHER_VARIANTS]
                if cats:
                    cat = cats[-1] if len(set(cats)) > 1 else cats[0]
                    if field in {f for _, r in cards[cat] for f in r}:
                        sites[(cat, field)].append(site | {"via": "ContentType"})
                        continue
                if others and not cats:
                    continue  # a non-card category's field of the same name
                owners = schema[field]
                if len(owners) == 1:
                    cat = next(iter(owners))
                    sites[(cat, field)].append(site | {"via": "unique field"})
                else:
                    unattributed[field].append(site)
    return sites, unattributed


# ---------------------------------------------------------------------------------------------
# Web readers: the web reads the host's catalog (`ti4-host/src/view/records.rs`), not the corpus.


HOST_RECORDS = ROOT / "crates" / "ti4-host" / "src" / "view" / "records.rs"


def host_catalog_map():
    """category -> {raw field -> host catalog field}, parsed from `content_records`."""
    text = HOST_RECORDS.read_text(encoding="utf-8")
    body = text.split("pub fn content_records")[1].split("#[cfg(test)]")[0]
    mapping = defaultdict(dict)
    current = None
    for line in body.split("\n"):
        m = CT.search(line)
        if m and m.group(1) in VARIANT_TO_CAT:
            current = VARIANT_TO_CAT[m.group(1)]
        elif m:
            current = None
        if m and m.group(1) in ("SecretObjectives", "PublicObjectives"):
            for f, h in (("name", "name"), ("phase", "phase"), ("points", "points"), ("text", "text")):
                mapping[VARIANT_TO_CAT[m.group(1)]][f] = h
            current = None
            continue
        if current is None:
            continue
        fm = re.match(r"\s*(\w+):\s*(.*)", line)
        if not fm:
            continue
        host_field, expr = fm.groups()
        for raw in re.findall(r'"([A-Za-z0-9_]+)"', expr):
            mapping[current][raw] = host_field
    # The attachment text is looked up in other categories (`attachment_text`).
    return mapping


def web_reads(host_map):
    """host catalog field -> number of non-test web sites reading it (snake or camel case)."""
    names = {h for m in host_map.values() for h in m.values()}
    counts = Counter()
    files = defaultdict(set)
    for rel, lines in ts_files():
        for line in lines:
            if is_comment(line):
                continue
            for h in names:
                camel = re.sub(r"_([a-z])", lambda x: x.group(1).upper(), h)
                if re.search(r"\.(%s|%s)\b" % (re.escape(h), re.escape(camel)), line):
                    counts[h] += 1
                    files[h].add(rel)
    return counts, files


# ---------------------------------------------------------------------------------------------
# Id references


def id_references(cards):
    """id -> list of (category, file, line, component) for production string literals naming it."""
    index = defaultdict(set)  # literal -> categories
    for cat, recs in cards.items():
        for cid, r in recs:
            index[cid].add(cat)
            if cat == "action_cards" and r.get("automationID"):
                index[r["automationID"]].add(cat + ":automationID")
            if cat == "promissory_notes" and cid.startswith("<color>"):
                # The engine names the generic notes by their bare alias (`promissory::GENERIC`:
                # "cf", "ps", "ta", "an") and maps back with the `<color>_` prefix.
                index[cid.replace("<color>_", "")].add(cat)
    refs = defaultdict(list)
    literals = Counter()
    literal_sites = defaultdict(list)
    for rel, crate, component, lines in rust_files():
        for i, line in enumerate(lines):
            if is_comment(line):
                continue
            for m in RUST_STR.finditer(line):
                lit = m.group(1)
                if lit is None:
                    continue
                if lit in index:
                    for cat in index[lit]:
                        refs[(cat, lit)].append((rel, i + 1, component))
                elif re.fullmatch(r"[a-z][a-z0-9_\-]{2,}", lit):
                    literals[lit] += 1
                    if len(literal_sites[lit]) < 3:
                        literal_sites[lit].append(f"{rel}:{i + 1}")
    for rel, lines in ts_files():
        for i, line in enumerate(lines):
            if is_comment(line):
                continue
            for m in TS_STR.finditer(line):
                lit = m.group(1) if m.group(1) is not None else m.group(2)
                if lit in index:
                    for cat in index[lit]:
                        refs[(cat, lit)].append((rel, i + 1, "web"))
    return refs, literals, literal_sites


# Literal shapes that look like card ids of a category, for the "named in code, missing in
# content" direction. Only literals inside a recognisably card-handling context count: a match
# arm or comparison in the module that owns the category.
OWNER_MODULES = {
    "action_cards": ["crates/ti4-engine/src/action_cards.rs", "crates/ti4-engine/src/reactions.rs"],
    "agendas": ["crates/ti4-engine/src/agenda_effects.rs", "crates/ti4-engine/src/laws.rs", "crates/ti4-engine/src/vote.rs"],
    "relics": ["crates/ti4-engine/src/relics.rs"],
    "secret_objectives": ["crates/ti4-engine/src/secrets.rs"],
    "public_objectives": ["crates/ti4-engine/src/objectives.rs"],
    "explores": ["crates/ti4-engine/src/exploration.rs"],
    "leaders": ["crates/ti4-engine/src/leaders.rs"],
    "breakthroughs": ["crates/ti4-engine/src/breakthroughs.rs"],
    "technologies": ["crates/ti4-engine/src/technology.rs", "crates/ti4-engine/src/faction_techs.rs"],
    "promissory_notes": ["crates/ti4-engine/src/promissory.rs"],
    "abilities": ["crates/ti4-engine/src/faction_abilities.rs"],
    "strategy_cards": ["crates/ti4-engine/src/strategy_cards.rs"],
}
ARM = re.compile(r'^\s*(?:"[a-z0-9_\-]+"\s*(?:\|\s*)?)+(?:=>|\|)|^\s*"[a-z0-9_\-]+",\s*$')


ENCLOSING = re.compile(
    r"^\s*(?:pub(?:\([a-z]+\))?\s+)?(?:const\s+)?fn\s+(\w+)"
    r"|^\s*(?:pub(?:\([a-z]+\))?\s+)?(?:const|static)\s+([A-Z_][A-Z0-9_]*)"
)
TYPED_ID = re.compile(
    r"\b(ActionCardId|LeaderId|TechnologyId|SecretObjectiveId|ObjectiveId|StrategyCardId|"
    r"BreakthroughId|RelicId)::new\(\s*\"([^\"]+)\"\s*\)"
)
TYPED_CAT = {
    "ActionCardId": ["action_cards"],
    "LeaderId": ["leaders"],
    "TechnologyId": ["technologies"],
    "SecretObjectiveId": ["secret_objectives"],
    "ObjectiveId": ["public_objectives", "secret_objectives", "agendas"],
    "StrategyCardId": ["strategy_cards"],
    "BreakthroughId": ["breakthroughs"],
    "RelicId": ["relics"],
}


def dangling_ids(cards, known_all):
    """Card-shaped literals that name no record.

    Two shapes are checked, in engine production code:
      * a literal handed to a typed card id constructor (`LeaderId::new("...")`), which must be
        an id of that id's category (any category for `ObjectiveId`);
      * a literal in a match arm or list line of a *dispatch function* of the category's owner
        module: a function or const in which at least one arm/list line names a real card id of
        that category (`effect_for`, `registered_aliases`, `resolve_with`, ...). Other match arms
        in those modules are decision kinds and option ids, not card ids, and are not checked.
    """
    found = []
    ids_of = {cat: {cid for cid, _ in recs} for cat, recs in cards.items()}
    for cat, files in OWNER_MODULES.items():
        ids = ids_of[cat]
        for rel in files:
            path = ROOT / rel
            if not path.exists():
                continue
            lines = production_lines(path.read_text(encoding="utf-8"))
            owner = []
            fn = "?"
            for line in lines:
                m = ENCLOSING.match(line)
                if m:
                    fn = m.group(1) or m.group(2)
                owner.append(fn)
            dispatch = set()
            for i, line in enumerate(lines):
                if not is_comment(line) and ARM.search(line):
                    if any(lit in ids for lit in re.findall(r'"([a-z0-9_\-]+)"', line)):
                        dispatch.add(owner[i])
            for i, line in enumerate(lines):
                if is_comment(line) or owner[i] not in dispatch or not ARM.search(line):
                    continue
                for lit in re.findall(r'"([a-z0-9_\-]+)"', line):
                    if lit in ids or lit in known_all:
                        continue
                    found.append({"category": cat, "literal": lit, "site": f"{rel}:{i + 1}",
                                  "shape": f"arm in {owner[i]}"})
    for rel, crate, component, lines in rust_files():
        if component != "engine":
            continue
        for i, line in enumerate(lines):
            if is_comment(line):
                continue
            for m in TYPED_ID.finditer(line):
                kind, lit = m.groups()
                cats = TYPED_CAT[kind]
                if any(lit in ids_of[c] for c in cats):
                    continue
                if "{" in lit or ":" in lit or "|" in lit:
                    continue  # a composed id (`leader:...`, `yssarilagent|...`)
                found.append({"category": "/".join(cats), "literal": lit, "site": f"{rel}:{i + 1}",
                              "shape": f"{kind}::new",
                              "other_category": sorted(known_all.get(lit, []))})
    return found


PARTIAL = re.compile(
    r"Effect::Deferred|Deferred \{|Ability::Unresolved|Effect::Unresolved|\bnot modell?ed\b|"
    r"\bunmodell?ed\b|\bnot (?:yet )?(?:implemented|enforced|delivered)\b|\bcannot yet\b|"
    r"\bapproximat|\bsimplif(?:y|ied|ication)\b|\bTODO\b"
)
ARM_TO_FN = re.compile(r'((?:"[a-z0-9_\-]+"\s*\|?\s*)+)=>\s*(?:Some\()?([a-z_][a-z0-9_]*)\b')


def partial_markers(cards):
    """Card id -> sites of a marker of partial implementation inside its handler.

    Measured by pattern: a `Deferred`/`Unresolved` return, or a comment saying "not modelled",
    "not implemented", "simplified", "approximation", "TODO", inside the function an arm maps the
    card to (`"mb1" | ... => Some(morale_boost)`), or after the card's own arm inside a dispatch
    function (`"articles_war" => { ... }`). A marker is evidence to read, not a verdict: some of
    them describe a fallback that only applies without a map.
    """
    out = defaultdict(list)
    ids_of = {cat: {cid for cid, _ in recs} for cat, recs in cards.items()}
    for cat, files in OWNER_MODULES.items():
        ids = ids_of[cat]
        for rel in files:
            path = ROOT / rel
            if not path.exists():
                continue
            lines = production_lines(path.read_text(encoding="utf-8"))
            fn_of_line = []
            fn = "?"
            for line in lines:
                m = ENCLOSING.match(line)
                if m:
                    fn = m.group(1) or m.group(2)
                fn_of_line.append(fn)
            # A doc comment belongs to the item below it.
            nxt = "?"
            for i in range(len(lines) - 1, -1, -1):
                m = ENCLOSING.match(lines[i])
                if m:
                    nxt = m.group(1) or m.group(2)
                elif lines[i].lstrip().startswith("///"):
                    fn_of_line[i] = nxt
                elif lines[i].strip() and not lines[i].lstrip().startswith("#["):
                    nxt = fn_of_line[i]
            handler = defaultdict(set)  # fn name -> card ids routed to it
            for line in lines:
                for m in ARM_TO_FN.finditer(line):
                    lits = set(re.findall(r'"([a-z0-9_\-]+)"', m.group(1))) & ids
                    if lits:
                        handler[m.group(2)] |= lits
            last_arm = {}
            for i, line in enumerate(lines):
                lits = set(re.findall(r'"([a-z0-9_\-]+)"', line)) & ids
                if lits and ARM.search(line):
                    last_arm[fn_of_line[i]] = (lits, i)
                elif re.match(r"^\s*_\s*=>", line):
                    last_arm.pop(fn_of_line[i], None)  # the fallback arm belongs to no card
                if not PARTIAL.search(line):
                    continue
                owners = set(handler.get(fn_of_line[i], set()))
                if not owners and fn_of_line[i] in last_arm:
                    owners = last_arm[fn_of_line[i]][0]
                for cid in owners:
                    out[(cat, cid)].append(f"{rel}:{i + 1}")
    return out


def name_references(cards):
    """(cat, id) -> engine/bot/web production sites whose string literal is the card's printed
    *name* (not its id): behaviour or a lookup keyed on the name."""
    by_name = defaultdict(set)
    for cat, recs in cards.items():
        for cid, r in recs:
            name = r.get("name")
            if isinstance(name, str) and len(name) >= 4 and name != cid and re.search(r"[A-Z ]", name):
                by_name[name].add((cat, cid))
    out = defaultdict(list)
    for rel, crate, component, lines in rust_files():
        if component not in ("engine", "bot", "host"):
            continue
        for i, line in enumerate(lines):
            if is_comment(line):
                continue
            for m in RUST_STR.finditer(line):
                lit = m.group(1)
                if lit in by_name:
                    for key in by_name[lit]:
                        out[key].append((f"{rel}:{i + 1}", component))
    for rel, lines in ts_files():
        for i, line in enumerate(lines):
            if is_comment(line):
                continue
            for m in TS_STR.finditer(line):
                lit = m.group(1) if m.group(1) is not None else m.group(2)
                if lit in by_name:
                    for key in by_name[lit]:
                        out[key].append((f"{rel}:{i + 1}", "web"))
            # object keys written bare: `Blitz: "own"`
            for key in re.findall(r'^\s*([A-Z][A-Za-z]+):\s', line):
                if key in by_name:
                    for k in by_name[key]:
                        out[k].append((f"{rel}:{i + 1}", "web"))
    return out


def objective_thresholds(cards):
    """Public and secret objectives: the numbers printed in `text` against the numbers in the
    code arm for the alias (`objectives.rs`, `secrets.rs`). A printed number missing from the arm
    is a candidate drift (or a number the arm takes from elsewhere); read the arm."""
    results = {}
    words = {"one": 1, "two": 2, "three": 3, "four": 4, "five": 5, "six": 6, "seven": 7,
             "eight": 8, "nine": 9, "ten": 10, "eleven": 11, "twelve": 12}
    for cat, rel in (("public_objectives", "crates/ti4-engine/src/objectives.rs"),
                     ("secret_objectives", "crates/ti4-engine/src/secrets.rs")):
        lines = production_lines((ROOT / rel).read_text(encoding="utf-8"))
        for cid, r in cards[cat]:
            text = r.get("text", "")
            printed = {int(n) for n in re.findall(r"\b(\d+)\b", text)}
            printed |= {v for w, v in words.items() if re.search(r"\b%s\b" % w, text, re.I)}
            printed.discard(1)  # "1 of your ..." is grammar more often than a threshold
            arms = []
            for i, line in enumerate(lines):
                if re.search(r'"%s"\s*(?:\||=>)' % re.escape(cid), line):
                    block = []
                    for j in range(i, min(len(lines), i + 8)):
                        block.append(lines[j])
                        if j > i and re.search(r'^\s*"[a-z0-9_]+"\s*(?:\||=>)', lines[j]):
                            break
                    arms.append((i + 1, " ".join(block)))
            code_numbers = set()
            for _, block in arms:
                code_numbers |= {int(n) for n in re.findall(r"(?<![\w.])(\d+)(?![\w.])", block)}
            missing = sorted(printed - code_numbers) if arms else []
            results[(cat, cid)] = {
                "printed": sorted(printed),
                "arm_sites": [f"{rel}:{n}" for n, _ in arms],
                "printed_not_in_arm": missing,
            }
    return results


# ---------------------------------------------------------------------------------------------
# Curation


def load_curation():
    if not CURATION.exists():
        return {"fields": {}, "issues": [], "implementation": {}}
    return json.loads(CURATION.read_text(encoding="utf-8"))


def check_evidence(curation):
    """Every curated evidence `file:line` must exist and (when a pattern is given) still match."""
    problems = []
    def check(ev, where):
        if not isinstance(ev, str) or ":" not in ev:
            return
        spec, _, pattern = ev.partition(" ~ ")
        file, _, line = spec.partition(":")
        path = ROOT / file
        if not path.exists():
            problems.append(f"{where}: {file} does not exist")
            return
        if pattern:
            lines = path.read_text(encoding="utf-8", errors="replace").split("\n")
            try:
                n = int(line.split("-")[0])
            except ValueError:
                return
            window = "\n".join(lines[max(0, n - 4): n + 3])
            if pattern not in window:
                problems.append(f"{where}: {spec} no longer contains {pattern!r}")
    for key, entry in curation.get("fields", {}).items():
        for ev in entry.get("evidence", []):
            check(ev, key)
    for issue in curation.get("issues", []):
        for ev in issue.get("evidence", []):
            check(ev, issue.get("id", "?"))
    return problems


# ---------------------------------------------------------------------------------------------
# Implementation status per card


def _sheet_context():
    units = json.loads((CONTENT / "units.json").read_text(encoding="utf-8-sig"))
    decks = {d["alias"]: d for d in json.loads((CONTENT / "decks.json").read_text(encoding="utf-8-sig"))}
    techs = json.loads((CONTENT / "technologies.json").read_text(encoding="utf-8-sig"))
    factions = json.loads((CONTENT / "factions.json").read_text(encoding="utf-8-sig"))
    upgrade = {u["requiredTechId"] for u in units if u.get("requiredTechId")}
    active = set(decks["techs_pok_c4"]["cardIDs"]) | {t["alias"] for t in techs if t.get("source") == "thunders_edge"}
    sheet = {x for f in factions for k in ("leaders", "promissoryNotes") for x in f.get(k, [])}
    return upgrade, active, sheet


UPGRADE_TECHS, ACTIVE_TECHS, SHEET_IDS = _sheet_context()


def implementation_status(cards, dump, refs, names):
    """card (cat, id) -> (status, detail). From the engine's registries when the dump is there."""
    status = {}
    assets = {}
    if dump:
        for faction, rows in dump.get("faction_assets", {}).items():
            for row in rows:
                assets[(row["kind"], row["id"])] = (row["implemented"], faction)
    kind_of = {
        "abilities": "Ability",
        "technologies": "Technology",
        "promissory_notes": "Promissory",
        "leaders": "Leader",
        "breakthroughs": "Breakthrough",
    }
    for cat, recs in cards.items():
        for cid, r in recs:
            engine_refs = [x for x in refs.get((cat, cid), []) if x[2] == "engine"]
            if cat == "action_cards" and r.get("automationID") and r["automationID"] != cid:
                engine_refs += [
                    x for x in refs.get((cat + ":automationID", r["automationID"]), []) if x[2] == "engine"
                ]
            s, detail = None, ""
            if dump:
                if cat == "action_cards":
                    w = dump["action_cards"]["windows"].get(cid, {})
                    eff = cid in dump["action_cards"]["registered"]
                    reactive = w.get("printed_window", "Action") != "Action"
                    if eff and (not reactive or w.get("bound_event")):
                        s = "registered"
                    elif eff:
                        s = "partial"
                        detail = "effect registered, printed window not bound to an event"
                    elif w.get("bound_event"):
                        s = "partial"
                        detail = "window bound, no effect registered"
                    else:
                        s = "none"
                    if w.get("bound_event"):
                        detail = (detail + "; " if detail else "") + f"window -> {w['bound_event']}"
                elif cat == "agendas":
                    reg = cid in dump["agendas"]["registered"]
                    s = "registered" if reg else "none"
                    if cid in dump["agendas"]["laws_unenforced"]:
                        s, detail = "partial", "law not enforced once in play (laws::unimplemented)"
                elif cat == "relics":
                    s = "registered" if cid in dump["relics"]["registered"] else "none"
                elif cat == "secret_objectives":
                    s = "registered" if cid in dump["secret_objectives"]["registered"] else "none"
                elif cat == "public_objectives":
                    pc = dump["public_objectives"]["per_card"].get(cid, {})
                    s = "registered" if (pc.get("requirement") or pc.get("cost")) else "none"
                    detail = "price (cost_of)" if pc.get("cost") else ("predicate (requirement_for)" if pc.get("requirement") else "")
                elif cat == "explores":
                    s = "none" if cid in dump["explores"]["unimplemented"] else "registered"
                elif cat in kind_of:
                    a = assets.get((kind_of[cat], cid))
                    if a is not None:
                        s = "registered" if a[0] else "none"
                        detail = f"faction asset of {a[1]}"
                    elif cat == "abilities":
                        s = "none" if cid in dump["abilities"]["unimplemented"] else "registered"
                    elif cat == "leaders":
                        reg = cid in dump["leaders"]["registered"] or cid in dump["leaders"]["modifiers"]
                        s = "registered" if reg else ("none" if cid in dump["leaders"]["unimplemented_all_factions"] else None)
                    elif cat == "breakthroughs":
                        reg = cid in dump["breakthroughs"]["registered"] or cid in dump["breakthroughs"]["module_claimed"]
                        s = "registered" if reg else "none"
            if cat == "promissory_notes" and cid.startswith("<color>_"):
                engine_refs += [x for x in refs.get((cat, cid.replace("<color>_", "")), []) if x[2] == "engine"]
                if s is None or s == "unreferenced":
                    s = "referenced" if engine_refs or cid == "<color>_sftt" else s
                    detail = "generic note: named by its bare alias (promissory::GENERIC) or 'support:<faction>'"
            if s is None:
                # No registry: literal references in engine production code are the signal,
                # refined by the data-driven paths that need no literal at all.
                if engine_refs:
                    s = "referenced"
                elif cat == "technologies" and cid in UPGRADE_TECHS:
                    s, detail = "data-driven", "unit upgrade: units.json requiredTechId names it"
                elif cat == "technologies" and cid not in ACTIVE_TECHS:
                    s, detail = "not-dealt", "superseded printing, not in decks.json techs_pok_c4 (technology::active_aliases)"
                elif cat == "attachments" and any(k in r for k in ("resourcesModifier", "influenceModifier", "techSpeciality", "planetTypes", "spaceCannonHitsOn")):
                    s, detail = "data-driven", "planet modifiers read from the record"
                elif cat == "strategy_cards" and names.get((cat, cid)):
                    s, detail = "name-keyed", "dispatched on the printed name (strategy_cards::card_name)"
                else:
                    s = "unreferenced"
                    detail = detail or "no engine registry for this category and no literal id reference"
            if s == "unreferenced" and cat in ("leaders", "promissory_notes") and cid not in SHEET_IDS:
                s, detail = "not-dealt", "not on any faction sheet (factions.json leaders / promissoryNotes)"
            status[(cat, cid)] = (s, detail, len(engine_refs))
    return status


# ---------------------------------------------------------------------------------------------


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--check", action="store_true", help="print a summary, write nothing")
    ap.add_argument("--dump", default=str(OUT / "registry_dump.json"))
    ap.add_argument("--json", metavar="PATH",
                    help="also write everything (fields, cards, issues, summaries) as one JSON file, "
                         "the input of the artifact page (build_page.py)")
    args = ap.parse_args()

    cards = load_content()
    known_all = all_content_ids()
    curation = load_curation()
    dump = None
    if Path(args.dump).exists():
        dump = json.loads(Path(args.dump).read_text())

    sites, unattributed = field_sites(cards)
    # Sites the pattern attributes wrongly (an event payload key that shares a field name, a
    # test inside a production file), each with its reason in the curation file.
    excluded = {e["site"] for e in curation.get("exclude_sites", [])}
    for key in list(sites):
        sites[key] = [x for x in sites[key] if f"{x['file']}:{x['line']}" not in excluded]
    for key, extra in curation.get("extra_sites", {}).items():
        cat, _, f = key.partition(".")
        for spec in extra:
            file, _, line = spec.partition(":")
            comp = COMPONENT.get(file.split("/")[1], "tool") if file.startswith("crates/") else "web"
            sites[(cat, f)].append({"file": file, "line": int(line), "component": comp,
                                    "code": "", "via": "curated"})
    host_map = host_catalog_map()
    web_count, web_files = web_reads(host_map)
    refs, _literals, _lsites = id_references(cards)
    dangling = dangling_ids(cards, known_all)
    reviewed = set(curation.get("non_id_literals", {}).get("literals", []))
    for d in dangling:
        d["verdict"] = "decision kind / option id (reviewed)" if d["literal"] in reviewed else "UNREVIEWED: no record has this id"
    names = name_references(cards)
    status = implementation_status(cards, dump, refs, names)
    partial = partial_markers(cards)
    thresholds = objective_thresholds(cards)
    evidence_problems = check_evidence(curation)

    # ---- fields.csv
    field_rows = []
    for cat, recs in cards.items():
        present = Counter()
        size = Counter()
        for _cid, r in recs:
            for f, v in r.items():
                present[f] += 1
                # bytes of `"field":value,` in the minified file
                size[f] += len(json.dumps(f)) + 1 + minified_size(v) + 1
        for f in sorted(present, key=lambda x: -size[x]):
            s = sites.get((cat, f), [])
            by = Counter(x["component"] for x in s)
            host_field = host_map.get(cat, {}).get(f)
            web_n = web_count.get(host_field, 0) if host_field else 0
            fields_cur = curation.get("fields", {})
            cur = fields_cur.get(f"{cat}.{f}") or fields_cur.get(f"*.{f}") or {}
            field_rows.append({
                "category": cat,
                "field": f,
                "records": present[f],
                "of": len(recs),
                "bytes": size[f],
                "engine": by.get("engine", 0),
                "content": by.get("content", 0),
                "host": by.get("host", 0),
                "bot": by.get("bot", 0),
                "wasm": by.get("wasm", 0),
                "server": by.get("server", 0) + by.get("selfplay", 0),
                "tools": by.get("tool", 0),
                "host_catalog_field": host_field or "",
                "web_sites": web_n,
                "classification": cur.get("class", "unclassified"),
                "use": cur.get("use", ""),
                "evidence": "; ".join(cur.get("evidence", [])),
                "sites": [f"{x['file']}:{x['line']}" for x in s],
            })

    # Ids spelled the same in two categories: a typed id per category must stay distinct.
    owners = defaultdict(set)
    for cat, recs in cards.items():
        for cid, _ in recs:
            owners[cid].add(cat)
            if cid.startswith("<color>_"):
                owners[cid.replace("<color>_", "")].add(cat)
    collisions = {
        cid: sorted(cats | {c for c in known_all.get(cid, set()) if c not in
                            {CATEGORIES[x][0][:-5] for x in cats}})
        for cid, cats in owners.items()
        if len(cats) > 1 or (known_all.get(cid, set()) - {CATEGORIES[x][0][:-5] for x in cats})
    }
    # ---- cards.csv
    issues_by_card = defaultdict(list)
    for issue in curation.get("issues", []):
        for c in issue.get("cards", []):
            issues_by_card[c].append(issue["id"])
    field_class = {(r["category"], r["field"]): r["classification"] for r in field_rows}
    card_rows = []
    impl_notes = curation.get("implementation", {})
    for cat, recs in cards.items():
        for cid, r in recs:
            s, detail, n_engine = status[(cat, cid)]
            all_refs = refs.get((cat, cid), [])
            comp = Counter(x[2] for x in all_refs)
            key = f"{cat}:{cid}"
            card_issues = issues_by_card.get(key, []) + [
                i for i in issues_by_card.get(f"{cat}:*", []) if i not in issues_by_card.get(key, [])
            ]
            note = impl_notes.get(key) or impl_notes.get(f"{cat}:*") or {}
            card_rows.append({
                "category": cat,
                "id": cid,
                "name": r.get("name") or r.get("title") or "",
                "source": r.get("source", ""),
                "fields": " ".join(sorted(r)),
                "status": s,
                "status_detail": detail,
                "mechanism": note.get("mechanism", ""),
                # Fields of this record that the rules read (classes gameplay / gameplay-text):
                # what a typed gameplay struct for this card would still have to carry.
                "gameplay_fields": " ".join(sorted(
                    f for f in r if field_class.get((cat, f)) in ("gameplay", "gameplay-text")
                )),
                "text_parsed_fields": " ".join(sorted(
                    f for f in r if field_class.get((cat, f)) == "gameplay-text"
                )),
                "data_fields": " ".join(note.get("data_fields", [])),
                "refs_engine": comp.get("engine", 0),
                "refs_host": comp.get("host", 0),
                "refs_bot": comp.get("bot", 0),
                "refs_web": comp.get("web", 0),
                "refs_other": sum(v for k, v in comp.items() if k not in ("engine", "host", "bot", "web")),
                "first_engine_ref": next((f"{f}:{l}" for f, l, c in all_refs if c == "engine"), ""),
                "partial_markers": " ".join(partial.get((cat, cid), [])[:6]),
                "name_keyed_sites": " ".join(f"{site}({c})" for site, c in names.get((cat, cid), [])[:6]),
                "threshold_check": (
                    "" if (cat, cid) not in thresholds
                    else ("no arm found" if not thresholds[(cat, cid)]["arm_sites"]
                          else ("printed " + ",".join(map(str, thresholds[(cat, cid)]["printed_not_in_arm"]))
                                + " not in arm " + thresholds[(cat, cid)]["arm_sites"][0])
                          if thresholds[(cat, cid)]["printed_not_in_arm"] else "ok")
                ),
                "id_collides_with": " ".join(c for c in collisions.get(cid, []) if c != cat),
                "issues": " ".join(sorted(card_issues)),
            })

    summary = Counter((r["category"], r["status"]) for r in card_rows)
    if args.check:
        for (cat, st), n in sorted(summary.items()):
            print(f"{cat:20} {st:14} {n}")
        print("dangling literals:", len(dangling), "unreviewed:", [d for d in dangling if d["verdict"].startswith("UNREVIEWED")])
        print("unattributed fields:", {k: len(v) for k, v in unattributed.items()})
        for p in evidence_problems:
            print("EVIDENCE:", p)
        return 1 if evidence_problems else 0

    OUT.mkdir(parents=True, exist_ok=True)
    with open(OUT / "fields.csv", "w", newline="") as fh:
        cols = [c for c in field_rows[0] if c != "sites"] + ["sites"]
        w = csv.DictWriter(fh, fieldnames=cols)
        w.writeheader()
        for row in field_rows:
            w.writerow(row | {"sites": " ".join(row["sites"])})
    # mechanism / data_fields are per category (curation "implementation"), not per card: kept
    # out of the CSV, where they would repeat 800 times.
    csv_cols = [k for k in card_rows[0] if k not in ("mechanism", "data_fields")]
    with open(OUT / "cards.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=csv_cols, extrasaction="ignore")
        w.writeheader()
        w.writerows(card_rows)

    content_only = {
        cat: sorted(cid for cid, _ in recs if not refs.get((cat, cid)))
        for cat, recs in cards.items()
    }
    id_refs = {
        "named_in_code_not_in_content": dangling,
        "in_content_never_named_in_production_code": content_only,
        "id_collisions_across_categories": collisions,
        "unattributed_field_sites": {k: [f"{s['file']}:{s['line']}" for s in v] for k, v in unattributed.items()},
    }
    (OUT / "id_refs.json").write_text(json.dumps(id_refs, indent=1) + "\n")

    per_category = {}
    for cat in CATEGORIES:
        rows = [r for r in card_rows if r["category"] == cat]
        per_category[cat] = {
            "cards": len(rows),
            "status": dict(Counter(r["status"] for r in rows)),
            "pure_id": sum(1 for r in rows if not r["gameplay_fields"]),
            "needs_data": sum(1 for r in rows if r["gameplay_fields"]),
            "text_parsed": sum(1 for r in rows if r["text_parsed_fields"]),
            "gameplay_fields": sorted({f for r in rows for f in r["gameplay_fields"].split()}),
            "bytes": dict(Counter({row["classification"]: 0 for row in field_rows if row["category"] == cat})
                          | Counter()),
        }
        b = Counter()
        for row in field_rows:
            if row["category"] == cat:
                b[row["classification"]] += row["bytes"]
        per_category[cat]["bytes"] = dict(b)
    bytes_by_class = Counter()
    bytes_total = 0
    for row in field_rows:
        bytes_by_class[row["classification"]] += row["bytes"]
        bytes_total += row["bytes"]
    audit = {
        "generated_by": "tools/card_audit/card_audit.py",
        "registry_dump": bool(dump),
        "fields": field_rows,
        "cards": card_rows,
        "issues": curation.get("issues", []),
        "id_refs": id_refs,
        "bytes_by_class": dict(bytes_by_class),
        "per_category": per_category,
        "bytes_total": bytes_total,
        "evidence_problems": evidence_problems,
    }
    audit["implementation"] = curation.get("implementation", {})
    if args.json:
        Path(args.json).write_text(json.dumps(audit, indent=None, separators=(",", ":")) + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}: {len(field_rows)} fields, {len(card_rows)} cards, "
          f"{sum(1 for d in dangling if d['verdict'].startswith('UNREVIEWED'))} unreviewed dangling literals "
          f"(of {len(dangling)} found), {len(evidence_problems)} evidence problems")
    for p in evidence_problems:
        print("EVIDENCE:", p, file=sys.stderr)
    return 1 if evidence_problems else 0


if __name__ == "__main__":
    sys.exit(main())
