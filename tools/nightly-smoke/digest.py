#!/usr/bin/env python3
"""Compile a markdown digest of one nightly smoke run.

    digest.py <run-dir>

Reads the run's meta.json, the harness trace (trace/), the backend's game record (server-data/
or the live /tmp data dir) and run.log. Prints, in order of importance for bug hunting:
failure / errors / rejections / suspicious log lines, then the game's interesting events
(action cards, Mecatol Rex, technologies, strategy cards, agendas, objectives, combat).
"""

import collections
import glob
import json
import os
import re
import sys

import te_report

SUSPICIOUS = re.compile(
    r"panicked|RUST_BACKTRACE|\bERROR\b|Error:|error\[|rejected:|click failed|pageerror|"
    r"console:|did not advance|no actionable control|never reached|game idle|timed out|✘",
)
# Vite dev-server proxy chatter: the /advisor/battle odds proxy refuses connections because no
# advisor runs in the smoke setup, and websocket reconnects reset sockets. Neither affects play.
NOISE = re.compile(
    r"\[frontend\].*(ECONNREFUSED|ECONNRESET|socket has been ended|ws proxy error|http proxy error)"
    r"|\[vite\] (ws |http )?proxy error|/advisor"
    # The first page loads before the backend answers /api/catalog; CatalogProvider retries (2026-10-22:
    # in almost every server run, always at the start, and the vite clock is UTC).
    r"|\[frontend\].*No catalog of the printed cards: CatalogError: /api/catalog could not be reached",
)
# A teardown error when the deadline kills the run (exit 124): the browser is closed under the harness.
TEARDOWN = re.compile(r"has been closed|Target page, context or browser|Test ended|browserContext\.close")
CLICK_LINE = re.compile(r"^#(\d+) .*? seat(\d+) click (turn-bar-[\w-]+)")

COMBAT = {
    "sustain_damage", "assign_casualty", "announce_retreat", "retreat_to",
    "fight_ground_combat_round", "assign_ground_casualty", "commit_ground_forces",
}
# The two printings of Mecatol Rex: the base planet and Thunder's Edge's legendary one (tile 112).
MECATOL_PLANETS = ("mr", "mrte")


def content_names():
    """id -> display name for every record of the catalogue (`fixtures/catalog/catalog.json`)."""
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "fixtures",
                        "catalog", "catalog.json")
    try:
        with open(path) as handle:
            records = json.load(handle)["records"]
    except (OSError, ValueError, KeyError):
        return {}
    return {record["id"]: record["name"] for kind in records.values() for record in kind}


NAMES = content_names()


def named(alias):
    alias = str(alias)
    return f"{NAMES[alias]} (`{alias}`)" if alias in NAMES else f"`{alias}`"


def load(path, default=None):
    try:
        with open(path) as handle:
            return json.load(handle)
    except (OSError, ValueError):
        return default


def jsonl(path):
    rows = []
    try:
        with open(path) as handle:
            for line in handle:
                line = line.strip()
                if line:
                    try:
                        rows.append(json.loads(line))
                    except ValueError:
                        pass
    except OSError:
        pass
    return rows


def unwrap(record):
    """Server records wrap their content as {format_version, payload, checksum}."""
    return record.get("payload", record) if isinstance(record, dict) else record


def vp_ledger_line(state, name):
    """One line per game: each seat's ledger rows (custodians, Imperial, objectives ...) and a flag
    where the ledger sum differs from the seat's victory points. The engine's ledger does not yet
    cover secrets, relics and Styx, so a gap is expected there and is not by itself a rules bug."""
    ledger = state.get("vp_ledger") or []
    if not ledger:
        return None
    rows = collections.defaultdict(collections.Counter)
    totals = collections.Counter()
    for entry in ledger:
        try:
            player, amount, source = entry[0], entry[1], entry[2]
        except (IndexError, TypeError):
            continue
        rows[player][str(source)] += amount
        totals[player] += amount
    parts = []
    for pl in state.get("players", []):
        pid = pl.get("id")
        vps = pl.get("victory_points", 0)
        detail = ", ".join(f"{src} {amt:+d}" for src, amt in rows[pid].items()) or "no rows"
        flag = "" if totals[pid] == vps else f" **ledger sum {totals[pid]} != {vps} VP**"
        parts.append(f"{name(pid)}: {detail}{flag}")
    return "- VP ledger: " + "; ".join(parts)


RESEARCH_SUBTYPES = ("research_technology",)
SCORE_SUBTYPES = ("score_objective", "imperial_score_objective", "score_secret_objective")


def decision_log_picks(decisions, subtypes):
    """What each seat picked for the given decision subtypes, from the server's decision log
    (declines left out). Used when the run has no final snapshot to read the state from."""
    picks = collections.OrderedDict()
    for d in decisions:
        if (d.get("context") or {}).get("subtype") not in subtypes:
            continue
        chosen = d.get("chosen")
        if not chosen or chosen in ("decline", "done", "none"):
            continue
        picks.setdefault(d.get("player"), []).append(chosen)
    return picks


def game_dir(run_dir, meta, game_id):
    for base in (os.path.join(run_dir, "server-data"), meta.get("server_data_dir", "")):
        if base and game_id:
            candidates = glob.glob(os.path.join(base, f"*{game_id}*"))
            if candidates:
                return candidates[0]
    return None


def bar_click_note(run_dir):
    """Which turn bar controls the harness clicked on the last decision, from run.log.

    The proctors read an unfinished decision as "Pass does nothing" when the harness had only
    toggled the Components menu (2026-10-22, s2-03 and s1-06: 40+ Components clicks, no Pass click).
    This states what was clicked so that the diagnosis rests on the log.
    """
    last, counts = None, collections.Counter()
    try:
        with open(os.path.join(run_dir, "run.log"), errors="replace") as handle:
            for line in handle:
                m = CLICK_LINE.match(line.strip())
                if not m:
                    continue
                if m.group(1) != last:
                    last, counts = m.group(1), collections.Counter()
                counts[m.group(3)] += 1
    except OSError:
        return ""
    total = sum(counts.values())
    if total < 10:
        return ""
    top = ", ".join(f"{k} x{v}" for k, v in counts.most_common(4))
    note = f"- Turn bar clicks on the last decision (#{last}, from run.log): {top}."
    if not counts.get("turn-bar-pass"):
        note += (" **Pass was never clicked on it**: a stall here is the click policy (menu toggles), "
                 "not a Pass that does nothing.")
    return note


def main():
    run_dir = sys.argv[1]
    meta = load(os.path.join(run_dir, "meta.json"), {})
    trace_dir = os.path.join(run_dir, "trace")
    game = load(os.path.join(trace_dir, "game.json"), {})
    report = load(os.path.join(trace_dir, "report.json"), {})
    snapshot = load(os.path.join(trace_dir, "final-snapshot.json"), {})
    browser_errors = load(os.path.join(trace_dir, "browser-errors.json"), [])
    trace = jsonl(os.path.join(trace_dir, "trace.jsonl"))
    exit_code = open(os.path.join(run_dir, "exit_code")).read().strip() if os.path.exists(
        os.path.join(run_dir, "exit_code")) else "still running"
    failure = ""
    if os.path.exists(os.path.join(trace_dir, "failure.txt")):
        failure = load(os.path.join(trace_dir, "failure.txt"), None) or open(
            os.path.join(trace_dir, "failure.txt")).read()

    gdir = game_dir(run_dir, meta, game.get("gameId"))
    init = unwrap(load(os.path.join(gdir, "init.json"), {})) if gdir else {}
    # decisions.jsonl is truncated; read from history.json's full decision log instead.
    history = unwrap(load(os.path.join(gdir, "history.json"), {})) if gdir else {}
    decisions = history.get("decisions", [])
    initial_state = init.get("initial_state", {})
    state = snapshot.get("state", {})

    faction = {}
    for player in state.get("players", []) or initial_state.get("players", []):
        faction[player.get("id")] = player.get("faction", "?")
    for row in trace:
        if row.get("faction"):
            faction[row["player"]] = row["faction"]
    name = lambda pid: faction.get(pid, (pid or "?")[:14])

    # Join the harness trace (subtype, labels) with the server's record of what was chosen.
    chosen_label = {}
    pointer = 0
    for index, row in enumerate(trace):
        ids = sorted(o["id"] for o in row.get("options") or [])
        for j in range(pointer, len(decisions)):
            d = decisions[j]
            if d.get("player") == row.get("player") and sorted(d.get("offered", [])) == ids:
                labels = {o["id"]: o.get("label") or o["id"] for o in row["options"]}
                chosen_label[index] = (d.get("chosen"), labels.get(d.get("chosen"), d.get("chosen")))
                pointer = j + 1
                break

    out = []
    p = out.append
    p(f"### Run digest — game seed {meta.get('game_seed')} · click seed {meta.get('click_seed')} · "
      f"{meta.get('players')} players · policy {meta.get('policy')} · preset {meta.get('preset') or 'none'} · cards {meta.get('card_set') or 'te'} · {te_report.label(meta, report)}"
      + (" · web=build (production bundle)" if meta.get("web") == "build" else "")
      + (f" · commit {str(meta['commit'])[:8]}" + (f" (+{meta['dirty_files']} uncommitted files)" if meta.get("dirty_files") else "")
         if meta.get("commit") else ""))
    p("")
    p("#### Outcome")
    rounds = report.get("roundStarts", {})
    p(f"- exit code: **{exit_code}**"
      + (" (stall detector killed it)" if os.path.exists(os.path.join(run_dir, "stalled")) else "")
      + (" (the deadline was reached; not a stall)" if exit_code == "124" else ""))
    p(f"- decisions resolved: {report.get('decisions', len(trace))}, clicks: {report.get('clicks', '?')}, "
      f"rounds reached: {max(map(int, rounds), default='?') if rounds else (trace[-1]['round'] if trace else '?')}, "
      f"game finished: {report.get('finished', False)}")
    p(f"- final turn status: `{json.dumps(report.get('finalStatus') or snapshot.get('turn_status'))[:300]}`")
    p(te_report.line(meta, report, state, faction))
    p(f"- repro: `{meta.get('repro', '?')}`")
    if gdir:
        p(f"- server game record: `{gdir}`")
    p("")

    p("#### Potential bugs (raw evidence)")
    if failure and exit_code == "124" and TEARDOWN.search(str(failure)) and not report.get("findings"):
        p("- Teardown error at the deadline (exit 124, the browser was closed under the harness): "
          "known harness noise, not a finding. `" + str(failure).strip()[:160].replace("`", "'") + "`")
    elif failure:
        p("- **Harness failure** (game could not continue or UI broke):")
        p("```")
        p(str(failure).strip()[:3000])
        p("```")
    elif not report and exit_code == "124":
        p("- No report.json: the deadline killed the run before Playwright wrote it (known harness noise when "
          "nothing else is listed here; the trace and run.log still hold the evidence).")
    stall_note = bar_click_note(run_dir)
    if stall_note:
        p(stall_note)
    if browser_errors:
        p(f"- **Browser errors** ({len(browser_errors)}):")
        for line in browser_errors[:20]:
            p(f"  - `{str(line)[:400]}`")
    rejections = report.get("rejections", [])
    if rejections:
        p(f"- **Server rejections of offered UI controls** ({len(rejections)}):")
        for line in rejections[:30]:
            p(f"  - `{line[:400]}`")
    exercises = report.get("exercises") or {}
    if exercises:
        p("- **Optional UI exercises** (`report.json` `exercises`): `" + json.dumps(exercises)[:1200] + "`")
    prep = report.get("prep")
    if prep:
        counts = {k: v for k, v in prep.items() if k != "cases"}
        p("- **Secondary pre-planning** (`report.json` `prep`): `" + json.dumps(counts) + "`")
        for line in (prep.get("cases") or [])[:12]:
            p(f"  - {str(line)[:300]}")
    findings = report.get("findings") or []
    if findings:
        p(f"- **Exercise findings** ({len(findings)}):")
        for line in findings[:20]:
            p(f"  - {str(line)[:400]}")
    log_hits = collections.OrderedDict()
    try:
        with open(os.path.join(run_dir, "run.log"), errors="replace") as handle:
            for line in handle:
                if SUSPICIOUS.search(line) and not NOISE.search(line):
                    key = re.sub(r"\d+", "N", line.strip())[:200]
                    log_hits.setdefault(key, [line.strip()[:400], 0])[1] += 1
    except OSError:
        pass
    if log_hits:
        p(f"- **Suspicious run.log lines** ({len(log_hits)} distinct; engine panics and backend errors appear here):")
        for text, count in list(log_hits.values())[:40]:
            p(f"  - ({count}×) `{text}`")
    if trace:
        last = trace[-1]
        p(f"- last offered decision: #{last['decision']} round {last['round']} {last['phase']} "
          f"{name(last['player'])} `{last['subtype']}` prompt “{last.get('prompt')}” options "
          f"{[o['id'] for o in last.get('options') or []][:12]}")
    if not (failure or browser_errors or rejections or log_hits):
        p("- none detected")
    p("")

    p("#### Interesting events")
    p("- seats: " + ", ".join(f"{name(pid)}" for pid in game.get("players", [])))
    # Action cards
    discarded = state.get("discarded_action_cards", []) or []
    initial_discard = initial_state.get("discarded_action_cards", []) or []
    played = list(discarded[len(initial_discard):]) if discarded else []
    sourced = collections.OrderedDict()
    reactions = []
    for index, row in enumerate(trace):
        source = row.get("source") or {}
        if isinstance(source, dict) and "ActionCard" in source:
            sourced.setdefault(source["ActionCard"], (row["round"], name(row["player"])))
        if str(row.get("subtype", "")).startswith("play_reaction_") and index in chosen_label:
            chosen, label = chosen_label[index]
            if chosen not in ("decline", "pass"):
                reactions.append(f"{label} (round {row['round']}, {name(row['player'])})")
        if row.get("subtype") == "prompt:action phase" or row.get("prompt") == "action phase":
            if index in chosen_label and str(chosen_label[index][0]).startswith("action_card|"):
                reactions.append(f"{chosen_label[index][1]} (action, round {row['round']}, {name(row['player'])})")
    p(f"- action cards discarded during the game ({len(played)}): {', '.join(named(c) for c in played) or 'none'}")
    if reactions:
        p(f"- action card plays seen in the UI: {'; '.join(reactions[:40])}")
    if sourced:
        p("- action cards that raised decisions: " + ", ".join(
            f"{named(card)} (r{rnd}, {who})" for card, (rnd, who) in sourced.items()))
    # Mecatol Rex
    mecatol_owner = None
    for system_id, system in (state.get("board") or {}).items():
        control = system.get("planet_control") or {}
        for planet in MECATOL_PLANETS:
            if planet in control:
                mecatol_owner = control[planet]
    activations_18 = sum(1 for d in decisions if d.get("prompt") == "activate a system"
                         and str(d.get("chosen")).replace("activate|", "") in ("18", "112"))
    custodians = [f"round {row['round']} by {name(row['player'])}" for row in trace
                  if row.get("subtype") == "remove_custodians"]
    p(f"- Mecatol Rex: custodians removed: {state.get('custodians_removed', '?')}"
      + (f" ({'; '.join(custodians)})" if custodians else "")
      + f"; controlled by: {name(mecatol_owner) if mecatol_owner else 'nobody'}"
      + f"; system 18 activated {activations_18}×")
    # Technologies
    start_tech = {pl.get("id"): set(pl.get("technologies", [])) for pl in initial_state.get("players", [])}
    researched = []
    for pl in state.get("players", []):
        gained = [t for t in pl.get("technologies", []) if t not in start_tech.get(pl.get("id"), set())]
        if gained:
            researched.append(f"{pl.get('faction')}: {', '.join(named(t) for t in gained)}")
    if not state:
        # No final snapshot (the harness died): read the techs from the server's decision log.
        log_tech = decision_log_picks(decisions, RESEARCH_SUBTYPES)
        researched = [f"{name(pid)}: {', '.join(named(t) for t in techs)}" for pid, techs in log_tech.items()]
        if researched:
            researched.append("(from the decision log; no final snapshot)")
    p(f"- technologies gained: {'; '.join(researched) or 'none'}")
    # Strategy cards
    picks = collections.defaultdict(list)
    for index, row in enumerate(trace):
        if row.get("subtype") == "draft_strategy_card" and index in chosen_label:
            picks[row["round"]].append(f"{name(row['player'])}→{chosen_label[index][1]}")
    if picks:
        p("- strategy cards: " + " | ".join(f"r{r}: {', '.join(v)}" for r, v in sorted(picks.items())))
    # Agendas and laws
    votes = sum(1 for row in trace if row.get("subtype") in ("cast_vote", "vote_tiebreak"))
    laws = state.get("laws") or {}
    p(f"- agenda votes cast: {votes}; laws in play: {', '.join(f'{k}={v}' for k, v in laws.items()) or 'none'}")
    # Objectives and score
    scored = state.get("scored_objectives") or {}
    if not state:
        # No final snapshot: the objectives scored come from the decision log, VP are unknown.
        log_scored = decision_log_picks(decisions, SCORE_SUBTYPES)
        scored = {name(pid): objs for pid, objs in log_scored.items()}
        if scored:
            scored["(source)"] = "decision log; no final snapshot"
    p(f"- objectives scored: {json.dumps(scored)[:400] if scored else 'none'}")
    p("- victory points: " + ", ".join(f"{pl.get('faction')} {pl.get('victory_points', 0)}"
                                        for pl in state.get("players", [])))
    ledger = vp_ledger_line(state, name)
    if ledger:
        p(ledger)
    relics = [f"{pl.get('faction')}: {', '.join(named(r) for r in pl.get('relics', []))}" for pl in state.get("players", []) if pl.get("relics")]
    if relics:
        p(f"- relics: {'; '.join(relics)}")
    subtypes = collections.Counter(row.get("subtype") for row in trace)
    combat = sum(v for k, v in subtypes.items() if k in COMBAT)
    p(f"- combat decisions: {combat}; ground force commitments: {subtypes.get('commit_ground_forces', 0)}; "
      f"productions: {subtypes.get('produce_unit', 0)}; transactions: "
      f"{subtypes.get('propose_transaction', 0) + subtypes.get('answer_transaction', 0)}")
    planet_picks = {k: v for k, v in subtypes.items() if k and (k.endswith("pick_planet") or "planet" in k)}
    if planet_picks:
        p(f"- planet-selection decisions: {dict(planet_picks)}")
    common = [(k, v) for k, v in subtypes.most_common() if v > 3]
    rare = [(k, v) for k, v in subtypes.most_common() if v <= 3]
    p(f"- decision subtypes: {dict(common)}")
    if rare:
        p(f"- rare decision subtypes (3× or fewer): {dict(rare)}")
    print("\n".join(out))


if __name__ == "__main__":
    main()
