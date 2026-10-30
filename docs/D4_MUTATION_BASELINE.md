# D4 mutation baseline of the typed core (package D4-6, lane B)

Branch `core-d4b` (from `core-integration-4` @ `42e69e3c`), measured 2026-10-30 with `tools/engine_core/mutation_ratchet.sh` (a thin batch driver
over `tools/engine_core/mutants.sh`, `docs/ENGINE_COMPUTE.md` 1). **Tests used: the core's and the factions crate's own tests only**
(`cargo test --lib --tests` of the crate that is mutated: 307 + 29 inline/integration tests of `ti4-engine-core`, 87 of `ti4-engine-core-factions`).
The 4 200 tests of `ti4-engine` (string-engine unit tests, lockstep tests) and the bridge's adapter tests are NOT in these numbers: they exercise the core
only through the dual, and the pilot of `ENGINE_COMPUTE.md` shows the adapter tests kill about twice as much of `query/tech.rs` as the inline tests do
(52 % against 26 %). The numbers below are therefore the answer to "what do the core's own tests notice", which is what D4 must raise.

## 1. Result

Score = killed / (killed + survived); a timeout counts as killed (column `timeouts`; 61 of 2 514 killed mutants, most in the loop-heavy `action_cards` (20), `kernel` (14) and `containers` (9));
`unviable` points (the rewrite does not compile under the switch) are left out. `body kill` is the function-body kind (one mutant per function: a
function-coverage proxy). Wall = build + all mutants, `JOBS=4`, `CARGO_BUILD_JOBS=2`, `nice -n 10`, one module at a time on a loaded machine
(other agents' gates ran meanwhile); the whole baseline is **33 minutes of machine time** (1 960 s), plus the wait for `/root/.ti4-heavy.lock`
(the run queued behind a 3.5 h gate).

| module | points | killed | survived | unviable | score | body kill | timeouts | wall s |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| core/choice | 35 | 12 | 23 | 12 | 34% | 9/19 (47%) | 1 | 65 |
| core/containers | 288 | 217 | 71 | 4 | 75% | 81/109 (74%) | 9 | 118 |
| core/driver | 117 | 106 | 11 | 5 | 90% | 20/24 (83%) | 0 | 42 |
| core/kernel | 904 | 387 | 517 | 25 | 42% | 169/309 (54%) | 14 | 204 |
| core/keys | 34 | 15 | 19 | 6 | 44% | 2/5 (40%) | 0 | 49 |
| core/leaf | 422 | 49 | 373 | 2 | 11% | 15/83 (18%) | 2 | 100 |
| core/query | 1621 | 148 | 1473 | 15 | 9% | 56/456 (12%) | 0 | 252 |
| core/rules-top | 1460 | 222 | 1238 | 70 | 15% | 66/338 (19%) | 4 | 263 |
| core/rules/action_cards | 1164 | 354 | 810 | 12 | 30% | 59/106 (55%) | 20 | 289 |
| core/rules/agenda | 761 | 152 | 609 | 37 | 19% | 34/128 (26%) | 6 | 154 |
| core/rules/production | 861 | 118 | 743 | 45 | 13% | 33/161 (20%) | 5 | 181 |
| core/rules/tactical | 126 | 0 | 126 | 8 | 0% | 0/32 (0%) | 0 | 55 |
| core/state | 142 | 60 | 82 | 2 | 42% | 26/52 (50%) | 0 | 56 |
| factions/ca_combat | 217 | 4 | 213 | 3 | 1% | 4/84 (4%) | 0 | 18 |
| factions/cards | 63 | 51 | 12 | 1 | 80% | 10/11 (90%) | 0 | 18 |
| factions/cb_economy | 384 | 181 | 203 | 2 | 47% | 86/159 (54%) | 0 | 20 |
| factions/cc_ground | 370 | 131 | 239 | 7 | 35% | 56/125 (44%) | 0 | 21 |
| factions/cd_registry | 328 | 172 | 156 | 6 | 52% | 63/113 (55%) | 0 | 19 |
| factions/ce_movement | 261 | 110 | 151 | 4 | 42% | 48/91 (52%) | 0 | 19 |
| factions/top | 41 | 25 | 16 | 1 | 60% | 20/26 (76%) | 0 | 17 |
| **total** | 9599 | 2514 | 7085 | 267 | 26% | | 61 | 1960 |


Per D4 threshold group (todo 6.3), measured now against the initial threshold:

| group | modules | points | killed | score | threshold (initial / target) | body kill | body threshold |
|---|---|---:|---:|---:|---|---:|---|
| `query/*` | core/query | 1 621 | 148 | **9 %** | 60 / 75 | 12 % | 90 |
| `state/*`, `containers/*`, `keys/*`, `choice/*` | core/state, containers, keys, choice | 499 | 304 | **60 %** | 75 / 85 | 63 % | 90 |
| `rules/*`, `kernel/*`, `driver/*` | core/rules-*, kernel, driver | 5 393 | 1 339 | **24 %** | 60 / 75 | 34 % | 85 |
| factions crate | factions/* | 1 664 | 674 | **40 %** | 50 / 70 | 47 % | 85 |
| everything measured | | 9 599 | 2 514 | 26 % | 60 overall | 35 % | |

Not measured (owned by B12, int-5 not merged; todo 2.1 and trap T15): `rules/combat/*`, `rules/invasion/*`, `rules/reactions/*`, `kernel/ability.rs`
(`EXCLUDE` in `mutants.sh`: the baseline of `core/kernel` leaves `ability.rs` out). Add their rows to `tools/engine_core/d4/mutation_modules.tsv`
after int-5 and run `tools/engine_core/mutation_ratchet.sh core/rules/combat ...`.

Module names (`core/leaf` = the loose files of `src/`: `bits, board, galaxy, hex, ids, movement, tables, lib`; `core/rules-top` = the loose
files of `rules/` outside the sub-directories; `factions/top` = `lib.rs`, `common.rs`) are the keys of `tools/engine_core/mutation_floor.json`.

## 2. Floors (the ratchet)

`tools/engine_core/mutation_floor.json` holds the measured score rounded DOWN for every module above, so the ratchet starts green and only goes up:
`tools/engine_core/mutation_ratchet.sh MODULE...` runs the modules, compares, prints the survivors of a module below its floor and exits 1;
`UPDATE=1` raises floors (never lowers); `EXTRA_TEST_CRATES=ti4-engine-core-tests` adds the D4 kit crate's tests to the run (a mutant is killed when
any crate's tests kill it; the floors a D4-2x package raises are measured with its own tests, so it passes its list). `MODE=baseline` never fails.
The floors are far below the D4 thresholds on purpose: the thresholds are what D4-1/D4-2x work towards, the floor is "never worse than today".

## 3. Weakest modules (the list for lane A and the D4-2x packages)

By survivors: `core/query` 1 473 (0-33 % per file; `query/objectives.rs` 375 of 376 survive, `trade_deals.rs`, `reach.rs`, `trade.rs`, `transit.rs`,
`fleet.rs`, `laws.rs`, `board.rs`, `promissory.rs`, `relic.rs`, `explore.rs`, `diplomacy_items.rs` all 0-16 %; only `tech.rs` is at 33 %),
`core/rules-top` 1 238 (`diplomacy_book.rs`, `diplomacy.rs`, `diplomacy_window.rs`, `strategy_primaries.rs`, `trade.rs`, `explore.rs`, `explore_windows.rs`,
`leaders.rs`, `leader_windows.rs`, `relic_windows.rs`, `abilities.rs`), `core/rules/action_cards` 810 (`effects.rs` alone 634 of 895),
`core/rules/production` 743 (`mod.rs` 242 of 278, `strategy_cards.rs`, `window.rs` and `triggered.rs` 0-12 %), `core/rules/agenda` 609 (`table_effects.rs` 190 of 206,
`window.rs`, `reveal.rs`), `core/kernel` 517 (`fx.rs`, `fx_pay.rs`, `fx_move.rs` 0 %), `core/leaf` 373 (`movement.rs` 201 of 201, `galaxy.rs` 113 of 119, `board.rs` 15 of 15),
`core/rules/tactical` 126 of 126 (**0 % killed**, as the todo predicted), factions `ca_combat` 213 of 217 (**1 %**: the Bastion/Crimson query code plus the combat hooks),
`cc_ground`, `cb_economy` (`cabal/timing.rs` 66 of 67), `ce_movement`, `cd_registry`.

The three weakest by score among the big modules: `core/rules/tactical` 0 %, `factions/ca_combat` 1 %, `core/query` 9 %. The best: `core/driver` 90 %,
`factions/cards` 80 %, `core/containers` 75 %.

Weakest files (more than 20 survivors, by survivors):

| file | points | survived | score | body survivors |
|---|---:|---:|---:|---:|
| rules/action_cards/effects.rs | 895 | 634 | 29% | 31 |
| query/objectives.rs | 376 | 375 | 0% | 101 |
| rules/production/mod.rs | 278 | 242 | 12% | 50 |
| movement.rs | 201 | 201 | 0% | 32 |
| rules/agenda/table_effects.rs | 206 | 190 | 7% | 18 |
| rules/diplomacy_book.rs | 178 | 178 | 0% | 27 |
| rules/production/strategy_cards.rs | 198 | 174 | 12% | 29 |
| query/trade_deals.rs | 195 | 162 | 16% | 18 |
| rules/production/window.rs | 142 | 142 | 0% | 14 |
| rules/action_cards/mod.rs | 152 | 131 | 13% | 9 |
| query/reach.rs | 131 | 131 | 0% | 18 |
| query/trade.rs | 127 | 127 | 0% | 33 |
| query/tech.rs | 192 | 127 | 33% | 50 |
| rules/diplomacy.rs | 125 | 125 | 0% | 10 |
| rules/production/triggered.rs | 121 | 121 | 0% | 23 |
| rules/strategy_primaries.rs | 128 | 119 | 7% | 30 |
| rules/diplomacy_window.rs | 114 | 114 | 0% | 11 |
| rules/agenda/window.rs | 134 | 113 | 15% | 29 |
| galaxy.rs | 119 | 113 | 5% | 21 |
| rules/trade.rs | 108 | 106 | 1% | 32 |
| query/transit.rs | 106 | 106 | 0% | 20 |
| rules/agenda/mod.rs | 112 | 97 | 13% | 19 |
| kernel/fx.rs | 164 | 88 | 46% | 21 |
| rules/agenda/reveal.rs | 87 | 87 | 0% | 11 |
| rules/explore.rs | 83 | 83 | 0% | 27 |
| rules/explore_windows.rs | 95 | 81 | 14% | 18 |
| rules/leaders.rs | 132 | 80 | 39% | 24 |
| rules/leader_windows.rs | 130 | 80 | 38% | 13 |
| query/fleet.rs | 92 | 79 | 14% | 16 |
| kernel/fx_pay.rs | 78 | 78 | 0% | 18 |
| kernel/fx_move.rs | 69 | 69 | 0% | 26 |
| factions/cb_economy/cabal/timing.rs | 67 | 66 | 1% | 27 |
| query/laws.rs | 75 | 64 | 14% | 32 |
| query/diplomacy_items.rs | 61 | 61 | 0% | 13 |
| rules/relic_windows.rs | 55 | 55 | 0% | 12 |
| rules/agenda/effects.rs | 71 | 52 | 26% | 5 |
| query/board.rs | 57 | 52 | 8% | 28 |
| rules/abilities.rs | 50 | 50 | 0% | 22 |
| query/promissory.rs | 48 | 48 | 0% | 16 |
| factions/ca_combat/bastion/query.rs | 47 | 47 | 0% | 16 |

The complete survivor lists, one line per mutant (`file:line:col [fn] kind  original => mutated`, paths relative to the crate's `src/`), are committed per
module in `tools/engine_core/d4/mutation_survivors/<module>.txt` (824 KB in all): that is the starting worklist of each package's mutation pass
(todo 6.4 item 3). Regenerate with `tools/engine_core/d4/baseline_doc.py RESULTS_DIR --survivors-dir ...`.

## 4. What the survivors say (kill patterns, from reading the lists)

* `body` survivors dominate the weak modules (456 of 1 621 points in `query/*` are function-body mutants and 400 of them survive): the function is never
  exercised by a core test. These are the cheapest kills: one table test per accessor/query on a typed fixture kills the body mutant and usually
  several operator mutants in the same function.
* `query/*` is tested through the adapter/dual (the string engine is the oracle) and by nothing inside the core; `rules/tactical`, `rules/diplomacy*`,
  `rules/trade*`, `rules/explore*`, `rules/relic_windows`, `rules/strategy_primaries` have no inline tests at all (0 killed or 1-7 %): these are the D4-2a/2b windows
  with "0 tests" in the todo and the mutants confirm it.
* `core/containers` (75 %) survivors are mostly `set.rs` (28) and `deck.rs`/`opt32.rs`: boundary/ordering cases of the container API; D4-1 item 3.
* `core/driver` is the one rules module the core tests already hold (90 %; `driver/mod.rs` 47 % is the weak file: 10 survivors).
* `core/keys` 44 % / `choice` 34 %: tiny modules (34 and 35 points), `keys/mark.rs` 12 of 13 survive.
* Factions: the hook tests exercise Cabal/Nekro-style economy and registry code (35-52 %) but the combat hooks (`ca_combat`) are essentially untested in the crate
  (they were proven by the dual). That is D4-2e.
* Timeouts count as killed: `containers` (9) and `action_cards` (20) have the most; their true kill rate is a little lower than printed.

## 5. Measured costs (for later estimates)

| module | points | wall s |
|---|---:|---:|
| core/query | 1 621 | 252 |
| core/rules-top | 1 460 | 263 |
| core/rules/action_cards | 1 164 | 289 |
| core/kernel | 904 | 204 |
| core/rules/production | 861 | 181 |
| every other module | 50-760 | 17-154 |

About 0.2 s of wall per point at `JOBS=4` with a 1-3 s core test suite (the 336 core tests run in about 1.5 s per mutant), i.e. a full-core baseline is 30-35 minutes
of machine time, so the baseline can be re-run after every package; the ratchet runs only the modules a package touched. The build of the mutated copy is 35-60 s
cold per module (incremental: the scratch tree is reused between modules of one call). Disk: `$ROOT/mut/target` of a call is under 1.5 GB; the ratchet deletes it at the end of a call.

Estimated effort to the initial thresholds (basis: the 2 514 kills came from 336 tests = 7.5 kills per test on average, but the rule-window tests the D4-2 packages
write exercise much more code per test than the low-level tests that exist; 8-15 kills per test is the working figure): `query/*` +820 kills for 60 % = 55-100 tests;
`rules/kernel/driver` +1 900 kills = 130-240 tests; factions +160 kills = 15-20 tests; state group +120 kills. That is 300-400 tests, which agrees with the ~650-test
estimate of the todo for the full thresholds (function-body kill 85-90 % needs more than the 60 % score).

## 6. Caveats

* `mutants.sh` copies the tree with `rsync --checksum` and builds the scratch copy with `--offline --locked`; `Cargo.lock` must be consistent (this branch adds one
  dependency line for `serde` to `ti4-engine-core-bridge`, which the lock records).
* The rewriter does not mutate macro bodies (`accessors!`, `hook_group!`, `dual_window!` text): 267 points were unviable and generated accessors are not in the points at all.
  `state/*` has only 142 points for 3 690 lines for that reason: its accessor macros need table tests of their own (todo 6.4 item 5).
* Equivalent mutants are not yet marked `// mutants:skip`; the first package that raises a floor does that module by module.
* A second baseline with the kit crate's tests (`EXTRA_TEST_CRATES=ti4-engine-core-tests`) and one with the factions crate's tests against core mutants were not run
  (the kit crate is lane A's and not in this branch's base); the command is `EXTRA_TEST_CRATES=ti4-engine-core-tests tools/engine_core/mutation_ratchet.sh <modules>`.
