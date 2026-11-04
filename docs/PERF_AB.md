# Perf A/B: bot games on the legacy string engine vs the core (the `Game` wrapper)

Lane `core-perf-ab`, cut from `core-integration-23` (6d66acab5, deployed 54a15dba), 2026-11-04. RULES_VERSION 35, wire unchanged.

## Question and answer

Is it faster to run bot games on the core (the wrapper `Game` of `ti4-engine`, crates/ti4-engine-next, the rule bot as the table's decider)
than on the legacy string engine (`ti4-engine-legacy`)?

* **The engine alone: no, the core is slower: 3.9x to 5.7x per game** (same game, same answers; random-pick bot, after the two fixes below; 4.9x
  to 6.4x before them). Memory goes the other way: the core peaks at 27 to 38 MB, the string engine at 38 to 57 MB.
* **With the shipped rule bot (reads the view) 90 to 95 percent of a bot game was the bot, on both engines**, and the same bot cost on both.
  The cause was one hot spot in the thin crate: `Observed::seat` called `mahact_foreign_tokens`, which loaded the WHOLE position into a core view
  (`Full::of` -> `CoreView::of` -> `Codec::load`) for every seat sheet the bot read, for every faction, to answer 0 for all but the Mahact. A guard that
  asks the faction first (4 lines, in this branch, with a test) makes a rule-bot game **5.6x to 7.3x faster on the core and 7.9x to 12.1x faster on the
  string engine** (a 6-player game: 81 s to 11 s on the core, 82 s to 7 s on the string engine). Every decision of all 144 timed games is unchanged
  (chain hashes equal before and after).
* **After the fix, per decision with the rule bot, the core is 1.4x to 1.75x slower than the string engine**: the bot costs the same on both (0.9 to 1.1
  of the string engine's), the engine part is 3.0x to 4.6x slower on the core.
* A **second finding, not a speed one**: with the real (smart) bot the core stops with an error in 8 of the 24 games (list below). The random-pick
  `native_play` never reaches them.

## Method

* Binary `perf_ab` (new, `crates/ti4-engine-core-bridge/src/bin/perf_ab.rs`), ONE game per process so peak RSS (`VmHWM`, whole process including setup)
  is per game: `perf_ab <legacy|core> <random|record:F|replay:F> <players> <seed> [faction]`. Games are `lobby_spec("sol", players, seed)`: 3p, 4p, 5p, 6p x seeds
  1 to 6 (the seeds the `native_play` sweeps use), card set te, starting tech, no Thunder's Edge.
* `legacy`: `legacy_game(spec)` (the string `Game`, run under `AreaMask::STRING`, so no core area is switched on), stepped with `Game::step`. The thin decider is seated through
  the oracle's `ThinDecider` adapter (JSON conversion of each choice and the `Observed` view per decision; that adapter cost is inside the string engine's "engine"
  part, so the string engine's engine-only number is an upper bound).
  `core`: `spec.build()` state + galaxy in `Game::try_with_table` (the wrapper, what the server runs), stepped with `Game::step`.
* The same decider on both: the rule bot `ti4_rule_bot::choose_explained(choice, view, seed, seat, index)`, seeded by the game seed.
  * `random`: view `None` (the random pick of the rule bot only; what `native_play` plays). The two engines play the **identical** game in 19 of 24 seeds
    (same chain hash of every (player, prompt, answer)); the other 5 hit the documented string bugs (docs/STRING_ENGINE_BUGS.md) and differ in length.
  * `record`: the shipped rule bot with the view (`choose_seeing`). Smart games diverge between the engines in 23 of 24 seeds (the same known string/core differences,
    e.g. the first at decision 607 of 3p seed 1: the core asks "return a secret objective", the string does not, S5), so these rows compare per decision and per
    game mix, not game for game. Only 3p seed 4 is the identical game (before: string 10.3 s, core 11.5 s; after: 1.32 s and 1.93 s).
  * `replay`: each engine replays its OWN recorded smart game answering by option id from a file: no bot work, no view (the core's `wants_view` is false), so it is the engine's own
    cost on a real game's trajectory.
* Time is the wall time of the step loop only (setup, 5 to 15 ms, apart); `bot` = time inside the decider (includes the lazy view the bot reads). A "step" is one `Game::step`
  call; the core splits the string's glued steps (1165 core steps = 811 string steps for the same game), so compare per game and per decision, not per step.
* `cargo build --profile gate` (docs/ENGINE_BUILD_SPEED.md: no LTO, 16 codegen units; slower than release for cross-crate inlining, same code), `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`,
  private target dir. 1 process at a time, `/proc/loadavg` 1-minute value checked before each run (the sweep script `docs/perf_ab_data/sweep.sh` waits up to 10 minutes for a load below 1.5, then runs anyway and records `quiet_wait`).
* Cross-check with `native_play --quick` (NativeGame, the loop without the wrapper): 1165 steps in 1.1 s wall (fingerprint 74d5675ac754ac81); the same game through the wrapper `Game`
  with the random-pick bot: 1.73 s before the fixes and 1.47 s after (the wrapper adds the per-step mirror and the per-question view). The 2.9 s of the brief was not reproduced
  on an idle core (it presumably came from a loaded machine or a cold start).
* Raw data: `docs/perf_ab_data/before.jsonl` (baseline commit) and `after.jsonl` (with the fixes), one JSON line per run, incl. `load_before`, `load_after`.

### Load notes (honest)

The machine was **not idle**: another lane kept about one core busy the whole time (1-minute load 1.0 to 1.5 at every start). Every timed run started with load < 1.5
(max 1.49 before, 1.42 after; the script waited when it was higher) but 15 of the 144 baseline runs ended with a load above 1.5 (peak 2.7), 1 of the 144 after-runs (1.52). The machine
has 8 cores, so one other busy core mostly costs memory bandwidth and turbo clock, not a core; still read the numbers as +-5 to 10 percent. The ratios hold across all 24 seeds.
I could not get a fully quiet machine.

## Results

Per configuration, 6 games (seeds 1 to 6) summed. "s/game" is the wall time of the step loop, "bot s/game" the time inside the decider. Core games that stopped early (see below) count as played.

random-pick bot (no view), BEFORE the two fixes (the baseline commit 6d66acab5)

| players | engine | s/game | steps/game | decisions/game | ms/step | ms/decision | steps/s | bot s/game | peak RSS MB |
|---|---|---|---|---|---|---|---|---|---|
| 3 | legacy | 0.18 | 533 | 764 | 0.34 | 0.24 | 2911 | 0.00 | 38 |
| 3 | core | 0.90 | 796 | 743 | 1.12 | 1.21 | 889 | 0.00 | 27 |
| 4 | legacy | 0.30 | 762 | 1097 | 0.40 | 0.28 | 2504 | 0.00 | 42 |
| 4 | core | 1.67 | 1110 | 1097 | 1.50 | 1.52 | 666 | 0.00 | 31 |
| 5 | legacy | 0.32 | 703 | 1016 | 0.46 | 0.32 | 2187 | 0.00 | 42 |
| 5 | core | 1.87 | 1073 | 1012 | 1.74 | 1.85 | 573 | 0.00 | 29 |
| 6 | legacy | 0.44 | 860 | 1270 | 0.51 | 0.34 | 1962 | 0.00 | 44 |
| 6 | core | 2.73 | 1286 | 1258 | 2.12 | 2.17 | 471 | 0.00 | 32 |

random-pick bot (no view), AFTER the fixes

| players | engine | s/game | steps/game | decisions/game | ms/step | ms/decision | steps/s | bot s/game | peak RSS MB |
|---|---|---|---|---|---|---|---|---|---|
| 3 | legacy | 0.19 | 533 | 764 | 0.36 | 0.25 | 2761 | 0.00 | 39 |
| 3 | core | 0.75 | 796 | 743 | 0.95 | 1.01 | 1057 | 0.00 | 27 |
| 4 | legacy | 0.32 | 762 | 1097 | 0.41 | 0.29 | 2415 | 0.00 | 43 |
| 4 | core | 1.39 | 1110 | 1097 | 1.25 | 1.27 | 797 | 0.00 | 30 |
| 5 | legacy | 0.31 | 703 | 1016 | 0.44 | 0.30 | 2291 | 0.00 | 43 |
| 5 | core | 1.68 | 1073 | 1012 | 1.57 | 1.66 | 639 | 0.00 | 29 |
| 6 | legacy | 0.44 | 860 | 1270 | 0.51 | 0.34 | 1965 | 0.00 | 45 |
| 6 | core | 2.47 | 1286 | 1258 | 1.92 | 1.96 | 520 | 0.00 | 32 |

replay of recorded answers (no bot work at all), BEFORE

| players | engine | s/game | steps/game | decisions/game | ms/step | ms/decision | steps/s | bot s/game | peak RSS MB |
|---|---|---|---|---|---|---|---|---|---|
| 3 | legacy | 0.53 | 1106 | 1504 | 0.48 | 0.35 | 2091 | 0.00 | 46 |
| 3 | core | 1.74 | 1164 | 1241 | 1.49 | 1.40 | 670 | 0.00 | 29 |
| 4 | legacy | 0.68 | 1323 | 1845 | 0.51 | 0.37 | 1953 | 0.00 | 48 |
| 4 | core | 2.62 | 1437 | 1618 | 1.82 | 1.62 | 549 | 0.00 | 32 |
| 5 | legacy | 0.98 | 1571 | 2152 | 0.62 | 0.46 | 1603 | 0.00 | 51 |
| 5 | core | 3.92 | 1645 | 1773 | 2.38 | 2.21 | 419 | 0.00 | 32 |
| 6 | legacy | 1.33 | 1950 | 2767 | 0.68 | 0.48 | 1471 | 0.00 | 54 |
| 6 | core | 6.77 | 2298 | 2614 | 2.94 | 2.59 | 340 | 0.01 | 37 |

replay of recorded answers (no bot work at all), AFTER

| players | engine | s/game | steps/game | decisions/game | ms/step | ms/decision | steps/s | bot s/game | peak RSS MB |
|---|---|---|---|---|---|---|---|---|---|
| 3 | legacy | 0.55 | 1106 | 1504 | 0.49 | 0.36 | 2024 | 0.00 | 46 |
| 3 | core | 1.47 | 1164 | 1241 | 1.27 | 1.19 | 790 | 0.00 | 29 |
| 4 | legacy | 0.67 | 1323 | 1845 | 0.51 | 0.36 | 1976 | 0.00 | 49 |
| 4 | core | 2.31 | 1437 | 1618 | 1.61 | 1.43 | 622 | 0.00 | 32 |
| 5 | legacy | 0.97 | 1571 | 2152 | 0.62 | 0.45 | 1624 | 0.00 | 52 |
| 5 | core | 3.56 | 1645 | 1773 | 2.16 | 2.01 | 462 | 0.00 | 33 |
| 6 | legacy | 1.30 | 1950 | 2767 | 0.67 | 0.47 | 1501 | 0.00 | 54 |
| 6 | core | 6.15 | 2298 | 2614 | 2.68 | 2.35 | 374 | 0.01 | 37 |

rule bot reading the view (the shipped bot), BEFORE

| players | engine | s/game | steps/game | decisions/game | ms/step | ms/decision | steps/s | bot s/game | peak RSS MB |
|---|---|---|---|---|---|---|---|---|---|
| 3 | legacy | 20.77 | 1106 | 1504 | 18.77 | 13.81 | 53 | 20.12 | 49 |
| 3 | core | 17.02 | 1164 | 1241 | 14.63 | 13.71 | 68 | 15.21 | 31 |
| 4 | legacy | 29.77 | 1323 | 1845 | 22.50 | 16.14 | 44 | 28.97 | 53 |
| 4 | core | 27.50 | 1437 | 1618 | 19.14 | 16.99 | 52 | 24.74 | 33 |
| 5 | legacy | 46.90 | 1571 | 2152 | 29.85 | 21.80 | 34 | 45.73 | 53 |
| 5 | core | 40.27 | 1645 | 1773 | 24.48 | 22.71 | 41 | 36.19 | 34 |
| 6 | legacy | 81.75 | 1950 | 2767 | 41.91 | 29.55 | 24 | 80.17 | 56 |
| 6 | core | 81.28 | 2298 | 2614 | 35.37 | 31.10 | 28 | 74.34 | 38 |

rule bot reading the view (the shipped bot), AFTER

| players | engine | s/game | steps/game | decisions/game | ms/step | ms/decision | steps/s | bot s/game | peak RSS MB |
|---|---|---|---|---|---|---|---|---|---|
| 3 | legacy | 2.64 | 1106 | 1504 | 2.39 | 1.76 | 419 | 2.02 | 50 |
| 3 | core | 3.02 | 1164 | 1241 | 2.60 | 2.43 | 385 | 1.47 | 33 |
| 4 | legacy | 3.19 | 1323 | 1845 | 2.41 | 1.73 | 415 | 2.41 | 54 |
| 4 | core | 4.67 | 1437 | 1618 | 3.25 | 2.88 | 308 | 2.29 | 35 |
| 5 | legacy | 4.60 | 1571 | 2152 | 2.93 | 2.14 | 341 | 3.50 | 56 |
| 5 | core | 6.42 | 1645 | 1773 | 3.90 | 3.62 | 256 | 2.80 | 34 |
| 6 | legacy | 6.75 | 1950 | 2767 | 3.46 | 2.44 | 289 | 5.27 | 57 |
| 6 | core | 11.19 | 2298 | 2614 | 4.87 | 4.28 | 205 | 4.80 | 38 |

ratio core/legacy per decision, rule bot, after:
3 total 1.38 bot-only 0.88 engine-only 3.03
4 total 1.67 bot-only 1.08 engine-only 3.52
5 total 1.69 bot-only 0.97 engine-only 3.98
6 total 1.75 bot-only 0.96 engine-only 4.56
speedup before->after (rule bot, ms/decision):
legacy 3 7.9x
legacy 4 9.3x
legacy 5 10.2x
legacy 6 12.1x
core 3 5.6x
core 4 5.9x
core 5 6.3x
core 6 7.3x


### Same game, engine only (random-pick bot, games where both engines play the identical game)

| players | games | string ms/game | core ms/game before | ratio | core ms/game after | ratio |
|---|---|---|---|---|---|---|
| 3 | 5 | 181 | 893 | 4.9x | 758 | 3.9x |
| 4 | 6 | 304 | 1666 | 5.5x | 1392 | 4.4x |
| 5 | 5 | 314 | 1901 | 6.1x | 1697 | 5.6x |
| 6 | 3 | 436 | 2793 | 6.4x | 2504 | 5.7x |

(The string column is the "before" run; it moves by under 5 percent between the two sweeps, the random mode never touches `Observed`.)

### Core stops with the smart bot (not part of this lane to fix)

8 of 24 core games stop on a step error with the rule bot reading the view (the string engine finishes all 24): 4p seed 4 and 5p seed 4 `aftermath: MissingFact`; 4p seed 5
`tactical: UnportedTactical(NovaSeedCargo)`; 6p seed 3 `the position cannot be shown: Board: planet entry in an untouched system "atlas"`; 5p seed 5 and 6p seeds 4, 5, 6 not decoded one by one. The
random-pick `native_play` sweeps do not reach these. Reproduce: `perf_ab core smart 4 4` (prints `step error at step N: ...` on stderr).

## Where the time goes (profile, `perf record --call-graph fp`, gate profile + line tables + frame pointers, a private build)

Core engine alone (replay of 4p seed 6, 2.7 s before, 2.3 s after), inclusive share of the run:

| share (before -> after the fixes) | what |
|---|---|
| 34 -> 39 % | `Native::materialized`: the core position mirrored into a `GameState` after EVERY step (`step_native`), of which dropping the old `GameState` is 8 -> 9 % |
| 18 -> 21 % | `env_with_plays` / `env_for` every step: `AbilityTable::registry` 12 -> 15 % (specs x seats, ids built with `format!` and interned through `Render::code`, 9 -> 10 %), the facts, the roster |
| 13 -> 1.5 % | `combat_texts`: the space cannon scan asked `planets_of` of all 231 systems, which scans all 159 planets each time (231 x 159 cell reads per step). FIXED (below) |
| 7 -> 8 % | `TableAsker::ask`: `materialize` + `Observed::new` per question even for a decider that does not want the view |
| 25 -> 25 % | the engine proper (`live_step`, `live_reconcile`, windows, the resolver) |
| 4 -> 4 % | `GameState::clone` (the string-side data the mirror copies) |
| under 1 % | `CoreState::clone` (the two clones per step the brief suspected: the core's persistent chunks make them cheap) |

Core with the rule bot (3p seed 4, 11.5 s before): **87 % in `choose_seeing`, 77 % of the run in `Observed::seat` -> `mahact_foreign_tokens` -> `Full::of` -> `CoreView::of` -> `Codec::load`**
(plus `IdStr::from_id`, `BTreeMap` iteration). After the guard (4p seed 2): the bot is 49 % of the run, of which the biggest remaining parts are again whole-position loads per query:
`production::available` -> `ProdView::of` 17 %, `revealed_objective_progress_imagining` -> `Seen::facts` 13 %, `backward::prep::tech_preps` 17 %; `Codec::load` is still 18 % of the run.
The engine side (mirror 18 %, env 10 %, drop 5 %) is as above.

## What was changed (kept in this branch; both keep every fingerprint)

1. `queries::factions::mahact_foreign_tokens` (crates/ti4-engine-next/src/queries/factions.rs): return 0 at once unless the player's faction is `mahact`. The hook answers 0 for everyone else
   anyway, so the answer is the same; it spares one whole-position core load per `Observed::seat`. Test `the_mahact_foreign_tokens_shortcut_answers_what_the_hook_answers` (a Mahact table, every seat, every 40th step,
   against the hook). Effect: rule-bot games 5.6x to 12.1x faster (table above), both engines, because the thin `Observed` is shared.
2. `combat_live::space_cannon_kinds` (crates/ti4-engine-next/src/ties/combat_live.rs): one pass over the system and planet cells instead of `planets_of_sorted` for each of the 231 systems. The old scan is kept as
   `space_cannon_kinds_by_systems`, the oracle of the test `the_space_cannon_scan_of_the_combat_texts_matches_the_scan_by_systems` (also asserts every planet prints the system that lists it). Effect: core engine alone
   -14 to -17 % (random 4p seed 1: 1.73 s to 1.47 s; replay 4p seed 6: 2.67 s to 2.29 s).

Checks: quick game 1165 steps fingerprint 74d5675ac754ac81 unchanged (`native_play --quick`; `wrapper_native` 9 of 9 tests pass, including the pin); chain hashes of all 144 games identical before/after
(`before.jsonl` vs `after.jsonl`); the two new tests pass (`cargo test --profile lockstep -p ti4-engine-core-bridge --lib`, 2 tests). The heavy gates (Z-gate etc.) were not run (brief: no heavy gates); neither change
touches a rule: one is a pure early return in a view query, the other a re-ordering of a read-only scan whose result is sorted and deduplicated.

## Conclusion and what to optimize first

1. Done: the 4-line Mahact guard (the bot was 90 % of a bot game because of it; benefits the string engine, the core, the server's bot seats and every bot self-play run).
2. **The per-step mirror** (`native.materialized()` into `self.state`, 25 to 39 % of the core's own cost): make the mirror lazy (materialize on the first read of `game.state` after a step; the host reads it once
   per step at most) or incremental by slice. Likely the single biggest engine win (about 1.5x on the engine alone), but it changes when `Game.state` is current, so it needs a look at all readers.
3. **`env_for` per step** (15 to 21 %): the ability registry and its text ids are rebuilt from the specs for every step; cache them per (kernel, seating, the facts `applies.holds` reads) or at least build the ids
   without `format!` and without re-interning. Needs an invalidation story, so not done here.
4. **The bot's whole-position loads** (still 18 % of a rule-bot game): memoize one `CoreView`/`Full` per `Observed` (a `OnceCell`) so `ProdView::of`, `ObjView`, `Seen::facts` and the other queries stop calling
   `Codec::load` per query; the same pattern as the Mahact fix, a bigger lever than any engine change for bot self-play (the bot is 49 % of the run after fix 1).
5. `TableAsker::ask` builds `materialize` + `Observed::new` for every question even when `Table::wants_view(player)` is false (a human seat, the random bot): about 7 %. Not safe to skip as is, because
   `ask_seeing` also attaches `about_invasion` and `details.agenda` to the journaled choice; it needs the same enrichment on the no-view path first.
6. The two core clones per step (`before = core.clone()` in `Game::step_native` and `NativeGame::step`) are under 1 %: not worth touching.

Bottom line for the question: the core is not faster than the string engine for bot games; it is 1.4x to 1.75x slower per decision with the rule bot and about 4x to 6x slower engine-only, with the lower memory. The
bigger speed lever was never the engine: it was the bot's view code (fixed: 6x to 12x). Items 2 to 4 are where the remaining core time is; I did not measure what they would bring in total (item 2 alone is 25 to 39 % of the engine part).
