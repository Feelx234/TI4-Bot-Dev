# Core-state port: integration trial

Branch `core-integration` (worktree `.claude/worktrees/core-integration`), cut from `core-state-main` 201f6714. Nothing
pushed, no live branch touched, package branches untouched. Pure refactor: no change to `RULES_VERSION`, serde output,
fixtures or fingerprints (not re-measured here: no golden runs were made).

## Merge order and conflicts

Each package at its tip as of 2026-10-28 morning (`--no-ff`, one commit each).

| # | Branch | Conflicts |
|---|--------|-----------|
| 1 | core-a5-kernel | 0 |
| 2 | core-a1-board-queries | 0 |
| 3 | core-a2-movement-tactical | 0 |
| 4 | core-a3-technology | 0 |
| 5 | core-a4-objectives | 2 |
| 6 | core-c0-seams | 2 |
| 7 | string-disentangle-s1-s8 | 0 |
| 8 | core-d1-driver | 0 |
| 9 | core-ca-combat-factions | 0 |
| 10 | core-cb-economy-factions | 0 |
| 11 | core-cc-ground-factions | 0 (already contained) |
| 12 | core-ce-movement-factions | 2 (+1 silent loss, see below) |
| 13 | core-cd-registry-factions (has commits) | 2 |
| 14 | core-b1-combat | 0 (+1 semantic break with CE) |
| 15 | core-b2-invasion | 0 |
| 16 | core-b3-production | 3 |
| 17 | core-b4-trade | 5 (+1 semantic break) |
| 18 | core-b5-explore-relics | 5 |
| 19 | core-b7-leaders | 0 (already contained via B5 line) |
| 20 | core-b8-strategy | 4 |
| 21 | core-b9-agenda | 6 (+1 follow-up fix) |
| 22 | build-speed (b60de954) | 1 |

Total: 32 textual conflicts in 10 merges, 4 follow-up fixes. Almost all are additive list collisions: every package
appends to the same handful of registries.

## Conflicts and resolutions

* **A4**: `query/mod.rs` (module list: reach + objectives, both kept); `choice.rs` `Observed` (A2's `typed_position` +
  seat cache and A4's `objview`/`objective_view`, both kept).
* **C0**: `ti4-engine/Cargo.toml` (`string-objectives` + `string-factions`); `dual.rs` `AreaMask::build_default` (A4 line + C0 comment).
* **CE**: `bridge/check.rs` (CC and CE compare blocks, both kept); `core_factions/mod.rs` (CE's `pub mod ce`, CC's header).
* **CD**: `check.rs` (CA, CC, CE, CD blocks, all kept); `core_factions/mod.rs` (modules ca..ce, all `cfg(typed-core)`).
* **B3**: `rules/mod.rs` add/add (`production` added); `lib.rs` (kept `core_factions` unguarded; module lists unioned);
  `render/mod.rs` (`uninhabited!` and `registry!`: Combat/Invasion/Production ported).
* **B4**: `query/mod.rs` (tech + trade); `rules/mod.rs` (+trade); `Cargo.toml` (+`string-trade`); `dual.rs` (+TradeDiplomacy);
  `render/mod.rs` (`Render` has both `position` and `shapes` fields; `new()` sets both; Trade ported).
* **B5**: `board_equivalence.rs` (B7 `leaders` mode and B5 `cards` mode, both dispatched); `query/mod.rs` (explore, promissory,
  relic); `Cargo.toml` (+`string-exploration`); `dual.rs` (+ExplorationRelics); `relics.rs` `grants_sustain` (B1 dual on
  `Area::Combat` via `board_view` vs B5 on `Area::ExplorationRelics`; took B5, which owns relics).
* **B8**: `rules/mod.rs` (+strategy); `Cargo.toml` (+`string-strategy`); `dual.rs` (+Strategy); `lib.rs` (strategy_view/typed/check).
* **B9**: `rules/mod.rs` (+agenda); `Cargo.toml` (+`string-agenda`); `core_factions/mod.rs` (B9 carried an old header and a
  second `pub mod ca`; dropped); `dual.rs` (+AgendaLaws); `lib.rs` (vote_dual, agenda_view/typed/check); `render/mod.rs`
  (`agenda` leaves `uninhabited!`; first resolution kept it, fixed in a follow-up commit).
* **build-speed**: `verify_package.sh` wasm step (took build-speed's opt-in `RUN_WASM=1` over the packages' `SKIP_WASM`);
  package-added `release/` bin paths switched to `gate/`.

Not seen as conflicts, as the brief predicted: duplicate `ti4-engine-core-factions` creation and the frontier-fragment codec
duplicates (git merged them cleanly; the FRONTIER spelling is what is in the tree, verified by the test suite passing).

### Silent losses and semantic breaks (no conflict markers, found by the compiler)

1. **`pub mod core_factions;` vanished from `ti4-engine/src/lib.rs`** in the CE merge (CA/CB declared it near the end with
   `cfg(typed-core)` + `doc(hidden)`; CE/CD moved it up unguarded; git took the deletion without the addition). Restored,
   unguarded, next to `pub mod dual;`. Lesson: grep that each package's new `pub mod` lines survive.
2. **B1 x CE**: B1 added `sustained` and `galvanized` to `kernel::hooks::BorrowedStats`; CE's typed nomad `borrowed_stats`
   built the old shape. It now picks the borrowing piece as the string function does (damaged one when the type cannot
   sustain by itself, else first) and fills both fields.
3. **A2 x B4**: A2 made `core_adapter::adjacency_for` return `Arc<Adjacency>`; B4's `TradeView` held an `Rc`. Changed to `Arc`.
4. **B8 x B9**: render `uninhabited!` list still named `agenda` after the B9 merge (see above).

## API duplications: status after the unification pass

Each item is its own commit on `core-integration` (behavior-neutral; after each: `cargo check --profile gate --workspace
--all-targets` and `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1 cargo test -p ti4-engine`, 4022 passed every time).

* **Done: `typed::<T>(id)`**: eight identical copies (`board_view`, `core_combat`, `core_invasion`, `leaders_view`,
  `agenda_view`, `production_view`, `trade_core`, `explore_core`) replaced by `core_adapter::typed`. (`objectives.rs`'s
  `typed` is a different method and stays.)
* **Done: `has_commander_ability`**: the core one in `rules::leaders` is now `has_commander_ability_with(core, who, commander,
  imperia)`; `has_commander_ability` (via `Cx`/`Env`) and `has_commander_ability_in` (state only, Imperia read off the
  fleet pool by `imperia_from_pool`) are thin callers. The CB/CD/CE copies are re-exports of `_in`; CC's wrapper passes the
  `mahact_imperia_grants` direct hook. Left: the string-side `promissory.rs` and `leaders_view.rs` (string engine, the
  oracle of the dual) and `cc_ground/mahact/query.rs::imperia_grants` (the hook implementation itself; it could call
  `imperia_from_pool` once CC is quiet).
* **Done: test kits**: the four identical `table()` kits of CB/CC/CD/CE are `ti4-engine-core-factions::common::testkit`
  (the per-package `testkit` is a re-export, so no call site changed). The four string-side kits (`bastion`, `crimson`,
  `deepwrought`, `firmament`) share `factions::shared_testkit` (`a/b/c/content/scripted/trio`); each keeps only what is
  its own.
* **Done: relic sustain**: `board_view::relic_grants_sustain` was dead after B5 took the relic dual (nothing called it) and
  is deleted; `query::relic::grants_sustain` is the reader. The combat rules still ask the seam data row
  (`seam::data::grants_sustain`), which is the kernel's way of expressing the same card; two answers on purpose until the
  seam rows replace `query::relic` (note for B1/B5 owners).
* **Done: `Rd` bundles**: B1 and B2 `Rd` are one `kernel::read::Rd` (L3; `rules::combat::Rd` and `rules::invasion::Rd` are
  re-exports). A shared home under `rules/` is impossible: `check_layers.py` rejects a rules area importing another, and a
  `rules::Rd` path counts as one. The L1Z1X commander field is carried by both (None for combat). The other areas
  (production, trade, strategy, agenda, leaders) use their own `Cx` carrying an `Env`, which is a different shape.
* **Kept, documented: the five `Env` traits** (`leaders`, `strategy`, `abilities`, `agenda`, `query::trade`). The seam did not
  replace them: each is still the boundary through which the string engine's views (`leaders_view`, `strategy_view`,
  `agenda_view`, `trade_core`) supply faction answers, and only trade (`KernelEnv`) and agenda (`Facts`) have a kernel-backed
  implementation, with some methods answering a neutral `false`. Merging them would touch every implementor for no gain; they go
  away wholesale when wave C answers through the hook table. Documented in `rules/mod.rs`. `abilities::Env` is implemented by
  `LeaderView` only. The `Faction`/`KernelFaction` traits of A5/B1 were not touched.
* **Still open**: eight near-identical `string-*` feature lines and `AreaMask::build_default` branches; the three-mode
  `board_equivalence` dispatch; the `core_factions` cfg rule (`lib.rs` unguarded, modules `cfg(typed-core)`).

## Build and test status

* `cargo check --profile gate --workspace --all-targets` is clean (warnings only: `core_factions/ca.rs:455` unused `mut`,
  `invasion.rs:10370`, `technology_effects_tests/g4.rs:58`), **excluding** `ti4-tensor`, `ti4-mlp`, `ti4-advisor`, `ti4-review`,
  `ti4-bot-agent`, `ti4-replayer`: `torch-sys` cannot build on this machine (the pinned libtorch is a Windows one). These
  crates do not depend on the changed code.
* `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1 cargo test -p ti4-engine` (debug): 4022 passed, 0 failed, 4 ignored, plus the
  other targets of the crate all green.
* Same env, `ti4-engine-core`, `ti4-engine-core-factions`, `ti4-engine-core-bridge`, `ti4-host`, `ti4-rule-bot`: all green.
* No failing tests, so nothing to assign to an owner. Not run: release golden runs, `dual_run`, `board_equivalence`,
  `gen-cards --check`, wasm, `ti4-selfplay` tests, CPU A/B.

## Recommended next tasks

1. Table-driven `AreaMask::build_default`, `string-*` features and `board_equivalence` modes.
2. Decide the `core_factions` cfg rule and add a CI grep that every package's `pub mod` survives merges.
3. When CC is quiet: `mahact::imperia_grants` calls `imperia_from_pool`; when wave C answers through the kernel, delete the `Env` traits.
4. Re-run the golden dual runs per area on this line before any of it merges to main.

Final checks of the unification pass: `gen-cards --check` current; `check_layers.py core` 0 violations (297 files);
`lint_core.sh` 0 hits; core, factions (68) and bridge tests green with `TI4_AREAS=all=dual`.
