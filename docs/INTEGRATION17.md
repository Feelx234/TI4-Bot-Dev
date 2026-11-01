# core-integration-17: merge record (2026-11-01)

Branch `core-integration-17`, cut from `core-integration-16` @ `4ebb38ce3`. Two merges, one at a time, in the ordered list: lane Q (`core-q-research`, the last native stops of integration 15) and lane D2-7 (`core-d27-split`,
the package split of the string engine). Lanes R (`core-r-shadow3`) and D2-8 lanes A, C (`core-d28a-types`, `core-d28c-setup`) are NOT merged here (integration 18). Not pushed. The live branch `nightly-fixes-2026-10-26` was not touched.
Pure refactor: `RULES_VERSION`, serde, fixtures and fingerprints of the string engine are unchanged (no file of those was edited; the bench-hash check below confirms it).

Read first: `docs/INTEGRATION16.md`, `docs/INTEGRATION15.md`, `docs/Q_RESUME.md`, `docs/D27_RESUME.md`, `docs/todo/D2-7-legacy-split.md`.

**Package names from here on (R5, R11 of `docs/D27_RESUME.md`)**: the package of `crates/ti4-engine` is `ti4-engine-legacy` (lib `ti4_engine_legacy`); `crates/ti4-engine-next` is the thin facade package `ti4-engine`. Every
`cargo test -p ti4-engine ...` in the older records (INTEGRATION9 to 16, the `*_RESUME.md` notes) means the string engine, i.e. today `-p ti4-engine-legacy`; those records are history and were not rewritten (the split did not rewrite
docs either).

## 1. Merges

| # | Branch | Tip merged | Merge commit | What it brings | Files in conflict |
|---|---|---|---|---|---|
| 1 | `core-q-research` (Q, cut from int. 15) | `4fbfd06a6` | `4f9ddeaab` | `rules::production::payable` (the research gate counts planets and goods, not the Triad and not War Machine's five), used by `Research::after_offers` and the Inheritance Systems check; Brother Milor's fighter forms in the combat texts (`milor_in_play`, `forms_of`, `researches_in_combat` in `ti4-engine/src/combat_live.rs`); the `payable` / Milor tests; `docs/Q_RESUME.md` | 2 (`rules/production/strategy_cards.rs`, `ti4-engine-core-tests/tests/research_e2.rs`: the known N x Q one) |
| 1b | (this integration) | -- | `513e7cdae` | test for the join, section 3 | -- |
| 2 | `core-d27-split` (D2-7) | `2014339a4` | `a6c9bf7f1` | package rename `ti4-engine` -> `ti4-engine-legacy` (directory kept), thin facade `ti4-engine` in `crates/ti4-engine-next`, bridge / aliases / tools retargeted, `docs/D27_RESUME.md`, `docs/todo/D2-7-legacy-split.md` | 0 |

After EVERY merge: `cargo check --workspace --all-targets` (without ti4-review, ti4-replayer, ti4-mlp, ti4-tensor, ti4-advisor) green and `awk '/^fn settle_run/,/^}/' crates/ti4-engine/src/dual_window.rs | grep -ci neutral` = **0**
(after Q, after the split, at the end). No `Out { .. }` literal or registration was dropped; the registry-equality tests, the audit test and `no_two_rules_of_the_core_table_share_a_slot` are green in the legacy lib (section 5).

## 2. The one conflict: lane N's War Machine gate x lane Q's `payable`

Both lanes fixed the same cause of the same symptom (a Technology research asked or offered to a seat whose only pool is a stale War Machine mark): the core used `production::available`, which counts War Machine's five (the mark stays
on the seat until the next activation) and the Triad; the string gates with `payment::affordable` (planets and trade goods only).

* N (in the base): `KernelEnv::technology_secondary_affordable` and `Research::after_offers` subtracted `war_machine_bonus` from `available` (War Machine only).
* Q: new `rules::production::payable` = `available` minus the Triad and, for resources, War Machine; used by `Research::after_offers` and by the `afford_inheritance` check (`strategy_cards.rs` line 653, auto-merged).

Determination: Q's `payable` is the superset and the string's own definition; ONE gate. `after_offers` takes Q's call (the two hunks collided: N's line dropped, comment merged). `technology_secondary_affordable` (N's, in `driver/kernel_env.rs`,
not in conflict) now calls `payable` too (a `Cx::new` like `can_produce_in`), so the follower's eligibility and the research window that follows it can no longer disagree. `research_e2.rs`: both sides' tests kept (N's
`war_machines_bonus_is_no_way_to_pay_a_research`, Q's `a_stale_war_machine_mark_does_not_make_a_research_affordable` and `the_triad_does_not_make_a_research_affordable`).

**No assertion was changed.** All regression tests of both lanes pass on the merged tree: core `research_e2` 9, `production_rules` 7 (incl. `payable_leaves_out_the_triad_and_for_resources_the_war_machine`), `strategic_arms` 41 (incl. N's
`war_machines_bonus_does_not_make_the_technology_secondary_affordable`), legacy `strategy_research_tests` 28.

## 3. Bugs found, and how each is guarded

| # | Where | What | Fix | Guard (fails before the fix) |
|---|---|---|---|---|
| B1 | **N x Q** (`KernelEnv::technology_secondary_affordable`) | After the textual merge, N's gate still used `available - war_machine_bonus`: the Triad stayed in the pool of the follower's eligibility although Q's research window (and the string) exclude it. A seat with the Triad and one trade good (3 + 1 = 4) was offered the Technology secondary, the research window then found nothing to pay with: the same counted `StrategyAbility` stop as Q's cause 1, one window earlier | `technology_secondary_affordable` uses `production::payable` | `strategic_arms::the_triad_does_not_make_the_technology_secondary_affordable` (new). Seen failing with N's old body restored (`FAILED`), passing with the fix |

No other break was found: the split merged with 0 conflicts and nothing newer than its cut needed retargeting (section 4).

## 4. Retargeting to the split (what was looked for)

The split was cut from int. 15; lanes N, P, Q (and J, K, L, E2, H5 before) came after or were merged into the line after it. Checked on the merged tree, by `git grep`:

* `-p ti4-engine` / `--package ti4-engine` in `tools/`, `.cargo/`, scripts and CI files: only the already retargeted lines (`-p ti4-engine-legacy -p ti4-engine`, `clippy_core.sh`, `end_of_d1.sh`, `verify_package.sh`, `verify_stage.sh`, `nightly-smoke/prewarm.sh`, the five aliases). Nothing newer names the old package. `verify_stage.sh` keeps `--features ti4-engine/string-movement` (the facade forwards the features).
* `ti4_engine::` in `crates/ti4-engine-core-bridge`, `-core-tests`, `-core`, `-factions`, `crates/ti4-engine/{tests,examples,src}`: every hit in the bridge is one of the glue modules that stay on the thin crate (`core_sync, generated, live, render, product, seen_load, kernel_sync, rng, fingerprint, choice, decision_context`); the legacy-only modules in
  the bridge's newer files (lane P's shadow harness, N's / Q's / J's code) already say `ti4_engine_legacy::`; the only other hits are the `wrapper_props.rs` lines (the optional `wrapper` feature uses the facade, as the split decided) and doc comments of the core. `ti4-engine-core-bridge/Cargo.toml` depends on both.
  So the lanes' new files needed NO edit; the workspace compiles without a facade-only crutch being added.
* Docs / scripts added by the lanes after the cut (`INTEGRATION16.md`, `N_RESUME.md`, `P_RESUME.md`, `Q_RESUME.md`, `STRING_ENGINE_BUGS.md`): the only commands naming the old package are the result lines in INTEGRATION16 and the `ti4-engine --lib` result lines of P / Q (history; see the note at the top).
  Left as the split left them: the ~40 doc comments and old `plans/` / `docs/` commands that say `cargo test -p ti4-engine` (D2-7 "open leftovers"); the `print_prefix_tables` header text in `ti4-engine-core/src/keys/tables.rs` is compared by a test to a generated string, so it was not touched.

## 5. Edits of assertions, fixtures and harnesses (reasons)

None. No `native_play` pin moved: the quick game is the same game as in int. 16 (1,165 steps, 9 rounds, fingerprint `74d5675ac754ac81`, `--twice` the same) and its shadow pin (589 compared, step 862) is unchanged.

## 6. Checks (cheap), `CARGO_BUILD_JOBS=2`, private target `/root/deploytmp/int17` (profile `lockstep` with debug assertions + overflow checks for the three core crates, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0`; deleted at the end)

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (without the torch crates) after each merge | ok (Q, split, final); `settle_run` neutral-side grep **0** each time |
| `cargo test -p ti4-engine-legacy --lib` plain | **5295 passed**, 0 failed, 7 ignored (int. 16: 5291; +Q's 4) |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (WHOLE lib) | 5295 passed, 0 failed, 7 ignored |
| `ti4-engine-core --lib`, `ti4-engine-core-factions --lib`, `ti4-rule-bot-typed --lib` | 477, 151, 1 passed |
| `ti4-engine-core-bridge --lib` and `--test kernel_env --test play_facts --test menu_sources` | 122, and 3 / 3 / 2 passed |
| the D4 crate (`ti4-engine-core-tests`, `lockstep` profile, the flags of `tools/engine_core/d4/run_core_tests.sh`, run in this worktree) | 154 result lines, **1962 passed, 0 failed** (int. 16: 1958; +Q's 3 and my test, N's 1 already counted) |
| `check_layers.py core`, `lint_core.sh` | 574 files, 0 violations; 576 files, 0 hits |
| `tools/engine_core/clippy_core.sh` | ok (engine 41 = baseline 41; core crates 0 warnings) |
| `cargo check -p ti4-wasm --target wasm32-unknown-unknown`, `cargo c-web`, `cargo check -p ti4-engine-core-tests --features wrapper --all-targets` | ok |
| bench-hash check (`verify_benches.sh`, `ONLY=bench`, `selfplay` built from the facade, `gate`) | **200 identical, 0 different, 0 not in the baseline** |
| usage | five-hour window 11 % at the start, below 20 % throughout (weekly figure not a criterion) |

Not run (user rule): `end_of_d1.sh` and its steps (golden / wide dual, `board_equivalence`, `core_sync_gate`, `alpha_roundtrip`, benches beyond the one check, soak, census, ratchet, workspace clippy beyond `clippy_core.sh`), mutation batches,
`TI4_CORE_STRICT=1`, Playwright, `cargo wire-web` (fails with the unchanged base config too, `docs/D27_RESUME.md`).

## 7. The native playthrough and the shadow (`tools/engine_core/native_play.sh`, gate profile, `te` card set)

### 7.1 The whole game, plain

| run | games | finished with zero gaps | native steps | notes |
|---|---|---|---|---|
| `--quick` (sol, 4 players, seed 1) | 1 | **1** | 1,165, 9 rounds, fingerprint `74d5675ac754ac81` | `--twice`: the same fingerprint (int. 16: the same) |
| `--factions all --players 3,4,5,6 --seeds 1` | 96 | **96** | 103,150 (int. 16: the same) | Q's result reproduced on the merged tree (N x Q join: no stop) |
| `--factions all --players 3,4,5,6 --first-seed 2 --seeds 3` | 288 | **288** (int. 16: 287) | 298,499 (int. 16: 297,968) | yin 6p seed 3 (`CombatUnported`, int. 16's one gap) now plays: Q's Brother Milor forms |

### 7.2 The shadow (`--shadow`, the 96-game set)

**59 clean to the end, 9 stop at the known string bug S1 (excluded), 28 show a first difference, 0 gaps**: the same split as int. 16, and the first-difference step of every game is the same as in its table (sol 4p 862, sol 5p 867, sol 6p 1026,
letnev 3p 684, letnev 4p 289, xxcha 3p 700, jolnar 5p 369, arborec 4p 652, mentak 5p 1012, muaat 3p 514, nekro 6p 1100, saar 4p 303, sardakk 3p 230 / 4p 433 / 5p 333 / 6p 280, winnu 3p 500 / 4p 769 / 6p 603, ghost 4p 548, argent 4p 464,
cabal 3p 766 / 4p 881, empyrean 5p 557, mahact 4p 1002 / 6p 1172, nomad 4p 887, titans 5p 910). The classes (SP, OT, RW, RD, TG, EV, AW, OTHER) and the work list are INTEGRATION16 section 7.2 / 8, untouched by Q and the split. Exit code 1 as in int. 16 (differences are reported, not hidden).

## 8. Remaining gates

1. Integration 18: lane R (`core-r-shadow3`, the shadow differences) and D2-8 lanes A, C (`core-d28a-types`, `core-d28c-setup`).
2. Everything of INTEGRATION16 section 8 that is not touched here: the shadow classes (work list of lane R), the Technology-secondary case of cabal 4p, open staged-event announcers, Thunder's Edge corners, the Ssruu / borrowed-agent variant of the waiver.
3. Split leftovers (`docs/D27_RESUME.md`): D2-8b (fill `crates/ti4-engine-next`), D2-9 (strip the dual wiring; L2), doc comments and old docs that say `-p ti4-engine`, a clippy baseline still keyed to `crates/ti4-engine/`.
4. Heavy gates still to run: golden and wide dual sweeps, `board_equivalence`, `core_sync_gate --strict`, `alpha_roundtrip --strict`, the benches beyond one check, `census`, `soak`, the mutation ratchet and the mutation batches of every resume note since integration 9 (plus N, P, Q), workspace clippy, `TI4_CORE_STRICT=1`.

## 9. Tip

`git log -1 core-integration-17` (the last commit is this document's). The private target dir and the logs under `/root/deploytmp/int17*` were deleted after the last check.
