# core-integration-6: merge record (2026-10-30)

Branch `core-integration-6`, cut from `core-integration-5` @ `670963aa` (tooling branches merged in a second round, section 7). Cheap checks only: the user deferred every heavy gate (golden dual, benches,
soak, `core_sync_gate`, `alpha_roundtrip`) to the end of D1. Not pushed. The live branch `nightly-fixes-2026-10-26` was not touched. The `core-w*`
(lent bodies) branches are NOT merged (still running).

## 1. Merges, in order

| # | Branch | Tip merged | What it brings | Conflicts |
|---|---|---|---|---|
| 1 | `core-d1b2` | `4c9b0d47` | D1b remainder: space cannon offense (`cannon_dual.rs`, `rules/combat/space_cannon_offense.rs`), the aftermath head and `Window::Combat`, `choice::turn` (new **Turn** area) + `render/turn.rs` + `driver::menu`/`dispatch`/`step`/`tactical_tail`, `fleet::enforce_everywhere`, `menu_dual.rs` | none |
| 2 | `core-d2-6a` | `5557c60a` | allow-list (`allow_list.rs`, `allow_list_check` bin, `tools/engine_core/allow_list.json`, empty) | none |
| 3 | `core-d2-1` | `8f658a07` | D2-1/D2-2: `fixtures::seated_dealt`, `core_sync::{unmapped, unsupported, guard}` (counted `Unmapped`, strict mode, disabled-feature policy), `ti4-host` creation guard, bridge `roundtrip.rs` + `alpha_roundtrip`, `marks_census.py` | none |
| 4 | `core-d2-3` | `43189100`, then `58cce225` | D2-3 text tables (`containers::Interner`, `CoreState.texts/owners`), D2-4a `resolver_sync.rs` + `TI4_CHURN=resolver`, D2-4b `ResolverState` in `Pending`, D2-5 `materialize`, `reverse_lend::with_string`, `lend_table`, `TI4_CHURN=state`, and the D2-5-find-02 fix (the core answers `has_legal_target` / `playable_hand` in the scope of the game) | 3 files, below |
| 5 | `core-d3-0` | `621ad11e` | typed bot crate `crates/ti4-rule-bot-typed`, `ti4_engine_core::seen`, `ti4_model::count` (+ feature `observed-count`), `tools/rulebot/d3/` | none |
| 6 | `core-d4a` | `b6faaba5` | test crate `crates/ti4-engine-core-tests` (kit, 600+ tests, snapshots), `tools/engine_core/d4/`, `docs/d4a_mutation_survivors/` | none (but see 3) |
| 7 | `core-d4b` | `9ed8c4a2` | golden games (`golden` bin + `golden/` module, `tools/engine_core/golden/`), coverage (`d4/coverage.py`, `docs/ENGINE_CORE_COVERAGE.md`), mutation baseline and ratchet (`mutation_ratchet.sh`, `mutation_floor.json`, `docs/D4_MUTATION_BASELINE.md`), `d4_gate.sh` steps 4-6 | `Cargo.lock` only (see 2) |

The first merge of `core-d2-3` was at `43189100`; its second tip (`58cce225`, `playable_hand`) was merged again after the check run found the second
difference of the same cause.

## 2. Conflicts and how they were resolved (keep both sides)

| File | Conflict | Resolution |
|---|---|---|
| `ti4-engine/src/core_sync/mod.rs` | two lists of `mod` / `pub use` lines (d2-1: `unmapped`, `unsupported`; d2-3: `materialize`, `texts`, `TextCodes`, `bad`) | all modules kept; `bad` is `pub use` (d2-3 needs it outside `core_sync`; d2-1 had it `pub(crate)`) |
| `ti4-engine/src/core_sync/lend.rs` | `Lend::region`: d2-1 added `unsupported::check(self.gs)?` and kept `Region<'_>`; d2-3 removed the lifetime (the seeds are owned now) | signature of d2-3, body of d2-1 (the check stays) |
| `ti4-engine-core-bridge/src/bin/core_sync_gate.rs` | d2-1 moved the per-position logic into the library module `roundtrip.rs`; d2-3 had edited the old body (attach/adopt the tables, high-water marks) | d2-1's bin taken; d2-3's edit re-applied in `roundtrip.rs` (`cx.attach(&mut core)` after the core is made, `cx.adopt(&core)` after the load loop, `max_texts`/`max_blobs` read from `core.texts`) |
| `Cargo.lock` (merge 7) | `ti4-engine-core-bridge` dependency list: d2-1 added `rand`, `rand_chacha`, d4b added `serde` | all three kept, in order |
| `docs/ENGINE_KNOWN_ISSUES.md`, `dual_window.rs`, `check_layers.py` | no textual conflict (git merged them) | `ENGINE_KNOWN_ISSUES.md`: the rows of D2 lane B, D4 lane A and D4 lane B, which lived uncommitted in the `core-integration` worktree, were appended |

## 3. Breaks the combination caused (all fixed on this branch, none by weakening a test)

| Where | Break | Fix |
|---|---|---|
| `ti4-engine-core-tests/src/rig.rs:103` | d4a was cut from int-4; int-5 (B12) gave `AbilityTable::registry` a third closure argument (the lender) | closure takes `|spec, _, _|` |
| `ti4-engine-core-tests/tests/choice_layer.rs:37` | **assertion edit**: pinned `Area::ALL.len() == 19`; d1b2 added the `Turn` area | `20`, message names the cause |
| `ti4-engine-core-tests/tests/leader_heroes.rs:58` | clippy `unused_must_use` on `keep.replay()` | `let _ = keep.replay();` |

## 4. Checks run (cheap) and results

All with a private `CARGO_TARGET_DIR`, profile `lockstep` unless noted, by Haiku runners in scratch worktrees (tip `72e17552`; the test crate again at `5f57b1d5`).

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (without ti4-review, ti4-replayer, ti4-mlp, ti4-tensor, ti4-advisor) | ok |
| `cargo clippy` of core, factions, engine, bridge, core-tests, rule-bot-typed (all targets) | ok; one warning in the merged tree (`leader_heroes.rs:58`), fixed |
| `cargo test -p ti4-engine --lib` | 4709 passed, 0 failed, 4 ignored (int-5: 4589) |
| `cargo test -p ti4-engine-core -p ti4-engine-core-factions -p ti4-rule-bot-typed` | ok (core 356 + 26 + 3, factions 114, rule-bot-typed 3) |
| `ti4-engine` tests `decision_delivery_inventory`, `content_ids_resolve` | 4 + 4 passed |
| `cargo test -p ti4-engine-core-bridge --lib` | 22 passed |
| `cargo test -p ti4-engine-core-tests --no-fail-fast` | see section 5 |
| `check_layers.py core`, `lint_core.sh`, `tools/rulebot/d3/test_census.py` | 0 violations, 0 hits, 5 tests ok |

## 5. The test crate and its profile

`ti4-engine-core-tests` pins its own profile: `a_state_with_a_slice_missing_refuses_to_read_it_in_debug` fails unless debug assertions are on for the three core
crates (D4-design-01). In the plain `lockstep` profile that one test fails (610 passed, 1 failed); with the flags of `tools/engine_core/d4/run_core_tests.sh`
(`--config profile.lockstep.package.<crate>.debug-assertions=true --config ...overflow-checks=true` for `ti4-engine-core`, `-factions`, `-tests`) the result is in the
last line of this section.

**Result with the D4 flags: 611 tests passed, 0 failed, exit 0** (d4a's own count at its last green commit was also 611); clippy of the crate (all targets): 0 warnings after the `leader_heroes.rs` fix.

## 6. Deferred (heavy, end of D1)

* The combined gate: `verify_package.sh none` (tests, benches 200/200 x6 incl. `TI4_CHURN=resolver`, `state`, `resolver,state`), the sharded golden dual with `all=dual`
  and the wide mask (new label `resolver.state`), `board_equivalence` templates + golden, `core_sync_gate --strict`, `alpha_roundtrip --strict` on a fresh copy of
  `/var/lib/ti4-public`, `marks_census.py --strict`, `tools/engine_core/d4_gate.sh`, the D3 baseline record (`tools/rulebot/d3/baseline.sh`), mutation runs of the
  merged crates. Resume notes: `docs/D1B2_RESUME.md`, `docs/D2_1_RESUME.md`, `docs/D2_3_RESUME.md`, `docs/D3_RESUME.md`, `docs/D4A_RESUME.md`.
* The bail table (`lend_table::BAILS`, `TI4_BAIL_PROBE` + `bail_table`) and one `with_string` wrapper per label: needs the golden dual.
* Not merged on purpose: the `core-w*` lent-body branches (running).
* The one-script version of all of the above: `tools/engine_core/end_of_d1.sh` (section 7).

## 7. Second round: the tooling branches, one table, one script

**Already in, checked with `git merge-base --is-ancestor`:** `core-dual-shards` (`bdbe148d`), `core-compute` (`cad8f451`), `core-soak` (`6389525c`) (carried by
int-4/int-5). **Merged now:** `core-d4b` (row 7 of section 1; one conflict in `Cargo.lock`, section 2).

**Folded:** the two copies of the field-to-slice table (`bridge::roundtrip::slice_of_key` and `core_sync::materialize::slice_of_field`) were compared key by key
(80 keys, identical); `roundtrip::slice_of_key` now delegates to the engine's table.

**Fixed after the merge:** three clippy `type_complexity` warnings in the golden tool (`golden/bless.rs`, `golden/record.rs`): type aliases, no behaviour change.

**Checks of the tip `b9a0902b` (then `18321bc7`), same set as section 4, all green:** workspace `cargo check --all-targets` ok; `cargo test -p ti4-engine --lib` 4709 passed;
core + factions + rule-bot-typed + bridge `--lib` 517 passed; inventory tests 8 passed; `ti4-engine-core-tests` with the D4 profile flags 611 passed; `check_layers.py core`
421 files, 0 violations; `lint_core.sh` 423 files, 0 hits; `test_census.py` 5 ok; `bash -n` of `mutation_ratchet.sh`, `d4_gate.sh`, `verify_package.sh`. Clippy after the fix of the three
warnings: see the last line of this section.

Clippy of the bridge after the aliases (tip `18321bc7`): 0 warnings or errors in `crates/ti4-engine-core-bridge/`; `cargo test -p ti4-engine-core-bridge --lib` 46 passed (the golden module's tests included); `end_of_d1.sh --dry-run` prints the 20 steps and `bash -n` is ok. Clippy of the other five crates at `b9a0902b`: 0 hits.

**`tools/engine_core/end_of_d1.sh`** (written, NOT run): the combined heavy run of the end of D1 in one script that re-executes itself under `/root/heavy2.sh`
(`SHARDS=3`) and holds the lane for the whole run. `--dry-run` prints the 20 steps and what each runs without taking the lock; `--only`, `--skip`, `--from` select steps.
Order: `layers`, `lint` (+ immutable surfaces), `build`, `clippy`, `tests` (packages, the engine suite with every area on Dual and fail-fast, the D4 crate with its
profile, gen-cards, xtask, wasm tests), `dual-all` (every area, bail probe on), `dual-drv` (driver=dual), `dual-wide`, `board-t`, `board-g`, `sync`
(`core_sync_gate --strict` + `marks_census.py --strict`), `alpha` (`alpha_roundtrip --strict` on a copy of `/var/lib/ti4-public`), `bench-off`, `bench-resolver`,
`bench-state`, `bench-resolver+state` (200/200 identical x6 each), `census` (the bail table from the probe, the golden games on `all=core` and `all=string`), `soak` (one
round), `d4gate` (steps 3-5), `ratchet` (mutation floors, last). A failing step does not stop the run; the summary (`<out>/summary.txt`) names every failure. Usage:
`CARGO_TARGET_DIR=/root/deploytmp/eod tools/engine_core/end_of_d1.sh --out /root/end-of-d1-<date>`.
