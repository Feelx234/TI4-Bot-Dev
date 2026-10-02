#!/usr/bin/env python3
"""Write a self-contained HTML sheet of everything six factions can do.

Source is the engine's own content corpus (`crates/ti4-content/content/*.json`), which is what the
engine actually plays - not a wiki's transcription of it. Each card also carries where the engine's
Rust source mentions the id, as a pointer for the leader/faction-ability work (LEADER-FIX-002) and not
as a claim that the rules are right: an ability can be implemented, referenced, and still unreachable.

Usage:  python scripts/make_faction_reference.py [out/six-faction-abilities.html]
"""

import datetime
import html
import io
import json
import os
import re
import subprocess
import sys

CONTENT = os.path.join("crates", "ti4-content", "content")
FACTIONS = ["hacan", "jolnar", "letnev", "l1z1x", "xxcha", "sol"]
ACCENT = {
    "hacan": "#c8862b",
    "jolnar": "#4b7fd4",
    "letnev": "#b0472f",
    "l1z1x": "#6fae4f",
    "xxcha": "#3fa091",
    "sol": "#d4b13f",
}

CSS = """
:root { color-scheme: light dark; }
body { margin:0; padding:0 0 4rem; font:15px/1.55 system-ui, "Segoe UI", Roboto, sans-serif;
       background:#12161c; color:#e6e9ee; }
header { padding:1.6rem 1.4rem 1rem; border-bottom:1px solid #2a313b; }
h1 { margin:.1rem 0 .4rem; font-size:1.6rem; letter-spacing:.2px; }
.sub { color:#9aa6b4; font-size:.86rem; }
nav { display:flex; flex-wrap:wrap; gap:.4rem; margin-top:.9rem; }
nav a { text-decoration:none; color:#e6e9ee; background:#1c222b; border:1px solid #2f3844;
        padding:.32rem .7rem; border-radius:999px; font-size:.86rem; }
nav a:hover { border-color:#6d7a8a; }
section { padding:1.2rem 1.4rem 0; border-top:3px solid var(--accent, #444); margin-top:1.4rem; }
h2 { margin:.2rem 0 .3rem; font-size:1.28rem; border-left:6px solid var(--accent); padding-left:.6rem; }
h2 .key { font-size:.72rem; color:#8b97a6; background:#1c222b; padding:.1rem .45rem;
          border-radius:4px; margin-left:.4rem; vertical-align:middle; }
h3 { margin:1.1rem 0 .5rem; font-size:.78rem; text-transform:uppercase; letter-spacing:.09em;
     color:#93a1b1; }
.facts { margin:.2rem 0 0; color:#b9c4d0; font-size:.88rem; }
.grid { display:grid; gap:.7rem; grid-template-columns:repeat(auto-fill,minmax(310px,1fr)); }
.card { background:#1a1f27; border:1px solid #2b333d; border-left:3px solid var(--accent);
        border-radius:8px; padding:.7rem .85rem; break-inside:avoid; }
.card h4 { margin:0 0 .35rem; font-size:.98rem; }
.tag { display:inline-block; margin-left:.45rem; font:400 .68rem/1 ui-monospace, Consolas, monospace;
       color:#93a1b1; background:#232a34; padding:.12rem .38rem; border-radius:4px;
       vertical-align:middle; }
.body p { margin:.3rem 0; }
.window { color:#c8d3df; font-weight:600; }
.who { color:#cbb27a; }
.perm { border-left:2px solid var(--accent); padding-left:.5rem; }
.unlock { color:#93a1b1; font-size:.85rem; }
.ok, .no { display:inline-block; margin-top:.45rem; font-size:.72rem; padding:.1rem .4rem;
           border-radius:4px; }
.ok { color:#9ed3a8; background:#1c3125; }
.no { color:#e2a8a0; background:#331f1d; }
.note { margin:1.4rem 1.4rem 0; padding:.8rem .95rem; background:#171d25; border:1px solid #2b333d;
        border-radius:8px; color:#a9b5c2; font-size:.84rem; }
@media print { body { background:#fff; color:#111; } .card { background:#fff; } }
"""


def load(name):
    with io.open(os.path.join(CONTENT, name), encoding="utf-8") as handle:
        return json.load(handle)


def engine_source_blob():
    """One string of every line of the engine's Rust source, for id lookups."""
    parts = []
    for root, _dirs, files in os.walk(os.path.join("crates", "ti4-engine", "src")):
        for name in files:
            if name.endswith(".rs"):
                with io.open(os.path.join(root, name), encoding="utf-8", errors="replace") as handle:
                    parts.append(handle.read())
    return "\n".join(parts)


def esc(text):
    if not text:
        return ""
    return html.escape(str(text)).replace("\n", "<br>")


def main():
    target = sys.argv[1] if len(sys.argv) > 1 else os.path.join("out", "six-faction-abilities.html")
    blob = engine_source_blob()

    def seen(token):
        return bool(re.search('["|]' + re.escape(token) + '["|]', blob))

    def status(token):
        if seen(token):
            return '<span class="ok">engine: id present</span>'
        return '<span class="no">engine: id not found</span>'

    def card(title, body, tag=""):
        tag_html = '<span class="tag">' + esc(tag) + "</span>" if tag else ""
        return '<article class="card"><h4>' + esc(title) + tag_html + '</h4><div class="body">' + body + "</div></article>\n"

    factions = {f.get("alias"): f for f in load("factions.json")}
    abilities = {a["id"]: a for a in load("abilities.json")}
    leaders = {l["id"]: l for l in load("leaders.json")}
    techs = {t.get("alias"): t for t in load("technologies.json")}
    units = load("units.json")
    planets = {p["id"]: p for p in load("planets.json")}
    notes = {n.get("alias"): n for n in load("promissory_notes.json")}

    sections = []
    for key in FACTIONS:
        faction = factions[key]
        parts = ['<section class="faction" id="' + key + '" style="--accent:' + ACCENT[key] + '">\n']
        parts.append("<h2>" + esc(faction["factionName"]) + ' <span class="key">' + key + "</span></h2>\n")
        facts = ["Home system <b>" + esc(faction.get("homeSystem")) + "</b>"]
        facts.append("Commodities <b>" + esc(faction.get("commodities")) + "</b>")
        if faction.get("complexity"):
            facts.append("Complexity <b>" + esc(faction["complexity"]) + "</b>")
        if faction.get("priorityNumber"):
            facts.append("Initiative priority <b>" + esc(faction["priorityNumber"]) + "</b>")
        parts.append('<p class="facts">' + " &nbsp;·&nbsp; ".join(facts) + "</p>\n")

        parts.append("<h3>Faction abilities</h3>\n<div class=\"grid\">\n")
        for ability_id in faction.get("abilities", []):
            ability = abilities.get(ability_id, {})
            body = ""
            if ability.get("window"):
                body += '<p class="window">' + esc(ability["window"]) + "</p>\n"
            if ability.get("windowEffect"):
                body += "<p>" + esc(ability["windowEffect"]) + "</p>\n"
            if ability.get("permanentEffect"):
                body += '<p class="perm">' + esc(ability["permanentEffect"]) + "</p>\n"
            parts.append(card(ability.get("name", ability_id), body + status(ability_id), ability_id))
        parts.append("</div>\n")

        parts.append("<h3>Leaders</h3>\n<div class=\"grid\">\n")
        for leader_id in faction.get("leaders", []):
            leader = leaders.get(leader_id, {})
            body = ""
            who = " · ".join(x for x in [leader.get("name"), leader.get("title")] if x)
            if who:
                body += '<p class="who">' + esc(who) + "</p>\n"
            if leader.get("abilityWindow"):
                body += '<p class="window">' + esc(leader["abilityWindow"]) + "</p>\n"
            if leader.get("abilityText"):
                body += "<p>" + esc(leader["abilityText"]) + "</p>\n"
            if leader.get("unlockCondition"):
                body += '<p class="unlock">Unlock — ' + esc(leader["unlockCondition"]) + "</p>\n"
            title = str(leader.get("type", "leader")).title() + " leader"
            parts.append(card(title, body + status(leader_id), leader_id))
        parts.append("</div>\n")

        tech_ids = list(dict.fromkeys(list(faction.get("factionTech", [])) + list(faction.get("startingTech", []))))
        parts.append("<h3>Faction technologies</h3>\n<div class=\"grid\">\n")
        for tech_id in tech_ids:
            tech = techs.get(tech_id, {})
            starting = tech_id in faction.get("startingTech", [])
            label = tech.get("name", tech_id) + (" — starting technology" if starting else "")
            body = ""
            if tech.get("types"):
                body += '<p class="window">' + esc(", ".join(tech["types"])) + "</p>\n"
            if tech.get("requirements"):
                body += '<p class="unlock">Requires ' + esc(tech["requirements"]) + "</p>\n"
            body += "<p>" + esc(tech.get("text")) + "</p>\n"
            parts.append(card(label, body + status(tech_id), tech.get("initials", tech_id)))
        parts.append("</div>\n")

        unique = [u for u in units if u.get("faction") == key and u.get("ability")]
        if unique:
            parts.append("<h3>Unique units</h3>\n<div class=\"grid\">\n")
            for unit in unique:
                stats = " · ".join(
                    x
                    for x in [
                        str(unit["cost"]) + " cost" if unit.get("cost") is not None else "",
                        str(unit["combatDieCount"]) + " dice" if unit.get("combatDieCount") else "",
                        "hits on " + str(unit["combatHitsOn"]) + "+" if unit.get("combatHitsOn") else "",
                        "sustain damage" if unit.get("sustainDamage") else "",
                        "capacity " + str(unit["capacityUsed"]) if unit.get("capacityUsed") else "",
                    ]
                    if x
                )
                body = ""
                if unit.get("baseType"):
                    body += '<p class="who">' + esc(unit["baseType"]) + "</p>\n"
                if stats:
                    body += '<p class="window">' + esc(stats) + "</p>\n"
                body += "<p>" + esc(unit.get("ability")) + "</p>\n"
                parts.append(card(unit.get("name", unit["id"]), body + status(unit["id"]), unit["id"]))
            parts.append("</div>\n")

        homes = [planets[p] for p in faction.get("homePlanets", []) if p in planets]
        if homes:
            parts.append("<h3>Home worlds</h3>\n<div class=\"grid\">\n")
            for planet in homes:
                body = (
                    '<p class="window">Resources ' + esc(planet.get("resources")) + " · Influence "
                    + esc(planet.get("influence")) + "</p>\n"
                )
                if planet.get("techSpecialties"):
                    body += '<p class="window">Tech specialty: ' + esc(", ".join(planet["techSpecialties"])) + "</p>\n"
                if planet.get("legendaryAbilityName"):
                    body += '<p class="who">' + esc(planet["legendaryAbilityName"]) + "</p>\n"
                if planet.get("legendaryAbilityText"):
                    body += "<p>" + esc(planet["legendaryAbilityText"]) + "</p>\n"
                parts.append(card(planet.get("name", planet["id"]), body, planet.get("tileId", "")))
            parts.append("</div>\n")

        owned = [notes[nid] for nid in faction.get("promissoryNotes", []) if nid in notes]
        if owned:
            parts.append("<h3>Promissory notes</h3>\n<div class=\"grid\">\n")
            for note in owned:
                body = ""
                if note.get("playArea"):
                    body += '<p class="window">' + esc(note["playArea"]) + "</p>\n"
                body += "<p>" + esc(note.get("text")) + "</p>\n"
                alias = note.get("alias", "")
                parts.append(card(note.get("name", alias), body + status(alias), alias))
            parts.append("</div>\n")

        parts.append("</section>\n\n")
        sections.append("".join(parts))

    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip()
    nav = "".join(
        '<a href="#' + key + '" style="border-color:' + ACCENT[key] + '">' + esc(factions[key]["factionName"]) + "</a>"
        for key in FACTIONS
    )
    legend = (
        '<div class="note"><b>Reading the badges.</b> The grey tag is the id this engine uses. The green or '
        "red badge is a search of <code>crates/ti4-engine/src</code> for that id: green means the engine "
        "mentions it somewhere, red means the id appears nowhere in the engine's source. Neither proves the "
        "rules work - an ability can be implemented, referenced and still unreachable, which is exactly the "
        "L1Z1X agent defect (its window is never raised), and red can mean the behaviour lives under another "
        "name. They say where to look, not what works.</div>"
    )
    head = (
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n"
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n"
        "<title>Six factions — every ability, in full</title>\n<style>" + CSS + "</style></head><body>\n"
        "<header>\n<h1>Six factions — every ability, in full</h1>\n"
        '<div class="sub">Hacan · Jol-Nar · Letnev · L1Z1X · Xxcha · Sol — faction abilities, leaders, '
        "technologies, unique units, home worlds and promissory notes, with the complete printed text.</div>\n"
        '<div class="sub">Generated from the engine\'s own content corpus, <code>crates/ti4-content/content/</code>'
        " (" + ", ".join(["factions", "abilities", "leaders", "technologies", "units", "planets", "promissory notes"])
        + ") on " + datetime.date.today().isoformat() + ", engine commit " + (commit or "unknown")
        + ". Regenerate with <code>python scripts/make_faction_reference.py</code>.</div>\n"
        '<nav>' + nav + "</nav>\n</header>\n" + legend + "\n"
    )

    directory = os.path.dirname(target)
    if directory:
        os.makedirs(directory, exist_ok=True)
    with io.open(target, "w", encoding="utf-8") as handle:
        handle.write(head + "".join(sections) + "</body></html>\n")
    print(target, os.path.getsize(target), "bytes")


if __name__ == "__main__":
    main()
