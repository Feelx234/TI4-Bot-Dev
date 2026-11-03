#!/usr/bin/env python3
"""Silent-gap diff (f): content-catalog ids vs the core's rules and ports.

    python3 -I tools/engine_core/silent_gaps_def.py [--tsv FILE] [--summary]

For every content kind (action cards, promissory notes, relics, technologies, leaders, agendas, public / secret objectives,
breakthroughs, explores, faction abilities) the catalog id (crates/ti4-content/content/*.json) is mapped to its core enum variant
(crates/ti4-engine-core/names/<kind>.toml) and the variant is searched in the non-generated, non-test source of the core crates
(`::Variant` mentions in ti4-engine-core/src outside generated/, and in ti4-engine-core-factions/src). A variant that appears nowhere is
"in the enum, no port": the content compiles, nothing in the core reads it. Action cards are exact instead: `Fx::of`, `Simple::of` and the
`REGISTERED` list of the audit `every_action_card_is_ported_or_a_documented_bail`. Static: a variant read through data (a generated
table field, a generic rule) shows as no mention, a mention in a doc comment is stripped (comments are blanked), and a mention is not a proof
that the whole text of the card is ported (that is the lockstep tests' job). Each row also says whether the string engine has a handler
(registered_aliases() of the string objectives / secrets / action cards) when the engine lists one.
"""
import glob
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from lent_sites import mask  # noqa: E402

REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
CORE = os.path.join(REPO, "crates/ti4-engine-core")
CONTENT = os.path.join(REPO, "crates/ti4-content/content")
DROPPED = {"bastion", "crimson", "keleres", "keleresm", "keleresx", "keleresa", "deepwrought", "firmament", "obsidian", "ralnel"}

KINDS = [
    # (kind, json file, id field, names toml, generated file, enum, faction field)
    ("action_card", "action_cards", "alias", "action_cards", "action_cards", "ActionCard", None),
    ("promissory", "promissory_notes", "alias", "promissory_notes", "promissory_notes", "PromissoryNote", "faction"),
    ("relic", "relics", "alias", "relics", "relics", "Relic", None),
    ("technology", "technologies", "alias", "technologies", "technologies", "Technology", "faction"),
    ("leader", "leaders", "id", "leaders", "leaders", "Leader", "faction"),
    ("agenda", "agendas", "alias", "agendas", "agendas", "Agenda", None),
    ("public_objective", "public_objectives", "alias", "public_objectives", "public_objectives", "PublicObjective", None),
    ("secret_objective", "secret_objectives", "alias", "secret_objectives", "secret_objectives", "SecretObjective", None),
    ("breakthrough", "breakthroughs", "alias", "breakthroughs", "breakthroughs", "Breakthrough", "faction"),
    ("explore", "explores", "id", "explores", "explores", "Explore", None),
    ("ability", "abilities", "id", "abilities", "abilities", "Ability", "faction"),
]


def prod(path):
    t = open(path).read()
    i = t.find("#[cfg(test)]")
    t = t if i < 0 else t[:i]
    return mask(t)


def raw_texts():
    """The production text of the core crates with comments cut (strings kept), and the comments alone: an id written in code vs only in a doc."""
    code, docs = [], []
    for root in (os.path.join(REPO, "crates/ti4-engine-core/src"), os.path.join(REPO, "crates/ti4-engine-core-factions/src")):
        for f in glob.glob(root + "/**/*.rs", recursive=True):
            if "/generated/" in f or "/tests/" in f or f.endswith("_tests.rs") or f.endswith("/tests.rs"):
                continue
            t = open(f).read()
            i = t.find("#[cfg(test)]")
            t = t if i < 0 else t[:i]
            for line in t.split("\n"):
                s = line.lstrip()
                if s.startswith("//"):
                    docs.append(s)
                else:
                    code.append(re.sub(r"//.*$", "", line) if '"' not in line.split("//")[0][-1:] else line)
    return "\n".join(code), "\n".join(docs)


def core_text():
    files = []
    for root in (os.path.join(REPO, "crates/ti4-engine-core/src"), os.path.join(REPO, "crates/ti4-engine-core-factions/src")):
        for f in glob.glob(root + "/**/*.rs", recursive=True):
            if "/generated/" in f or "/tests/" in f or f.endswith("_tests.rs") or f.endswith("/tests.rs"):
                continue
            files.append(f)
    return {f: prod(f) for f in sorted(files)}


def names(toml):
    out = {}
    for line in open(os.path.join(CORE, "names", toml + ".toml")):
        m = re.match(r'"([^"]+)"\s*=\s*"([^"]+)"', line)
        if m:
            out[m.group(1)] = m.group(2)
    return out


def sources_of(gen):
    """variant -> source name, from the data table (`// Variant` then `source: Source::X`)."""
    t = open(os.path.join(CORE, "src/generated", gen + ".rs")).read()
    out = {}
    for m in re.finditer(r"//\s*(\w+)\s*\n\s*\w+Data \{(.*?)\n    \},", t, re.S):
        s = re.search(r"source:\s*Source::(\w+)", m.group(2))
        out[m.group(1)] = s.group(1) if s else ""
    return out


def string_registered():
    out = {}
    src = os.path.join(REPO, "crates/ti4-engine/src")

    def aliases(path, fn):
        t = open(os.path.join(src, path)).read()
        m = re.search(r"fn %s\(\)[^{]*\{(.*?)\n\}" % fn, t, re.S)
        return set(re.findall(r'"([^"]+)"', m.group(1))) if m else None

    out["public_objective"] = aliases("objectives.rs", "registered_aliases")
    out["secret_objective"] = aliases("secrets.rs", "registered_aliases")
    t = open(os.path.join(src, "action_cards.rs")).read()
    m = re.search(r"const REGISTERED_ALIASES: &\[&str\] = &\[(.*?)\];", t, re.S)
    out["action_card"] = set(re.findall(r'"([^"]+)"', m.group(1))) if m else None
    return out


def action_ported():
    """Cards with an Fx, a Simple or a REGISTERED card play."""
    base = os.path.join(CORE, "src/rules/action_cards")
    got = set()
    t = prod(os.path.join(base, "effects.rs"))
    for fn in ("Fx", ):
        m = re.search(r"impl Fx \{.*?pub const fn of\(card: ActionCard\)[^{]*\{(.*?)\n    \}\n", open(os.path.join(base, "effects.rs")).read(), re.S)
        got |= set(re.findall(r"C::(\w+)", re.sub(r"=>[^\n]*", "", m.group(1)))) if m else set()
    m = re.search(r"impl Simple \{.*?pub const fn of\(card: ActionCard\)[^{]*\{(.*?)\n    \}\n", open(os.path.join(base, "mod.rs")).read(), re.S)
    got |= set(re.findall(r"C::(\w+)", re.sub(r"=>[^\n]*", "", m.group(1)))) if m else set()
    got |= {"Parley", "DirectHit1", "DirectHit2", "DirectHit3", "DirectHit4", "SkilledRetreat1", "SkilledRetreat2", "SkilledRetreat3", "SkilledRetreat4"}
    return got


def main(argv):
    text = core_text()
    blob = "\n".join(text.values())
    code_raw, docs_raw = raw_texts()
    both_lower = (code_raw + "\n" + docs_raw).lower().replace("\u2019", "'")
    reg = string_registered()
    ap = action_ported()
    rows, tally = [], {}
    for kind, jf, idf, toml, gen, enum, ff in KINDS:
        recs = json.load(open(os.path.join(CONTENT, jf + ".json")))
        nm = names(toml)
        src = sources_of(gen)
        for r in recs:
            rid = r[idf]
            var = nm.get(rid)
            if var is None:
                rows.append((kind, rid, "", "", "no variant name in names/%s.toml" % toml, "", "", "0", "0", "-1"))
                continue
            faction = (r.get(ff) if ff else "") or ""
            source = src.get(var, r.get("source", ""))
            if kind == "action_card":
                port = var in ap
                mentions = 1 if port else 0
            else:
                mentions = len(re.findall(r"::%s\b" % re.escape(var), blob))
                port = mentions > 0
            idm = idd = 0
            if len(rid) >= 6:
                idm = len(re.findall(r'["`:]%s["`:]' % re.escape(rid), code_raw))
                idd = len(re.findall(r"\b%s\b" % re.escape(rid), docs_raw))
            name = (r.get("name") or "").lower().replace("\u2019", "'")
            named = both_lower.count(name) if len(name) >= 5 else -1
            string = ""
            if reg.get(kind) is not None:
                string = "y" if rid in reg[kind] else "n"
            rows.append((kind, rid, var, faction, source, str(mentions), string, str(idm), str(idd), str(named)))
            tally.setdefault(kind, [0, 0, 0])
            tally[kind][0] += 1
            if not port:
                tally[kind][1] += 1
                if faction in DROPPED:
                    tally[kind][2] += 1
    nomention = [r for r in rows if r[5] == "0"]
    real = [r for r in nomention if r[7] == "0" and r[9] in ("0", "-1")]
    if "--tsv" in argv:
        with open(argv[argv.index("--tsv") + 1], "w") as fh:
            fh.write("kind\tid\tcore_variant\tfaction\tsource\tcore_mentions\tstring_registered\tid_in_core_code\tid_in_core_docs\tname_in_core_text\tdropped_faction\n")
            for r in rows:
                fh.write("\t".join(list(r) + ["y" if r[3] in DROPPED else ""]) + "\n")
    print("kind: ids, no ::Variant mention in the core, of which dropped factions, of the rest: no trace by id or by printed name either")
    for k, (n, miss, dr) in tally.items():
        notrace = sum(1 for r in real if r[0] == k and r[3] not in DROPPED)
        print("  %-17s %4d %4d %4d %4d" % (k, n, miss, dr, notrace))
    if "--summary" in argv:
        for r in nomention:
            print("  %-17s %-28s %-30s faction=%-10s source=%-12s id_in_code=%s id_in_docs=%s name_in_core=%s%s" % (r[0], r[1], r[2], r[3], r[4], r[7], r[8], r[9], "  DROPPED" if r[3] in DROPPED else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
