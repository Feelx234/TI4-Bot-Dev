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
seven 100-game tier files together (700 games): 3-8 players (83/173/107/177/69/91 seats of games), 51 distinct maps (all 12
hyperlane maps among them, plus the seeded random board), every one of the 24 lobby factions seated 83 to 218 times.
Rules versions: `std`/`te`/`factions`(half)/`bot-*` at the newest (35), `old` over 18 older versions.

**Game order matters for a short run.** `dual_run CONFIG N` plays the first N games of the matrix, which are the setups in
order (3 players first). A tier file is meant to be played whole (`dual_run std-100.json 100`); the soak asks `gen` for
exactly the games it plays.

## 3. The runner: `tools/engine_core/soak.sh`

```
export DUAL_RUN=<built dual_run>      # gate profile is fine; release is about 2x faster for a long soak
tools/engine_core/soak.sh start       # background, nice 19 + ionice idle, log /root/soak-reports/soak.log
tools/engine_core/soak.sh status      # running?, head of LATEST.md, log tail
tools/engine_core/soak.sh stop        # SIGTERM; the slice in flight is abandoned (and redone on the next start)
tools/engine_core/soak.sh run         # foreground (what start launches)
```

Not installed anywhere (no cron, no systemd): only this command starts it.

**A round** is one slice per lane (`plan` in `soak_analyze.py`): `SLICE_GAMES` (default 40) games split over the lanes by
weight, at least 2 per lane. A slice = (lane, first seed, games, mask). Lanes start at `SEED_BASE` (100000) and each lane's next
seed advances by the games it played, so every slice is a new seed range; the mask rotates over lanes and rounds
(`MASKS="all driver wide"`):

| mask | spec | note |
|---|---|---|
| `all` | `all=dual` | every area compares at every call: about 1.3 M calls per game instead of a few hundred, so its slices get 0.3 of the games (`SOAK_MASK_SCALE=all=0.3`) |
| `driver` | `driver=dual` | the ported windows (combat, invasion, strategy, agenda... through the driver) |
| `wide` | `driver,invasion,production,leaders,exploration,agenda=dual` | the wide mask of the gates; raw `area=dual,...` specs also work in `MASKS` |

A slice runs `dual_sharded.sh` with `SHARDS` (default 3) shards, i.e. at most 3 busy processes, at `nice -n 19`, `ionice -c3`.
It never sets `TI4_DUAL_FAIL_FAST`, so a game with a difference runs to its end and every difference (the first 200 per game)
is kept.

**Lock awareness.** The soak never holds `/root/.ti4-heavy.lock`. Before a slice (and before every round) it waits while the
lock is held (`flock -n` fails) or someone queues for it (a `->` waiter on the lock file's inode in `/proc/locks`). While a
slice runs it checks every `SOAK_POLL` (10) s; when a gate takes or queues for the lock the slice's whole process group is
`SIGSTOP`ped (no CPU; memory stays) and continued when the lock is free and unqueued. A slice that needs more than
`SLICE_TIMEOUT` (3600) active seconds is killed and counted as incomplete. `SOAK_HOLDS_LOCK=1` switches the checks off for a
one-off run that the caller wrapped in `flock /root/.ti4-heavy.lock` itself.

**Resume.** `/root/soak-reports/state.json` holds the finished slice ids, the next seed per lane, the round counter, the
totals and the difference/bail/window tables. A killed soak starts with the same plan (next seeds only advance when a slice
finished), so the interrupted slice is simply played again.

**Files.** `/root/soak-reports/YYYY-MM-DD/<lane>_<seed>_<games>_<mask>/` has `config.json` (the exact matrix),
`specs.jsonl` (the games), `merged.txt` (the merged `dual_run` text), `result.json` (per slice: calls, comparisons, per-area and
per-window counts, differences with game seed/map/factions/rules version/TE and a repro line, hash changes, silent windows),
and `shards/` only when the slice is evidence (a difference, a hash change, an incomplete slice). `slices.log` of the day has one
line per slice. `/root/soak-reports/LATEST.md` is the rolling summary an agent reads first (below).

### LATEST.md

Totals and a verdict (CLEAN / ATTENTION); the **differences** grouped by area and call site with the first example (game seed,
map, players, rules version, Thunder's Edge, factions, both sides of the first leaf paths) and a repro line; incomplete
slices; **silent bails**; calls and comparisons by mask and by lane; **bail counts per window label** (compared/bailed, worst
share first) and the **bail reasons** (`label [bail: reason]` rows of the dual report, by count); next seeds; the last 25 slices.

**Silent-bail detection** (the `compared == 0` idea of the lockstep tests, applied to the corpus): a window label that ran in a
slice and never compared (compared 0, bailed > 0; the `[bail: ...]` reason rows are excluded) is listed with the number of slices
it happened in; an area on the mask that made no call at all in a slice of at least 8 games is listed too. The bail table shows
the rest of the picture (a label that bails 100% of the time over the soak). Both only point; whether a bail is a documented one
(`ENGINE_KNOWN_ISSUES.md`) is for the reader. A real silent bail is a label that compares 0 on a corpus where the string engine
reaches it.

## 4. Repro: `tools/engine_core/soak_repro.sh`

Single game, full detail, no lock needed (about 10 s to a minute per game):

```
tools/engine_core/soak_repro.sh --line '<repro line from LATEST.md>'                # --seed S names the game
tools/engine_core/soak_repro.sh --record <slice>/result.json 0                      # 0-based difference of the slice
tools/engine_core/soak_repro.sh --record <slice>/result.json 0 --hash               # a hash change
tools/engine_core/soak_repro.sh --config C --games G --mask M --seed S [--failfast] [--nested]
```

It prints the game (index, seed, map, players, factions, the `selfplay replay` flags, bot overrides), then runs
`dual_run CONFIG GAMES MASK --seed S` (only that game; dual, then string) and prints every difference with both sides. The
config is the one stored in the slice directory, so the repro is independent of the soak's state. `--failfast` sets
`TI4_DUAL_FAIL_FAST=1` and `RUST_BACKTRACE=1`: the first difference panics at its call site. `dual_run --seed` needs the seed to
be unique in the config (the soak's lanes give every setup its own range, so it is); `--game I` otherwise.

## 5. Self-test

`tools/engine_core/test_soak.sh` (about 50 s, no engine): the corpus is deterministic and covers players 3-8, hyperlane maps,
Thunder's Edge on/off, rules versions 31-34 and all 24 lobby factions; a fake `dual_run` plays a soak round and the test
checks the state, LATEST.md, a planted difference with its repro line, silent-bail detection, that a held heavy lock pauses
and resumes a slice in flight, that a queued waiter makes the soak yield before starting, stop/resume, and that
`soak_repro.sh` parses a repro line.
