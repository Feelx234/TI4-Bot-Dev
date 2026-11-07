# core-integration-29 (2026-11-06/07)

Branch `core-integration-29`, cut from `core-integration-28` @ `f22ece5e8` (also the live branch tip). Not pushed, not deployed; the live
checkout and its branch were not touched. `RULES_VERSION` stays 36; `BOT_VERSION` unchanged. Logs and scripts: `/root/deploytmp/i29-logs/`.

## 1. Merges (in this order, `--no-ff`, each its own commit)

| # | branch (tip merged) | merge commit | what |
|---|---|---|---|
| 1 | `core-chance-2` (`3b38797df`) | `563f38a1b` | randomness as a question: `Ask::Chance` / `Pick::Chance`, resolver crate `ti4-engine-chance`, the core's streams are `ti4_chance::Streams`, unbiased Spy / Executive Sanctions (`Index { n: held }`), fork_core C4 tests; `docs/CHANCE2_RESUME.md` |
| 2 | `core-search-thin-1` (`2a99981d9`) | `3e173de7b` | C5: `DriverFlags::AFTER_GLUE`, thin `Game::pull` / `answer` / `Mid` behind feature `search`, `tests/fork_mid.rs`; `docs/C5_RESUME.md` |
| 3 | `core-mcts-prep-1` (`6e2ac1488`) | `40c7ee5b7` | new dev-tool crates `ti4-mcts-features`, `ti4-mcts-lab`, `tools/mcts`; `docs/MCTS_PREP1_RESUME.md` |
| 4 | `core-multiask-1` **as committed on its working branch `core-multiask-1-rebase` @ `7726d0d6d`** | `9ca0c66e4` | five multi-pick conversions + the Leadership "buy nothing" default, the fleet-removal fix, the journal naming the picked option, multi-ask's own goldens; `docs/MULTIASK1_RESUME.md` |

Not merged (by the brief): `core-mcts-prep-2` (still being written).

**Multi-ask is not finished.** `core-multiask-1` itself stops at `e057d2496` (21:27 UTC, re-baseline not done, wasm over); its agent works on
`core-multiask-1-rebase` (a plain continuation, `core-multiask-1` is its ancestor), last commit `7726d0d6d` at 22:44 UTC, minutes before the merge:
goldens and pins re-done for multi-ask alone, bench not re-recorded, `MULTIASK1_RESUME.md` not updated. I merged that tip (a superset of
`core-multiask-1` with the fixes) so the combined line has the Leadership decline and the removal fix; re-merge when told.

Since the merge the working branch moved on (7 commits up to `a02bb7f4b`, 00:01 UTC): `bc578f192` (shadow S3), `80a114782` (its own budget
commit, 6,660,000 / 2,126,000 / 1,532,000), `2f573b4f1` / `d72d53ba2` (replay_export probe, the in-progress table at 63), `807a02934`
(screenshots), `bf6735291` (payment lead atom), `a02bb7f4b` (the same `--shadow --twice` fix as mine). Expected conflicts at the re-merge:
`crates/ti4-wasm/size-budget.json` (keep int-29's 6,661,206 unless a new measurement says otherwise), `bin/native_play.rs` (same fix, take
one), `ti4-server/tests/it/replay_export.rs` (the answer count must be re-scanned on the combined line with the ignored probe), the pins and
g040 (re-baseline again).

## 2. Conflicts

| merge | file | the two sides | resolution |
|---|---|---|---|
| 2 | `ti4-engine-next/src/game/mod.rs` (`Native::new`, `NativeSnapshot::open`) | chance-2 dropped `seeds: Rc<TextSeeds>`; C5 dropped `after_glue`, added `#[cfg(feature = "search")] mid` | no `seeds`, no `after_glue`, `mid` kept |
| 2 | `ti4-engine-next/src/game/start.rs` (`Game` start) | same | same |
| 2 (semantic) | `live/mod.rs` `pull::{begin, answer}` (C5's search copy of `live_step_inner`) | chance-2 routes every arm call through `ti4_engine_chance::advance_resolved` and resolves a chance question left pending by a search | the same in both `pull` functions; so no chance question ever reaches `pull::answer` (no chance entry needed there) |
| 2 (semantic) | `game/mod.rs` `step_env` (C5's search copy) | `env_with_plays(.., n.seeds.clone(), ..)` | without `seeds`, as chance-2's `native_step` |
| 2 (pin) | `tests/fork_mid.rs` | pinned the int-28 quick game `538dc68f23dddcc9` | `43c985a35397f2c9` (chance-2), later `fd54dc6c956758bc` (`7d7944942`, `e68b52a74`) |
| 3 (semantic) | `ti4-mcts-lab/src/lib.rs` | `Table.seeds: Rc<TextSeeds>`, `env_with_plays(.., seeds, ..)` | dropped (chance-2 removed `TextSeeds`); the lab steps through `live::live_step` / `live_reconcile`, which go through the resolver |
| 4 | `ti4-engine-core/src/driver/live.rs` `is_option` | chance-2: match with a `Pick::Chance` arm; multi-ask: `choice.accepts(pick)` | multi-ask's `accepts`; `Choice::accepts` (choice/mod.rs) gains the `Pick::Chance` arm (`Request::accepts`) |
| 4 | `ti4-engine-core/src/driver/mod.rs` `Driver::resume` (stage) | chance-2: a chance pick is never a stage answer; multi-ask: `choice.accepts(&pick) \|\| accepts_unlisted` | `!matches!(pick, Pick::Chance(_)) && (accepts \|\| unlisted)` |
| 4 | core-tests `action_dispatch`, `aftermath_chain`, `production_use`, `status_phase`, `tactical_composer`, `turn_jobs2` | chance-2 wrapped each `advance_*` in `ti4_engine_chance::resolved`; multi-ask answers with `Choice::pick` / `answer_with` (and the status gain asks nothing) | multi-ask's answers, each call wrapped in `resolved` |
| 4 | bridge `native_play.rs`, `tests/wrapper_native.rs` | pins `43c985a35397f2c9` vs `4f79d5630eaf630d` | re-pinned for the combined line (`fd54dc6c956758bc`, section 4) |

The check of every new `Pick` match / `Ask` match: the workspace compiles with `--all-targets` (the matches are exhaustive); no multi-ask code
calls a `LiveFn` outside the resolver (the core tests and bridge tests pass, a dry tape would fail them).

## 3. Wasm

| build (real `build-wasm.mjs`, wasm-opt) | raw B | gzip B | brotli B |
|---|---:|---:|---:|
| int-28 | 6,624,713 | 2,106,859 | 1,516,834 |
| chance-2 alone (its doc) | 6,630,643 | 2,106,541 | 1,516,110 |
| multi-ask `e057d2496` alone (its doc) | 6,653,942 | 2,120,394 | 1,526,740 |
| **int-29 merged** (`/root/deploytmp/i29-logs/ti4_wasm.merged.wasm`) | **6,661,206** | **2,120,780** | **1,524,274** |
| budget approved after multi-ask | 6,660,000 | 2,126,000 | 1,532,000 |
| **new budget** (`d6f898554`) | **6,661,206** | 2,126,000 | 1,532,000 |

Over the approved budget by 1,206 B raw only (gzip and brotli under). Analysis builds with names (`wasm-named.sh`, `wasm_syms.py`; multi-ask
`7726d0d6d` alone 6,653,992, merged 6,658,576): chance-2 on top of multi-ask is +4.4 KB of code, the same items its doc lists
(`turn::inline::run` +4.4 KB of inlined window monomorphs, `drop_in_place<Box<CoreState>>` +2.8 KB inlining, `SmallList::push` +2.5 KB,
ti4-chance +2.9 KB moved from setup / core). The untried cuts of `CHANCE2_RESUME.md` were assessed against the named build and not made:
`GameRng::die/shuffle` through ti4-chance saves at most ~200 B (both already share `rand_chacha`; string domains vs `Domain`, and the thin
shuffle is fingerprint-relevant), merging `Tape::die/index` into `chance` ~100-200 B (118 + 144 B functions), a `Live` variant without a boxed
`Asking` is not a cheap change. None reaches 1.2 KB, so by the brief the raw budget is raised by exactly the overrun (old 6,660,000, new
6,661,206). Output probes: no legacy literal.

## 4. Re-baseline (once, combined line)

| pin | int-28 | chance-2 alone | multi-ask alone | **int-29** |
|---|---|---|---|---|
| quick game (`native_play --quick --twice`) | `538dc68f23dddcc9` 1,165 | `43c985a35397f2c9` 1,165 | `4f79d5630eaf630d` 1,110 | **`fd54dc6c956758bc` 1,110 steps**, deterministic |
| perf_kernel suite 1 `suite_hash` | `775f4136e60a99d5` | `c8bd543a5d560b91` | `2c99ed238f39713e` | **`31fe4e2bdd62069f`** (NOSTATE `6881f4a9caf01d02`), 13,751 steps, 12,706 decisions, 297 chance requests / 384 faces |

Pinned in: `native_play.rs` (test), `tests/wrapper_native.rs`, `tests/fork_mid.rs`, `tests/fork_core.rs` (doc), and NEW
`perf_kernel::tests::the_suite_hash_is_the_pinned_one` (the suite hash had no test pin before; also `docs/PERF_KERNEL.md`).

Goldens: `golden diff v36-std` on the merged line: only g040 differs (multi-ask's re-record plays a Spy at step 1399: the state after it
differs, 679 decisions affected). `golden bless v36-std --games g040 --expect <exactly the 84 subtypes the re-record changed>` (`8d215909a`;
`CHANGES.md` block). Ladder g036 and the other sets reproduce. `golden check`: 200 games, 146,690 decisions, 0 failed, under the default
mask, `--mask all=core` and `--mask all=string`.

Bench: **NOT re-recorded** (baselines unchanged). A fresh run of the six benches with the int-29 gate selfplay (`verify_benches.sh`, 6 workers,
out `/root/deploytmp/i29-bench`, about 30 min): every bench **0 identical / 200 different** (the multi-ask conversion changes every decision
stream; the 77 entries of int-28 and the Spy/Sanctions games are among them), but **11 games end in `engine_error`** instead of `victory`
(bench: 4p seeds 76, 88, 93, 6p seeds 10, 34, 70; v3: 4p 88; v4: 4p 30, 88; v5: 4p 88, 93). Replayed with a selfplay built from
`7726d0d6d` alone (multi-ask without chance-2, `/root/deploytmp/i29-ma-target`), the 4p games stop identically (same hash, same error), so
they are multi-ask's, not the merge's:

| game | error |
|---|---|
| 4p 88 (bench, v3, v4, v5) | round 1: `aftermath: the answer is no option of the question` (Freelancers' produce-one-unit payment) |
| 4p 76 | round 6: `aftermath: UnportedAftermath(CombatUnported)` |
| 4p 93 | round 6: `ask: TurnGap(StrategyAbility)` |

A baseline must hold victories only (`patch_bench.py` refuses), so nothing was patched; the outputs stay in `/root/deploytmp/i29-bench` for
`/root/deploytmp/i28-logs/patch_bench.py . /root/deploytmp/i29-bench --write` once the errors are fixed (multi-ask's later commit
`bf6735291` "a lead atom that can be completed always is" looks like the fix of the first one). perf_ab has no stored chains (it compares with
a base binary); no other nightly baseline pins game hashes.

## 5. Own commits

| commit | what |
|---|---|
| `7d7944942` | fork_mid pin after chance-2 |
| `e68b52a74` | quick-game / suite pins, new suite-hash test, `native_play --shadow --twice` plays the second run shadowed too |
| `8d215909a` | golden bless g040 |
| `d6f898554` | wasm budget |
| `e4816e502` | server `replay_export`: the mid-game export at 62 answers (on the combined line 64 and 65 are staged-event positions of the open live-vs-replay issue; 56-63, 66-70 agree) |
| (docs) | this file, `docs/I29_RESUME.md` |

`native_play --shadow --twice` fix: multi-ask's `7726d0d6d` swaps Politics' order (S10) only while shadowing, so a shadowed game is another game
(`7824cabdfbb528b6`) than the one played alone (`fd54dc6c956758bc`); `--twice` compared the shadowed first run with an unshadowed second and
reported DETERMINISM FAILED. The second run is now played the same way as the first.

## 6. Checks

| check | result |
|---|---|
| `cargo check --workspace` (without review, replayer, mlp, tensor, advisor) `--all-targets`, also with `ti4-engine/search`; `-p ti4-engine-core-tests --features wrapper --all-targets`; `-p ti4-host --features ts -p ti4-wasm` | green |
| `check_layers.py` core / thin / setup / chance / resolver | 576 / 193 / 14 / 5 / 1 files, 0 violations |
| `check_production_graph.sh` (strict) | 0 violations, 0 failed controls (MCTS crates dev tools only) |
| `clippy_core.sh` | ok, engine warnings 41 = baseline |
| `lint_core.sh` | 578 files, 0 hits, 6 allowed, 0 multi-ask guard hits |
| debug tests: ti4-chance, ti4-engine-chance, core lib, factions, core-tests, thin lib + tests | 3,178 passed, 0 failed |
| bridge (lockstep): lib, tests, bins (incl. golden smoke, fork_core C0/C4, fork_mid quick, wrapper_native, new suite-hash test) | all pass after the re-pin (253 + 174 re-run) |
| `fork_mid` `--ignored` (14-game suite, a copy at every question) | pass (598 s) |
| `fork_core` `--ignored` | `the_codec_round_trip_at_a_quiet_step_boundary_is_exact_strict` fails: the known V5b test, `#[ignore]`d on purpose since C0 |
| `ti4-mcts-features`, `ti4-mcts-lab` (gate, incl. doctests) | 6 + 1 + 7 passed |
| `ti4-host`, `ti4-rule-bot` | pass |
| `ti4-server --test it` | pass after `e4816e502`, except `undo::slow_a_card_that_is_drawn_ends_the_undo_and_a_recovered_game_knows_it` (fails identically on multi-ask alone) |
| `ti4-wasm --test it` | 2 failures, both identical on multi-ask alone: `host_batch::slow_a_movement_plan_is_applied_whole_or_not_at_all` ("the same question again 44 vs 42"; its panic poisons the host lock and fails 38 more tests in the same run), `host_history::slow_an_undo_goes_back_one_answer_and_the_game_goes_on_the_same` (generation 2 vs 1) |
| `ti4-engine-legacy --lib` (lockstep) | **155 failed** / 5,191 passed: the SAME 155 tests fail on multi-ask alone (`7726d0d6d`; lockstep duals of the core windows against the string: hand limit, gains, payments, Leadership, Politics; multi-ask's doc never ran this crate). chance-2 alone was green (5,346) |
| `native_play --quick --shadow --twice` (gate) | 1,110 steps, `7824cabdfbb528b6` (the shadowed game), deterministic, ZERO GAP; unshadowed `--twice`: `fd54dc6c956758bc`, deterministic |
| `perf_kernel suite 1` | `31fe4e2bdd62069f`, NOSTATE `6881f4a9caf01d02`, 297 chance requests / 384 faces |
| shadow sweep `native_play --factions all --players 3,4,5,6 --shadow` | 96 games, 0 gaps, 22 known string bugs, **3 first differences** (jolnar 4p step 849 `event_feats`, yssaril 5p step 606 `LEADER_UNLOCKED`, naaz 4p step 258 `strategy_cards`); all three reproduce with the same fingerprints on multi-ask alone (`7726d0d6d`; multi-ask's `bc578f192` "S3 also shows as the string's next feat" is probably the first) |
| golden check (all sets, default / core / string) | 200 games, 0 failed |
| wasm size gate | ok with the new budget (section 3); output probes: no legacy literal |

Not run (by the brief): `end_of_d1.sh`, soak / strict / census, the mutation ratchet, dual-all; web `tsc` / `vitest`.
