# core-integration-24 (2026-11-04)

Branch `core-integration-24`, cut from `core-rv36` @ `a5c9297f8` (contains core-botstops-1, core-perf-ab, core-integration-23; `RULES_VERSION` 36). Not pushed, not deployed
(only the deploy preflight was run). The live branch `nightly-fixes-2026-10-26` was not touched.

## 1. Merges

| # | Branch | Tip | Conflicts and joins |
|---|---|---|---|
| a | `web-feedback-ui-1` | `fc5ba7891` | none (web/ files, 4 screenshot folders HL to HO, `docs/FEEDBACK_TRIAGE_2026-11-04.md`; no Rust). Contains core-feedback-1 with the movement-dialog fix `d13b36b66`. |
| b | `core-heavy-prep` | `f6a0a56ab` | none (docs/HEAVY_RUN_PLAN.md, `end_of_d1.sh`, `record_benches.sh`, `wasm-size-gate.mjs`, `size-budget.json`). |
| c | `core-perf-why` | `096d18d90` | none (thin-layer perf: no string state per passed-by seat without a window observer, ability ids and table codes memoized per `Render`; new tool crate `ti4-perf-alloc`, bin `perf_why`). Added after the first two by the coordinator's order. |

Wiring of the new crate `crates/ti4-perf-alloc` (counting `GlobalAlloc`, needs `unsafe`): its only dependent is `ti4-engine-core-bridge` (the dev/oracle tool crate, which already
reaches the legacy engine); `cargo tree -i ti4-perf-alloc` shows nothing else, and `ti4-server`, `ti4-wasm` and the other production crates do not contain it. The crate
does not inherit the workspace lints (`unsafe_code = "forbid"` stays in force for every other member; the crate allows it for itself with a SAFETY comment). Join change: `check_production_graph.sh` now has
`ti4-perf-alloc` in its list of crates that must NOT be in a production tree (and in `DEV_TOOLS`); proved with a negative control (a temporary dependency of `ti4-server` is
reported for server and wasm; restored), strict 0 violations, 0 failed controls, self-test ok.

## 2. Version 36 follow-ups (docs/RV36_RESUME.md)

1. **Golden**: `golden bless-new --set v36-std ... --games 0,12,40,55 --require-dual-clean all` (the four tables of the v35 seed set: 3p, 4p, 5p, 6p; this time g055 is dual-clean, so
   4 games, 7,896 decisions), `golden ladder` (game 36, dual-clean), `golden index` (freezes v35-std, legacy te and templates), all under `flock /root/.ti4-heavy.lock`.
   `golden check` (200 games, 147,781 decisions) is green with the default mask, `all=core` and `all=string`; `every_version_has_a_set` green.
   **Bug found and fixed (B1)**: `golden check` failed at step 0 for every game recorded at 36 ("the engine asked a question the recording does not have"): the recorder plays the wrapper
   (the core splits steps the string glues: 839 steps against 681 for the same game), but `replay_game` replayed everything on the string engine since I21. Now a game recorded at the newest version replays on the wrapper it
   was recorded on, older versions stay on the frozen string engine (trait `Replay` in `golden/replay.rs`; README note). Guarded by the golden lib tests (`the_smoke_subset_reproduces` includes v36-std and the ladder).
2. **Streams with Ixthian Artifact (For) or a Parley**: none needs re-recording. Scan of `tools/engine_core/decision_corpus` and the golden sets: every stored stream that plays a Parley (3 of the 4 games of every bench source,
   13 of 20 `golden20` games) or reaches the Ixthian Artifact is at rules version 11, 18 or 23, and a stream of an older version replays on the frozen string engine at its own version, which version 36 did not change.
   `te` and `templates` (version 35) and `v35-std` contain neither. What does change is the bench baselines (they are re-recorded for the engine under test): see HEAVY_RUN_PLAN section 4 (Parley is played in about half the bench games, so a string recording at 35 cannot be reproduced 200/200 any more, by design S6/S7).
   **Finding (B2)**: `decision_corpus verify` fails on 9 of 10 sources by construction ("rules version 23 is not supported (the core plays 36 to 36)", TE content off) and on `fixtures` (stale since I22 regenerated the wire fixtures); it was already red before version 36. The `corpus` step of `end_of_d1.sh` is skipped by default with a logged reason (`CORPUS_SKIP=0` runs it); the same games replay from the golden sets in the `census` step.
3. **Quick-game fingerprint** `538dc68f23dddcc9` (1,165 steps): pinned in `tests/wrapper_native.rs` and `native_play.rs`, no other pin exists in code or tools; `native_play --quick --shadow --twice` prints it (shadow compared 808, deterministic, zero gaps), before and after the `core-perf-why` merge. Older docs keep `74d5675ac754ac81` as history of their time.

## 3. Bugs found

| # | Where | What | Fix |
|---|---|---|---|
| B1 | `golden/replay.rs` | newest-version golden games cannot replay on the string engine (step numbering) | replay on the wrapper for `rules_version == RULES_VERSION` |
| B2 | `end_of_d1.sh` step `corpus` | red by construction (above) | skipped with reason, `CORPUS_SKIP` |
| B3 | `end_of_d1.sh` step `lint` | base tag `deployed-int23-54a15dba` is stale: version 36 (`state.rs`) and the web feedback UI (`web/`) changed the immutable surfaces on purpose | base = new local tag `heavy-base-int24` on the tip (fallbacks core-integration-24); plan section 2 |
| B4 | `tests/setup_parity.rs` `the_native_modules_name_only_native_modules_and_the_listed_joins` | red since D2-9b (the `#[cfg(test)]` module of the thin `seating.rs` names the legacy oracle; the scan looked at the whole file) | the legacy-name assertion looks at the production part only (the `crate::` scan below it already did) |
| B5 | `dual_window.rs` `compare_resolver` | the golden dual (driver area) reports `resolver.state` differences on every game tested (3 of 3: 72, 29, 50), identical at core-botstops-1: the initiative list ORDER differs by the S1 ruling | seats compared as a set; test `the_order_of_the_initiative_list_is_not_compared_the_seats_are_s1` (games 1 and 8 then clean) |
| B6 | open, pre-existing | golden dual game 6 (`winnu,titans,mentak,ghost`, 4pHyperlanes, seed 7): one `[driver/Value] combat::settle` difference, the string's settle has a roll group `(0,[8,7])` the core lacks. Same at core-botstops-1. | not fixed (heavy-run item, HEAVY_RUN_PLAN 12.1) |
| B7 | plan | bench baselines cannot be 200/200 against a string recording (S6/S7) | two-stage procedure, plan section 4 |

No join bug between the three merges. No new string-engine bug.

## 4. Matrix (profile gate, `CARGO_INCREMENTAL=0`, private target `/root/deploytmp/i24-target`; ran on the tip before the `core-perf-why` merge, repeated after it where marked)

| check | result |
|---|---|
| `cargo check --workspace` (without review, replayer, mlp, tensor, advisor) `--all-targets` | green (also after the perf-why merge) |
| `check -p ti4-engine-core-tests --features wrapper --all-targets`, `-p ti4-host --features ts -p ti4-wasm`, `-p ti4-wasm --target wasm32-unknown-unknown` | green |
| `clippy_core.sh` | ok, engine warnings 41 = baseline (also after the merge and the dual fix) |
| clippy of the web crates (`l-web` set) `--all-targets` | green |
| `check_layers.py core` / `thin` | 575 files 0 / 191 files 0 |
| `check_production_graph.sh` strict + self-test | 0 violations, 0 failed controls |
| `check_output_probes.sh` on the wasm | ok: no legacy literal (server binary: in the deploy preflight) |
| wasm (cargo `wasm-release`, binaryen 123) + `wasm-size-gate.mjs` | 6,441,691 raw / 2,044,587 gzip / 1,471,798 brotli; budget 6.63 / 2.11 / 1.52 MB: ok (before the perf-why merge) |
| web `tsc --noEmit`, `vitest run` | clean; 278 files passed, 2 skipped; 2,750 tests passed, 4 skipped |
| `native_play --quick --shadow --twice` | 1,165 steps, fingerprint `538dc68f23dddcc9`, shadow compared 808, zero gaps, deterministic (also after perf-why) |
| perf_ab `core smart` 3p/4p/5p/6p x seeds 1-6 | 24 of 24 finished, 0 errors, 0 bad (before and after perf-why) |
| `wrapper_native` 3, `kernel_env` 9 | pass (after perf-why) |
| `golden check` (all 200 games), masks default / `all=core` / `all=string` | 0 failed |
| dual_run profile-20 `--game 1 / 6 / 8` `all=dual` (probe, NOT the gate) | 0 / 1 (B6) / 0 differences after B5 |
| after the `core-perf-why` merge, wasm size | 6,444,613 raw / 2,045,548 gzip / 1,472,249 brotli: ok against the budget (+2.9 KB raw); output probe ok |
| after the merge: bridge (lib + all tests) | 223 passed, 0 failed (incl. `setup_parity` 28, `host_create_golden`, `wrapper_native`, `kernel_env`, golden lib tests, `every_version_has_a_set`) |
| after the merge: host 320, wasm host 58 (3 ignored, feature `ts`), thin ti4-engine + model + content + rule-bot + ti4-bridge 964 (2 ignored) | all green |
| after the merge: `ti4-server` (all `tests/it`, incl. `bluff_reaction`, `--test-threads=2`) | 415 passed, 8 ignored, 0 failed |
| after the merge: web vitest/tsc | unchanged (no web file changed by the merge) |

## 5. Deploy preflight
`deploy/public/deploy.sh --ref core-integration-24 --preflight-only` on `75eac2cd7` (the tip at that time; the tag `heavy-base-int24` is on it, later commits are docs only): web client built, wasm compressed, production graph strict
0 violations / 0 failed controls, release server built, output probes ok (server binary and wasm: no legacy literal), preflight on a copy of the live data: **recovered 10, failed 0, refused 17** (all "saved with an older version of the rules", files kept;
int-23 had 14 refused, three more old games were created since), no `--allow-failed`. NOT deployed, nothing pushed, `current`/`previous` untouched.

## 6. Not run (heavy run, by order)
Z-gate, mutation ratchet, golden dual (all 20 games), benches (baselines are re-recorded first, plan section 4), soak, census, D4 crate, torch crates (not buildable here).
Prepared for the run: `docs/HEAVY_RUN_PLAN.md` (version 36, section 12 says what is red before the start and how to start).
