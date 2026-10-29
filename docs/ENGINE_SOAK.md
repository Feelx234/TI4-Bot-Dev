# Engine soak: a large differential corpus and a background runner for the dual run

Tooling only (branch `core-soak`, cut from `core-dual-shards` and merged with `core-integration-3` @ `1e55ab28`). No engine rule
changed; `tools/engine_profile/profile-20.json`, the bench configs and the baselines are untouched (byte-identical). Purpose: the
core port is checked by compute. Bugs of the kind "Nekro mech counted twice" or "Saar commander destination" were found only
by benches; the dual run finds them on its own once it plays many more, and more varied, games than the 20 of the golden
profile.

## 1. What was missing

* Every golden profile pins `rules_version: 23`; the model is at 35. Crisis Pact (31), Classified (32), Covert Legislation (33),
  base Diplomacy (35) and Thunder's Edge were never played by the dual run. Only 4p and 6p, two maps each, ten seeds each.
* Thunder's Edge is **off by default** (`GameSpec::thunders_edge`, `setup::DEFAULT_THUNDERS_EDGE = false`; default sources are
  `POK`, `FULL` only with the switch), so a corpus without the switch never sees its cards, relics and expedition.
* 20 games, one bot.

## 2. The corpus

`tools/engine_core/soak_corpus.py` generates selfplay matrices (the format `dual_run CONFIG GAMES AREAS` already reads). A
**lane** is one matrix with one bot and a mix of setups; `gen LANE GAMES FIRST_SEED` is deterministic, splits `GAMES` over the
setups and gives each setup its own seed range, so (lane, games, first seed) names the same games forever.

| lane | weight | varies |
|---|---|---|
| `std` | 38 | rules version 35 (newest; not pinned, so it follows the model), Thunder's Edge off, 3-8 players (10/25/15/25/10/15 %), every template map that builds for the count (hyperlane maps included) plus the seeded random board (`map_policy: random`, the seed picks), the lobby's faction draw |
| `te` | 14 | the same, Thunder's Edge on |
| `old` | 14 | 18 older rules versions (0, 1, 3, 5, 8, 12, 16, 19, 21, 22, 23, 26, 29, 30, 31, 32, 33, 34; every gate boundary just under 35), 3-6 players, alternating Thunder's Edge |
| `factions` | 18 | each of the 24 lobby factions pinned on seat a, 3-8 players, alternating Thunder's Edge; the other seats drawn |
| `bot-v1`, `bot-v3`, `bot-v5` | 5/5/6 | the rule bot at version 1, 3, 5 (matrix `bot: {bot_version}`), which plays other lines (other attacks, other purchases) than the shipped version 6 |

Excluded on purpose: diplomacy, the deck reserve and turn redo (being disabled), Last Bastion and Crimson (not in
`LOBBY_FACTIONS`, nobody can be seated as them), the Thunder's Edge factions and the Keleres (not in `LOBBY_FACTIONS` either).
Every game uses `starting_tech: true` (the product's choice step) and `speaker: random`.

Committed tier files (never replacing `profile-20.json`): `tools/engine_profile/soak/<lane>-{100,300,1000}.json` (seeds from
1; regenerate with `python3 tools/engine_core/soak_corpus.py profiles`), `maps.json` (the template aliases per player count,
from `selfplay maps`; regenerate when templates are added). The soak itself does not use the tiers: it asks `gen` for new
seed ranges, so it keeps finding new games.

**One engine-side harness change** (`ti4-engine-core-bridge`, no rule change): `workload::golden_games` now installs the
matrix's `bot` overrides (`tuning_from`) like `selfplay run` does; before it ignored them and always played the shipped bot.
A matrix without `bot` (all golden profiles) does exactly what it did. `dual_run` got `--specs CONFIG GAMES` (one JSON line per
game: index, seed, map, players, factions, rules version, `selfplay replay` flags), `--game I` and `--seed S` (play only that
game, unsharded text).

Coverage check: `dual_run --specs ... | soak_corpus.py coverage` prints the histogram of players, maps and factions. For the
five 100-game std/te/old/factions/bot-v1 tiers together: every one of the 24 lobby factions is seated 62 to 151 times, 49
distinct maps incl. all the hyperlane ones, 3-8 players.
