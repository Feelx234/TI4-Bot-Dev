#!/usr/bin/env python3
"""Write tools/rulebot/d3/base.json: the "before" of the typed bot (docs/todo/D3-bots.md D3-0 item 4/5).

    python3 -I tools/rulebot/d3/make_base.py BENCH_JSON TIME_FILE COUNTS_JSON REV > tools/rulebot/d3/base.json

BENCH_JSON  the `selfplay run --out` file of the golden set played by the base binary (its `combined.by_subtype`,
            `combined.bot_share`, `meta`)
TIME_FILE   `/usr/bin/time -f "%U %S %e"` output of that run
COUNTS_JSON `bot_shadow --json` of a build with `--features observed-count` (the view calls per decision subtype)
REV         the git revision the base binary was built from
"""
import json
import sys

bench = json.load(open(sys.argv[1]))
user, sys_s, wall = (float(x) for x in open(sys.argv[2]).read().split())
counts = json.load(open(sys.argv[3]))
rev = sys.argv[4]
combined = bench["combined"]
by_subtype = {k: {"n": v["n"], "smart": v["smart"], "ns": v["ns"]} for k, v in combined["by_subtype"].items()}
hot = ["action_menu", "activate_system", "produce_unit", "draft_strategy_card", "research_technology"]
view = {}
for sub in hot:
    n = counts["by_subtype"].get(sub, {}).get("calls", 0)
    names = counts.get("view_calls", {}).get(sub, {})
    if n:
        view[sub] = {
            "decisions": n,
            "calls_per_decision": round(sum(names.values()) / n, 1),
            "top": {k: round(v / n, 1) for k, v in sorted(names.items(), key=lambda kv: -kv[1])[:10]},
        }
games = combined["games"]
out = {
    "rev": rev,
    "profile": bench["meta"].get("profile"),
    "games": games,
    "decisions": combined["decisions"]["n"] * combined["decisions"]["mean"],
    "bot_share": combined["bot_share"],
    "games_per_hour": bench["meta"].get("games_per_hour"),
    "cpu_s": round(user + sys_s, 1),
    "wall_s": wall,
    "by_subtype": by_subtype,
    "view_calls_per_decision": view,
    "note": "string bot at the base commit; view calls from 3 golden games (profile-20 games 0, 5, 11) of bot_shadow --features observed-count",
}
json.dump(out, sys.stdout, indent=1, sort_keys=True)
print()
