# tools/rulebot

Rule-bot self-play (`selfplay.sh`, see `docs/RULE_BOT_OPTIMISATION.md`).

- `golden.json` / `baseline*.json`: the golden benchmark (200 games) and its recorded summaries
  (`baseline-v1.json`, `-v2`, `-v3` are bot versions 1 to 3; `baseline-v4.json`, `-v5` bot versions 4 and 5; `baseline.json` the shipped bot, version 6).
- `golden-lent.json`: the targeted census of the lent bodies (package W0 of `docs/todo/lent-bodies.md`), NOT a baseline: 19 games
  whose seats are fixed to the factions with most guards (Nomad, Naaz, Letnev, Yin, Mentak, Yssaril; Argent, Jol-Nar, Sardakk,
  Mahact, Hacan, Titans; and a 4-player table Nomad, Naaz, Letnev, Mentak), because the golden games draw from the lobby pool and a given
  faction sits in only 17-25% of them. Run it like the golden dual: `TI4_NESTED_LOG=1 SHARDS=4 tools/engine_core/dual_sharded.sh
  <gate>/dual_run tools/rulebot/golden-lent.json 20 all=dual OUT/shards > OUT/lent.txt` (under `flock /root/.ti4-heavy.lock`), then
  `tools/engine_core/lent_census.py OUT/lent.txt` prints the table window x reason x runs and fails on a bail with no reason.
- `completion*.json`, `stress.json`: time-to-completion and stuck/panic hunting sets.
- `results/` (git-ignored): **stored games**. Every `selfplay run` and `compare` (so `selfplay.sh ab`)
  writes its games, one JSON line each, with the per-round timeline (snapshots of every seat at the end
  of each action phase, victory point events, activations, home attacks, eligible objectives, losses; the
  shape of the former `analyze` output) to `results/<key>.jsonl`. The key hashes the games, every seat's
  bot numbers, the shipped defaults and the harness version. The same run again reads the file instead of
  playing (`--no-cache` plays anyway; `bench` and `baseline` always do; `SELFPLAY_SALT=x` forces new keys
  after a bot change that no knob shows). `selfplay metrics results/<key>.jsonl` prints the A/B metrics
  of a stored compare run without any rerun. Delete files freely; they are rebuilt by rerunning.
- A/B metrics (`compare`): VP per round and VP after rounds 3, 4 and 5 (paired by table; the primary
  strength measure from round 2 on), win share, VP per seat, rounds, per faction and per map.
  `--mode separate` plays the table once with the baseline on every seat and once with the
  candidate on every seat: the same VP metrics are then the absolute strength of the bot.
