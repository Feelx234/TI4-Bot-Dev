# core-integration-30 (2026-11-07)

Branch `core-integration-30`, cut from `core-integration-29` @ `437ca1bd9`. Not pushed, not deployed; the live checkout and its branch were
not touched. `RULES_VERSION` stays 36; `BOT_VERSION` unchanged. Logs and scripts: `/root/deploytmp/i30-logs/`.

## 1. Merges (in this order, `--no-ff`, each its own commit)

| # | branch (tip merged) | merge commit | what |
|---|---|---|---|
| 1 | `core-multiask-1-fix` (`07ba85255`) | `98417e72d` | the rest of multi-ask (descendant of `7726d0d6d`, which int-29 had): legacy lockstep duals collapsed (`multiask_dual.rs`), Titan Prototype for a Minister of Industry builder, combat texts (Mentak hero forms, Z-Grav Naaz mech), payment lead atom, slow web tests re-pinned, bench baselines with victories only, shadow S5 / H3; `docs/MULTIASK1_RESUME.md` 7-8 |
| 2 | `core-search-api-1` (`8a32981b2`) | `dac75d626` | C6/C7: crate `ti4-engine-search`, thin `Render::fork_warm` (feature `search`), core `Sx::dice_on` / `die_on` (combat chance requests carry their threshold); `docs/C6_RESUME.md` |
| 3 | `core-determinise-1` (`0d8274e79`) | `2fd7b8c78` | C8: `determinize` in `ti4-engine-search` only; `docs/C8_RESUME.md` |
| 4 | `core-mcts-core-1` (`3842c3535`) | `9c9228115` | L6: crate `ti4-mcts` (Gumbel root, max^n, chance nodes, tree reuse, `MctsAgent`); `docs/L6_RESUME.md` |
| 5 | `core-mcts-prep-2` (`dc13cad3c`) | `c487c3d25` | L2/L4/L7: crate `ti4-mcts-net` (forward pass, `.t4w`), `mcts_dataset`, `mcts_eval`, the lab's agents / eval, `tools/mcts`; core read-only `Seen::movable_counts`, `SeatReach::retarget`, `SeatFleet::movable_count`; `docs/MCTS_PREP2_RESUME.md` |

Merges 2-4 are a stack (search-api -> determinise -> mcts-core): each merge brought only its own commits. Merges 2, 3, 4 had no conflict.

## 2. Conflicts

| merge | file | the two sides | resolution |
|---|---|---|---|
| 1 | `bridge/src/bin/native_play.rs` (`--twice`) | int-29: the second run through `shadow_play` when shadowing; multi-ask-fix: `NativeGame { shadowing }` + `play_game` | multi-ask-fix's (its lane's; same game, `7824cabdfbb528b6` twice) |
| 1 | `ti4-server/tests/it/replay_export.rs` | int-29 62, multi-ask-fix 63 | 63, re-scanned on int-30 with the ignored probe: 64, 65 differ, 56-63 and 66-70 agree (`ad114bfde`) |
| 1 | `crates/ti4-wasm/size-budget.json` | 6,661,206 vs 6,660,000 | int-29's, then re-measured (section 3) |
| 1 | `scripts/wasm-size-gate.mjs` | two history comments | both |
| 5 | `Cargo.toml` members | `ti4-mcts` vs `ti4-mcts-net` | both |
| 5 | `ti4-mcts-lab/src/lib.rs` | int-29 dropped `use std::rc::Rc` (chance-2 removed `TextSeeds`); prep-2 added `pub mod agents; pub mod eval;` | the two modules, no `Rc` (nothing uses it) |
| 5 | `tools/engine_core/check_production_graph.sh` | dev-tool list + tree check: `ti4-engine-search ti4-mcts` vs `ti4-mcts-net` | all three |

Semantic checks: the workspace (`--all-targets`, also `ti4-engine --features search`) compiled after the five merges with no change: prep-2's lab code
already used the int-29 API (`env_with_plays` without seeds). No new `Tape::dice` caller came with multi-ask-fix (C6's join note); multi-ask-fix's
changes to `live/mod.rs` are in the legacy crate, not in the thin step functions C6's `position.rs` copies (`tests/lab.rs` and the search crate's
quick-game test pass).

## 3. Wasm

| build (real `build-wasm.mjs`, binaryen wasm-opt) | raw B | gzip B | brotli B |
|---|---:|---:|---:|
| int-29 merged | 6,661,206 | 2,120,780 | 1,524,274 |
| multi-ask-fix alone (its doc 8.5) | 6,658,414 | 2,122,423 | 1,527,816 |
| **int-30 merged** (`/root/deploytmp/i30-logs/ti4_wasm.merged.wasm`) | **6,665,523** | **2,122,498** | **1,525,693** |
| budget before | 6,661,206 | 2,126,000 | 1,532,000 |
| **new budget** (`a6bafa884`) | **6,665,523** | 2,126,000 | 1,532,000 |

Over by 4,317 B raw (gzip and brotli under). Expected from the parts: multi-ask-fix's own fixes +2.9 KB (6,655,547 -> 6,658,414 on its line) and
C6's hit threshold about +1.1 KB (its doc). Within the approved 5 KB, so raw is raised by exactly the overrun; no trims tried. Output probes: no
legacy literal. The MCTS / search crates are not in the wasm (production graph strict: 0 violations).

## 4. Re-baseline (once, combined line)

| pin | int-29 | multi-ask-fix alone | **int-30** |
|---|---|---|---|
| quick game (`native_play --quick --twice`) | `fd54dc6c956758bc` 1,110 | `4f79d5630eaf630d` 1,110 | **`fd54dc6c956758bc`, 1,110 steps, unchanged**, deterministic |
| quick game shadowed (`--quick --shadow --twice`) | `7824cabdfbb528b6` | `efcb0318f94f6606` (unshadowed spec) | `7824cabdfbb528b6`, deterministic, ZERO GAP, compared 807 |
| perf_kernel suite 1 | `31fe4e2bdd62069f` (NOSTATE `6881f4a9caf01d02`) | `2c99ed238f39713e` | **`31fe4e2bdd62069f` unchanged** (NOSTATE `6881f4a9caf01d02`), 13,751 steps, 12,706 decisions, 297 chance requests / 384 faces |

The hit-threshold commit (`82b356bb0`) moves no hash (measured): the threshold rides on the request and the answer never reads it, so the draws and
every decision stay the same. multi-ask-fix's fixes touch no position of these games. Pins kept, reasons added to the pin comments
(`496440d02`: `native_play.rs`, `wrapper_native.rs`, `fork_mid.rs`, `perf_kernel.rs`, `docs/PERF_KERNEL.md`).

Goldens: `golden diff v36-std` and `golden diff ladder`: every game reproduces; `golden check` 200 games, 146,690 decisions, 0 failed under the
default mask, `all=core` and `all=string`. Nothing blessed.

Bench (`verify_benches.sh`, the int-30 gate selfplay, 6 workers, `/root/deploytmp/i30-bench`): 154 / 158 / 170 / 162 / 166 / 161 identical
(bench, v1..v5), **229 entries moved, all 1,200 games `victory`** (no `engine_error`). multi-ask-fix recorded its baselines without chance-2
(unbiased Spy / Executive Sanctions). Patched with `/root/deploytmp/i28-logs/patch_bench.py . /root/deploytmp/i30-bench --write` (`afd5132ae`,
table `/root/deploytmp/i30-logs/bench-moved-table.md`); re-verified with the same binary: **200 identical, 0 different, six times**.

Bridge pins (multi-ask's ten re-pinned `native_play.rs` tests and the new first-difference test): bridge lib 153 passed under the lockstep
profile; no re-pin needed.

Shadow sweep `native_play --factions all --players 3,4,5,6 --shadow` (gate): 96 games, 96 finished, 0 gaps, **0 first differences**, 24 at a known
string bug (S1 / S3 / S5; multi-ask-fix alone 21, int-29 22; with chance-2 these are other games).

## 5. Own commits

| commit | what |
|---|---|
| `670d48f0e` | `end_of_d1.sh` layers step and `verify_package.sh` run every `check_layers.py` mode (core, thin, setup, chance, resolver, search, mcts); `clippy_core.sh` covers `ti4-mcts-net` |
| `31c567298` | the net evaluator wired into `ti4-mcts` (section 6) |
| `afd5132ae` | bench baselines |
| `a6bafa884` | wasm budget |
| `496440d02` | pin reasons |
| `ad114bfde` | server `replay_export` count comment (63 kept) |
| (docs) | this file, `docs/I30_RESUME.md`, `docs/HEAVY_RUN_PLAN.md` budget line |

`lint_core.sh` is the core hygiene lint (no `String`, `format!`, serde in the core crates); the search and MCTS crates are dev tools that use
`String` errors and JSON by design (117 / 61 / 34 hits if pointed at them), so they are not added to its default roots; their layer rules are
`check_layers.py search` / `mcts` (0 violations) and they are in `clippy_core.sh` and the production-graph dev-tool list.

## 6. The net evaluator (L6 + L4 join)

`ti4_mcts::eval::NetEvaluator` (and `EvalSpec::Net`, `--eval net:PATH` for `mcts_ab` / `mcts_bench`): encode the actor's `Seen` with the lab's
`Encoder`, round to f16 as the lab's greedy agent does, one forward pass, the raw option logits of the candidates, the softmax of the win head.
**Seat order**: the net's value is relative to the seating order (`encode::relative_seats`: the actor, then clockwise), the MCTS crate's to the seat
index (`(actor + k) % 4`). The evaluator puts the probabilities back on their absolute seats and then makes them actor-relative, so the two need not
agree; in the MCTS setup they do (the new test asserts both). `EvalSpec` lost `Copy` (holds an `Arc<Net>`), `play_game` takes `&EvalSpec`.
Test `tests::the_net_evaluator_maps_logits_and_the_seat_order_and_drives_a_search` (tiny fixture of `ti4-mcts-net`, three real positions, a 16-sim
search keeps its invariants and the game). `ti4-mcts`: 14 unit + 4 game tests pass (gate profile).

Smoke `mcts_ab --seeds 4 --eval net:/root/deploytmp/mcts-data/run-v1/weights.t4w --gumbel 0`: 4 games end, 502 searches, 1.3 s per game (plumbing).
Greedy net vs rule bot (`mcts_eval --agent policy:run-v1/weights.t4w --seeds 48 --first 900000 --agreement`): 240 games, **0 errors**, win share
25.8 % (Wilson 20.1-32.4 %), paired delta win share +0.8 +- 7.9 pts, agreement 81.8 %, encode 187 us / forward 589 us (loaded box).
(`mcts_eval` does not take an `MctsAgent` yet; that is the next step of L7.)

## 7. prep-2's P3 engine error (`ask: Live(Unknown(TurnGap(StrategyAbility)))`, seeds 900139, 900168, 900235)

**Fixed by multi-ask-fix's Titan Prototype commit `ed35c20a7`** (the relic read the builder's structure rules with `structure_facts`, which refuses a
Minister of Industry holder). Confirmed:

* prep-2's binary (`/root/deploytmp/mcts-data/bin/mcts_eval`), seed 900168: the baseline game ends in that error; int-30's: all 3 seeds x 5 games, 0 errors.
* int-30 with only that fix reverted (`structure_facts_with(.., false)`, a probe build, not committed): seed 900235 with the agent at place 3 ends in
  the same error again (900139 / 900168 play other games on the merged line and do not reach the relic then).

The regression test is multi-ask-fix's `ti4-engine-core-tests` `action_dispatch::the_titan_prototype_builds_for_a_minister_of_industry` (failed with
`Unknown(TurnGap(StrategyAbility))` before; passes here). No lab-level game test was added: the reproducing game needs the uncommitted
`run-v1` weights.

## 8. Checks

| check | result |
|---|---|
| `cargo check --workspace --all-targets` (without review, replayer, mlp, tensor, advisor), `ti4-engine --features search` | green |
| `check_layers.py` core / thin / setup / chance / resolver / search / mcts | 576 / 193 / 14 / 5 / 1 / 12 / 12 files, 0 violations |
| `check_production_graph.sh --self-test` and strict | ok; 0 violations, 0 failed controls |
| `clippy_core.sh` (now with `ti4-mcts-net`) | ok, engine warnings 41 = baseline |
| `lint_core.sh` | 578 files, 0 hits, 6 allowed, 0 multi-ask guard hits |
| lockstep: core, factions, core-tests, search, mcts-features, mcts-lab, mcts-net | 2,772 passed, 0 failed |
| lockstep: `ti4-engine-legacy --lib` | **5,353 passed, 0 failed**, 7 ignored (int-29: 155 failed) |
| lockstep: bridge (lib, tests, bins) | 259 passed, 0 failed |
| gate: `ti4-mcts` | 18 passed |
| fast web tier (`t-web` package set, debug) | 1,678 passed, 1 failed: `session_deterministic_replay::deterministic_replay_produces_identical_decision_logs`, the known timing test (lost the race while the bench ran; alone 3 of 3 pass) |
| slow web tests (debug, `--test it ::slow_`) | ti4-server 25 passed, ti4-wasm 10 passed (int-29: 3 failed, fixed by multi-ask-fix) |
| golden diff / check | section 4 |
| bench | 1,200 victories, verify 200 identical x6 |
| shadow sweep | 96 games, 0 gaps, 0 first differences |
| wasm size gate | ok with the new budget |

Not run (by the brief): `end_of_d1.sh`, soak / strict / census, the mutation ratchet, dual-all; web `tsc` / `vitest`.
