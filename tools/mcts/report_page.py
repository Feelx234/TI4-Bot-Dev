"""A small HTML page of MCTS results (lane L7): the eval reports of `mcts_eval` and, optionally, the held-out numbers of a training run.

    python3 tools/mcts/report_page.py --eval REPORT.json [REPORT2.json ...] [--train RUN_DIR] [--dataset SUMMARY.json ...] --out page.html
"""

from __future__ import annotations

import argparse
import html
import json
from pathlib import Path

CSS = """
/* Layout: one reading column; result tables with a share bar against the 25 % line. */
:root { --bg: #f6f7f9; --panel: #ffffff; --fg: #1d2330; --muted: #5d6677; --line: #d9dde5; --accent: #2f5db8; --good: #23864f; --bad: #b8432f;
  --display: "IBM Plex Sans Condensed", "Arial Narrow", sans-serif; --body: "IBM Plex Sans", system-ui, sans-serif; --mono: "IBM Plex Mono", ui-monospace, monospace; }
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) { --bg: #12151b; --panel: #1a1f28; --fg: #e3e7ee; --muted: #9aa3b3; --line: #2c3340; --accent: #7fa4ec; --good: #5cc48a; --bad: #e88672; color-scheme: dark } }
:root[data-theme="dark"] { --bg: #12151b; --panel: #1a1f28; --fg: #e3e7ee; --muted: #9aa3b3; --line: #2c3340; --accent: #7fa4ec; --good: #5cc48a; --bad: #e88672; color-scheme: dark }
body { background: var(--bg); color: var(--fg); font-family: var(--body); line-height: 1.5; }
main { max-width: 60rem; margin: 0 auto; padding-block: 2rem 3rem; padding-left: max(16px, env(safe-area-inset-left, 0px)); padding-right: max(16px, env(safe-area-inset-right, 0px)); display: grid; gap: 1.75rem; }
h1, h2 { font-family: var(--display); text-wrap: balance; margin: 0; }
h1 { font-size: 1.9rem; } h2 { font-size: 1.25rem; }
p { margin: 0; max-width: 65ch; color: var(--muted); }
section { display: grid; gap: .75rem; min-width: 0; }
.wrap { overflow-x: auto; background: var(--panel); border: 1px solid var(--line); border-radius: 6px; }
table { border-collapse: collapse; width: 100%; font-variant-numeric: tabular-nums; font-size: .9rem; }
th, td { padding: .45rem .7rem; text-align: right; border-bottom: 1px solid var(--line); white-space: nowrap; }
th:first-child, td:first-child { text-align: left; }
th { font-family: var(--display); font-weight: 600; color: var(--muted); text-transform: uppercase; letter-spacing: .04em; font-size: .75rem; }
tr:last-child td { border-bottom: 0; }
.bar { display: inline-block; position: relative; width: 7rem; height: .55rem; background: var(--line); border-radius: 3px; vertical-align: middle; }
.bar i { position: absolute; inset: 0 auto 0 0; background: var(--accent); border-radius: 3px; }
.bar b { position: absolute; top: -3px; bottom: -3px; width: 2px; background: var(--fg); opacity: .55; }
.pill { font-family: var(--mono); font-size: .8rem; padding: .1rem .45rem; border-radius: 999px; border: 1px solid var(--line); }
.up { color: var(--good); } .down { color: var(--bad); }
code { font-family: var(--mono); font-size: .85em; }
"""


def pct(x: float) -> str:
    return f"{100 * x:.1f} %"


def bar(x: float, base: float) -> str:
    return f'<span class="bar"><i style="width:{min(100, 100 * x):.1f}%"></i><b style="left:{100 * base:.1f}%"></b></span>'


def eval_section(r: dict) -> str:
    agent = r.get("agent", "?")
    label = {"rule": "Rule bot", "random": "Random tree answers"}.get(agent, "Greedy policy net" if agent.startswith("policy:") else agent)
    name = html.escape(f"{label} vs {'the best net' if r.get('mode') == 'gate' else 'three rule bots'}")
    base = r.get("baseline_share", 0.25)
    w = r["win_share"]
    paired = r.get("paired", {})
    d = paired.get("delta_win_share", 0.0)
    sprt = r.get("sprt", {})
    rows = []
    for fac, v in sorted(r.get("by_faction", {}).items()):
        a, rb = v["agent"], v.get("rule_baseline", {})
        rows.append(f"<tr><td>{html.escape(fac)}</td><td>{pct(a['share'])} {bar(a['share'], base)}</td><td>{pct(a['wilson95'][0])} to {pct(a['wilson95'][1])}</td>"
                    f"<td>{v['agent_mean_vp']:.2f}</td><td>{pct(rb.get('share', 0))}</td><td>{v.get('rule_mean_vp', 0):.2f}</td><td>{int(a['games'])}</td></tr>")
    net = r.get("net", {})
    agree = net.get("agreement_with_rule_bot")
    mode = "two candidate seats against two of the best" if r.get("mode") == "gate" else "one agent seat against three rule bots"
    return f"""<section><h2>{name}</h2>
<p>{mode.capitalize()}; seeds {r['seeds'][0]} to {r['seeds'][1] - 1} ({r['games']} games, {r['errors']} errors, {r.get('round_capped', 0)} stopped by the round cap). Baseline share {pct(base)}.</p>
<div class="wrap"><table><tr><th>Measure</th><th>Value</th></tr>
<tr><td>Win share</td><td>{pct(w['share'])} {bar(w['share'], base)} &nbsp; 95 % {pct(w['wilson95'][0])} to {pct(w['wilson95'][1])}</td></tr>
<tr><td>Paired Δ win share vs the rule bot on the same seat</td><td class="{'up' if d > 0 else 'down'}">{100 * d:+.1f} pts ± {100 * paired.get('delta_win_share_ci95', 0):.1f}</td></tr>
<tr><td>Paired Δ VP</td><td>{paired.get('delta_vp', 0):+.2f} ± {paired.get('delta_vp_ci95', 0):.2f}</td></tr>
<tr><td>Mean VP / margin over the best rival</td><td>{r.get('mean_vp', 0):.2f} / {r.get('vp_margin_over_best_rival', 0):+.2f}</td></tr>
<tr><td>SPRT p0 {sprt.get('p0')} vs p1 {sprt.get('p1')}</td><td><span class="pill">{sprt.get('verdict')}</span> LLR {sprt.get('llr', 0):.2f} after {sprt.get('observations')} games</td></tr>
<tr><td>Net answers per game, agreement with the rule bot</td><td>{net.get('per_game', 0):.0f}, {pct(agree) if agree is not None else 'not measured'}</td></tr>
</table></div>
<div class="wrap"><table><tr><th>Agent faction</th><th>Win share</th><th>Wilson 95 %</th><th>Mean VP</th><th>Rule bot share</th><th>Rule bot VP</th><th>Games</th></tr>
{''.join(rows)}</table></div></section>"""


def train_section(run: Path) -> str:
    lines = [json.loads(x) for x in (run / "log.jsonl").read_text().splitlines() if x.strip()]
    last = lines[-1]["held"]
    rows = "".join(f"<tr><td>{k}</td><td>{pct(v['top1'])}</td><td>{v['n']}</td></tr>" for k, v in last["top1_by_kind"].items())
    rounds = " · ".join(f"r{k} {v:.3f}" for k, v in last.get("value_ce_by_round", {}).items())
    return f"""<section><h2>Imitation net, held-out games</h2>
<p>Top-1 agreement with the rule bot on held-out games (records with a policy target), and the value head's cross-entropy against the
uniform guess (ln 4 = 1.386) and a guess from the factions alone.</p>
<div class="wrap"><table><tr><th>Measure</th><th>Value</th></tr>
<tr><td>Top-1, all kinds</td><td>{pct(last['top1'])} (uniform among kept options {pct(last['top1_chance'])})</td></tr>
<tr><td>Value CE</td><td>{last['value_ce']:.4f} (uniform {last['value_ce_uniform']:.4f}, faction prior {last['value_ce_faction_prior']:.4f})</td></tr>
<tr><td>Value CE by round</td><td><code>{rounds}</code></td></tr>
<tr><td>Held-out records</td><td>{last['records']}</td></tr>
</table></div>
<div class="wrap"><table><tr><th>Tree kind</th><th>Top-1</th><th>Records</th></tr>{rows}</table></div></section>"""


def dataset_section(summaries: list[dict]) -> str:
    # A `summary-<first>.json` of mcts_dataset (`played`, `errors`) or a `dataset_stats.py` output (`games`, `bytes`).
    games = sum(s.get("played", s.get("games", 0)) for s in summaries)
    records = sum(s["records"] for s in summaries)
    errors = sum(s.get("errors", 0) for s in summaries)
    size = sum(s.get("bytes", 0) for s in summaries)
    kinds: dict[str, int] = {}
    for s in summaries:
        for k, v in s.get("records_by_kind", {}).items():
            kinds[k] = kinds.get(k, 0) + v
    rows = "".join(f"<tr><td>{k}</td><td>{v}</td></tr>" for k, v in kinds.items())
    extra = f", {size / 1e6:.0f} MB of gzip shards" if size else ""
    return f"""<section><h2>Dataset</h2><p>{games} rule-bot games with exploration noise ({errors} dropped by an error), {records} recorded tree decisions
(about {records / max(1, games):.0f} per game){extra}.</p>
<div class="wrap"><table><tr><th>Tree kind</th><th>Records</th></tr>{rows}</table></div></section>"""


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--eval", nargs="*", default=[])
    ap.add_argument("--train")
    ap.add_argument("--dataset", nargs="*", default=[])
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    parts = []
    if a.dataset:
        parts.append(dataset_section([json.loads(Path(p).read_text()) for p in a.dataset]))
    if a.train:
        parts.append(train_section(Path(a.train)))
    for p in a.eval:
        parts.append(eval_section(json.loads(Path(p).read_text())))
    page = f"""<title>MCTS Prep Results</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono&family=IBM+Plex+Sans:wght@400;600&family=IBM+Plex+Sans+Condensed:wght@600&display=swap">
<style>{CSS}</style>
<main><header style="display:grid;gap:.4rem"><h1>MCTS bot, stepping stone 1</h1>
<p>Twilight Imperium 4p Learn-to-play, Sol, Letnev, Xxcha and Jol-Nar. The greedy imitation net answers the tree decisions (draft,
action menu, end of turn, activation, movement, scoring); the rule bot answers everything else. The bars mark the share against the
baseline line.</p></header>
{''.join(parts)}</main>"""
    Path(a.out).write_text(page)


if __name__ == "__main__":
    main()
