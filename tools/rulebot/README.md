# tools/rulebot

Rule-bot self-play (`selfplay.sh`, see `docs/RULE_BOT_OPTIMISATION.md`).

- `golden.json` / `baseline*.json`: the golden benchmark (200 games) and its recorded summaries
  (`baseline-v1.json`, `-v2`, `-v3` are bot versions 1 to 3; `baseline-v4.json`, `-v5` bot versions 4 and 5; `baseline.json` the shipped bot, version 6).
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

## D3: the typed bot (docs/todo/D3-bots.md)

* `tools/rulebot/d3/census.py` counts the string touchpoints of the bot (`census.txt`) and classifies every decision subtype (`subtypes.tsv`); `python3 -I tools/rulebot/d3/test_census.py` checks both files are the regenerated text.
* `bot_shadow` (crate `ti4-engine-core-bridge`) plays games as `selfplay run` does, answers with the string bot and compares the typed bot (`ti4-rule-bot-typed`) at every decider call: `bot_shadow --config tools/engine_profile/profile-20.json --games 0,5,11 [--bot bot_version=N] [--seen-parity STRIDE] [--mutant pick|smart|rng] [--strict]`.
  `--seen-parity` also compares every accessor of the typed view `Seen` with `Observed` at every STRIDE-th call and runs the perturbation test (other seats' hands scrambled).
* `cargo build -p ti4-engine-core-bridge --bin bot_shadow --features observed-count` counts the view calls (`Observed` methods, `PlayerId::new`, ...) per decision subtype and prints them; `tools/rulebot/d3/base.json` holds the baseline of the string bot before the typed one.
