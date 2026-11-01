# TODO D2-7: the legacy split, re-verified against `core-integration-15` (2026-11-01)

Theory document (read-only analysis; no builds, no tests; `cargo tree --offline` only). Branch `core-todo-d27`, cut from `core-integration-15` @ `3ab5e6e64`. Only this file changed.
Replaces the D2-7 text of `docs/todo/D2-prereqs.md` (branch `core-todo-d2`) and `docs/todo/D2-remaining.md` 3.3 (`core-todo-d2b`). The crate-split question for the core is settled (do NOT split the
core: `docs/todo/core-crate-split.md`, branch `core-todo-split`); this file is only about the string engine.

**Labels.** *Measured* = counted in the tree for this document. *Cited* = from a document. *Estimated* = my judgement. **VERIFY** = a check the implementation agent does first (cheap).

**Decisions of the user this file builds on** (2026-11-01): the string engine stays as a crate, `ti4-engine-legacy`, behind the feature `lend-string`, used only for tests and as the comparison oracle, never in
production; Architecture A (the core owns the loop; an embedded legacy `Game` is allowed only as a bridge-only test harness); only the latest `RULES_VERSION` (35) is supported, older saves are refused at
load/recovery; the core must build without `ti4-engine` for the lean mutation ratchet; the flip makes a thin new `ti4-engine` (the crate `ti4-host`, `ti4-server`, `ti4-wasm`, `ti4-rule-bot-typed` use) delegate to the core.

---

## 0. Summary, and what changed against the old plan

1. **The core, the factions and the core tests are already free of `ti4-engine` (measured).** `cargo tree --offline -p ti4-engine-core / -factions / -tests` shows no edge to `ti4-engine`
   (core: `rand*`, dev: `ti4-content`, `ti4-model`; factions: the core only; core-tests default: core, factions, `proptest`). The only remaining ties are one `#[path]` include
   (`ti4-engine-core/tests/adapter.rs:11` -> `../../ti4-engine/src/generated/mod.rs`), the xtask output path (`crates/xtask/src/content_gen/mod.rs:56`), ~40 doc comments in the core, and the optional
   `wrapper` feature of `ti4-engine-core-tests` (one file, `tests/wrapper_props.rs`, 6 `ti4_engine::` lines). **The "lean mutation ratchet without ti4-engine" precondition already holds**:
   `tools/engine_core/d4/mutation_modules.tsv` runs only `ti4-engine-core` and `ti4-engine-core-factions` tests. Nothing has to move for it.
2. **The string engine cannot be cut into a thin half by moving modules (measured).** The module graph of `crates/ti4-engine/src` (text-based `crate::` references, test blocks cut off) is one
   strongly connected block: every one of the 48 modules production crates name (`production`, `objectives`, `technology`, `seating`, `setup`, `choice`, ...) reaches **118 modules / 328 k lines of the
   388 k lines** (84 %). Only `dice`, `rng`, `event`, `observation`, `preview`, `fleet_strength`, `diplomacy` are leaves. So D2-7 is a **rename + facade**, and the thin `ti4-engine` is **new code that grows
   next to the facade** (D2-8b..g, D3); nothing production-reachable can be "extracted" from legacy. The old plan said the same ("whole-crate move + facade first, shrink later"); this file confirms it with numbers and
   adds the consequence for the surface (section 2.4).
3. **`typed-core` is a dead switch (measured).** `Cargo.toml` makes `ti4-engine-core` optional behind it, but 57 unconditionally declared modules (`dual_window`, `core_factions`, `reactions_core`, `live`, ...)
   `use ti4_engine_core::` (231 k lines incl. the big rule files), no dependent turns default features off. `cargo check -p ti4-engine --no-default-features` cannot pass (**VERIFY** once, 3 minutes; E0432 expected).
   Consequence: legacy at the split is **legacy-with-core-wiring** (L1). A pure-string legacy (L2: no dependency on the core) is a separate, optional package D2-7b (section 2.5), not part of the move.
4. **The move needs no directory rename and no freeze (new recommendation).** Cargo allows a directory that differs from the package name. Rename the **package** of `crates/ti4-engine` to
   `ti4-engine-legacy` (the directory stays), put the new thin crate (package `ti4-engine`) in a **new directory** `crates/ti4-engine-next`. Then no tracked file moves: all ~10 branches that edit
   `crates/ti4-engine/src` merge exactly as before, history and blame are untouched, and every path-bound artifact (4 793 refs in 34 files: `test_disposition.tsv`, `curation.json`, the clippy baseline,
   `decision_delivery_inventory.rs`, `adapter.rs`, xtask, the web comments) keeps pointing at the code it describes (the string engine). The old plan's `git mv` of 375 files, the merge of the live branch before the
   move and the freeze window all fall away. A cosmetic `git mv` to `crates/ti4-engine-legacy` stays possible at the very end (section 3.3), when legacy is frozen and nobody edits it.
5. **Nothing has to precede it.** It is behaviour-neutral (no `.rs` body of the engine changes), does not need `disable-xf123` (P1), the bail table (D2-5r) or the wrapper work (D2-8b..g). It should come **first**:
   it gives D2-8 a crate to grow in that no string lane touches, and the bridge its final dependency shape.
6. **Cost.** Split: **3-5 agent-hours**, one commit stack, cheap checks only (old estimate 6-10 / 8-12 h with the move, the live merge and the freeze). Optional L2 (cut the legacy's cord to the core): **10-16 h** (estimated).
   The expensive part of the flip is not the split but the surface the thin crate must give back to host/server/wasm/bot (section 2.4: 48 modules, 309 paths), which D2-8 under-counts.

**Recommendation (details in section 8):** do D2-7 now as the package-rename + facade (section 3.1), merge it into the integration line without a freeze, keep legacy-with-core-wiring (L1) at the split, decide L2 at D2-9.

---

## 1. The tree today (measured on `core-integration-15`)

### 1.1 Crates

| crate | files / lines (`.rs`) | edge to `ti4-engine` |
|---|---|---|
| `ti4-engine` | 375 / 391 036 (4 544 `#[test]`; 4 integration tests, 3 examples, `testdata/tech_waiver_parity.json`) | the thing itself |
| `ti4-engine-core` | 384 / 113 175 | none (comments + `tests/adapter.rs` `#[path]`) |
| `ti4-engine-core-factions` | 194 / 18 397 | none |
| `ti4-engine-core-tests` | 165 / 50 336 | optional (`wrapper` feature: `ti4-engine`, `ti4-engine-core-bridge`, `ti4-model`, `ti4-content`, `serde_json`) |
| `ti4-engine-core-bridge` | 54 / 16 153 (17 bins) | normal dependency `ti4-engine { features = ["typed-core", "native-subject"] }`; feature `observed-count` forwards to it |
| `ti4-rule-bot-typed` | 1 / 64 | normal dependency (`choice` only, 2 lines); also `ti4-rule-bot` |
| `ti4-host` / `ti4-server` / `ti4-wasm` / `ti4-rule-bot` / `ti4-selfplay` | 63 / 114 / 30 / 29 / 12 files | normal dependency; `ti4-host` feature `ts` forwards `ti4-engine/ts`; `ti4-server` and `ti4-wasm` reach the engine only through these |
| 11 other dependents | `ti4-advisor, -bridge, -calc, -cli, -legacy, -mlp, -policy, -replayer, -review, -sim, -training` | normal dependency (`cargo tree -i ti4-engine`: 18 reverse edges in all) |

### 1.2 `use ti4_engine` sites (lines / files)

host 332 / 44, server 157 / 60, wasm 25 / 11, rule-bot 189 / 27, rule-bot-typed 2 / 1, selfplay 13 / 4, **core-bridge 233 / 42**, core-tests 6 / 1, policy 450 / 15, mlp 274 / 62, training 179 / 56,
sim 68 / 8, review 54 / 10, replayer 27 / 10, calc 7 / 2, advisor 4, ti4-bridge 4, ti4-legacy 3, core (doc comments / `adapter.rs`) 4 / 4. At the split **none of them needs an edit** (the facade re-exports
everything); only the bridge is rewritten (section 3.1, commit 2), for clarity, not because it must be.

### 1.3 The bridge crate, the harness that needs both engines

`ti4-engine-core-bridge` is the only crate that has to see legacy, the core and the thin wrapper at once. Today it reaches legacy through `ti4_engine::...` (top paths: `dual` 26, `core_sync` 26, `choice` 26,
`generated` 18, `live` 14, `objectives` 11, `seating` 10, `core_factions` 8, `render` 6). Its parts: `oracle` (the subject trait + `Pilot`; `MaskSubject` = the legacy `Game` under `AreaMask::STRING`),
`native_subject` (a legacy `Game` stepped in shadow, `Game::step_facts` / `observed_windows` behind the engine feature `native-subject`), `native_play` (a whole game by the core alone; the legacy `Game` only
STARTS the game via the string setup and runs the optional `shadow_play`), `dual_run` / `dual_shard` (AreaMask dual), `islands`, `check_kernel_env`, `check_play_facts`, `golden`, `alpha`, `roundtrip`,
`allow_list`, `shadow`, `seen_parity`. It also depends on `ti4-selfplay`, `ti4-rule-bot`, `ti4-rule-bot-typed`, `ti4-host`, `ti4-server`, `ti4-wasm` (so a workspace build with the bridge unifies the features of the
product crates; section 5, R6).

### 1.4 What is inside `crates/ti4-engine/src` by role (measured, lines)

| role | modules | lines |
|---|---|---|
| **glue the thin crate needs** (core <-> string wire) | `core_sync` 7 411, `render` 6 934, `live` 2 825, `resolver_sync` 741, `kernel_sync` 1 000, `product` 208, `seen_load` 141 (+ `reverse_lend` 346, `lend_table` 641 for `lend-string`) | ~20 300 |
| **generated string tables** (xtask output, `adapter_out`) | `generated/` 22 files | 11 006 |
| **wire / surface types** | `choice` 4 679 (Choice, ChoiceOption, Decider, Table, DecisionLog, Observed, `ts` derives), `decision_context` 793 (`ts`), `fingerprint` 384, `event` 239, `rng` 498, `dice` 575, `observation` 56, `preview` 828, `secondary_preview` 414, `reaction_preview` 930, `registry` 287, `fixtures` 343 | ~10 000 |
| **lockstep / dual adapters** (compare or replay the core against the string) | `dual` 1 923, `dual_window` 5 172, `*_dual.rs` (14), `*_core.rs` (9), `*_view.rs` (7), `*_check.rs` (7), `core_adapter` 1 030, `core_combat/invasion`, `window_driver`, `menu_*`, `churn`, `cannon_dual`, `combat_live`, `invasion_live`, `production_live`, `tactical_live`, `explore_write`, `trade_write`, `turn_write`, `strategy_write`, `objectives_write`, `core_factions/` (3 665), `d1_*` | ~45 000 |
| **string rule bodies** | `game` 19 374, `combat` 17 222, `action_cards` 15 030, `invasion` 13 886, `production` 9 468, `strategy_cards` 6 351, `leaders`, `exploration`, `objectives`, `technology`, `transactions`, `diplomacy/`, `factions/` (92 688), ... | ~300 000 |
| **in-source tests** (inside the files above; `#[cfg(test)]` blocks and `*_tests.rs`, `*_live_tests.rs`, `faction_effects_tests/`, `technology_effects_tests/`) | 4 544 `#[test]`: **U 3 353** (module unit tests), **G 657** (Game-driven depth tests), **L 358** (lockstep, `compared` / `dual::serial`), **C 38** (codec / kernel_sync), **S 138** (string machinery) | cited: `docs/D4_TEST_DISPOSITION.md` |

Macro call sites that tie the rule bodies to the adapters: **~440-483** `dual!` / `dual_typed!` / `dual_window!` / `dual_query!` / `dual_hook!` sites (`tools/engine_core/lent_sites.tsv`, cited by `D2_8_0_AUDIT.md`);
in the non-adapter modules **145 lines** name `ti4_engine_core::` directly and ~1 000 lines name an adapter module (about a third of them inside macro arguments).

### 1.5 The product surface of `ti4-engine`

Distinct engine modules named by production code (tests, examples, `#[cfg(test)]` cut off): host 44, rule-bot 15, server 11, selfplay 5, wasm 4, rule-bot-typed 1 -> **48 distinct modules, 309 distinct paths**
(`tools/engine_core/boundary_inventory.py`: "engine paths used by any consumer (prod): 309 in 57 modules"). Host alone names 158 distinct paths. By kind (host + server + wasm + rule-bot, exact paths in the appendix):
wire constants and kinds (`production::PRODUCE_KIND`, `game::CONTINUE_ACTION_ID`, `invasion::COMMIT_KIND`, `seating::LOBBY_FACTIONS`, ...), types (`Game`, `Roll`, `Spend`, `Imagined`, `Movable`, `Relation`, `MapEdit`, ...),
**pure queries over a `GameState`** (`technology::prerequisites`, `planets::in_system`, `objectives::points_for`, `production::capacity`, `tactical::ship_routes`, `fleet_strength::matchup`, ...) and **mutating helpers**
(`promissory::take/receive/support`, `exploration::explore/gain_fragment`, `setup::start_game*`, `movement::replay_map_edits`, `laws::apply_to_galaxy`). `rule-bot` (string) additionally holds 500 `obs()` call sites on `Observed`.

---

## 2. Target layout

### 2.1 Crates and dependency rules

```text
ti4-model  <-  ti4-content  <-  ti4-engine-core  <-  ti4-engine-core-factions            (no edge to any ti4-engine*)
                                      ^                          ^
                                      |                          |
        ti4-engine-legacy -----------+ (L1: typed-core, hard dep; L2: none)                 frozen string engine = crates/ti4-engine (dir)
        ti4-engine   (thin)  --------+--- optional `lend-string` --> ti4-engine-legacy    = crates/ti4-engine-next (dir)
                 ^   ^   ^   ^
   host, server, wasm, rule-bot-typed, rule-bot (string, until D3), selfplay, policy, ...  -> ti4-engine only
   ti4-engine-core-bridge  -> ti4-engine-legacy (always), ti4-engine (+ lend-string), core, factions, host, server, wasm, selfplay, rule-bot*
   ti4-engine-core-tests   -> core, factions;  feature `wrapper` -> ti4-engine, bridge
```

Rules (enforce with `cargo tree`, section 6): (a) `ti4-engine-core*` never depends on `ti4-engine*`; (b) `ti4-engine-legacy` never depends on `ti4-engine` (no cycle with `lend-string`); (c) in production
(`cargo tree -p ti4-server`, `-p ti4-wasm`) there is no `ti4-engine-legacy` after D2-9 (during the facade stage there is: it is the engine); (d) only the bridge, `ti4-engine-core-tests[wrapper]` and the legacy's own tests
may name `ti4_engine_legacy`.

### 2.2 `ti4-engine-legacy` (today's `crates/ti4-engine`, package renamed)

* **Everything** of today's crate: all 375 files, `tests/`, `examples/`, `testdata/`, all 4 544 tests. Frozen after the flip: rule fixes go to the core (policy 6.5 of `ENGINE_CORE_STATE.md`); legacy changes only to keep the oracle building.
* Features unchanged at the split: `default = ["typed-core"]`, `typed-core`, `ts`, `observed-count`, `native-subject`, `string-movement|objectives|factions|trade|exploration|strategy|action-cards|agenda`. Add `oracle` (D2-prereqs D2-7 item 5) only when the bridge needs
  `build_default()` to be `STRING` unconditionally (T-G: legacy compiled with its own wiring must run `AreaMask::STRING` or the oracle compares the core with itself). **VERIFY** the current `AreaMask::build_default()` (`dual.rs:172`).
* `ts` can be dropped from legacy once the thin crate owns the wire types (D2-8); until then legacy has it (the facade forwards it).
* Lockstep tests (L 358) and the dual/adapter modules **stay inside legacy**: they use `pub(crate)` string internals and the core at once; hoisting them to an integration crate would mean a rewrite of ~360 tests against a public API that
  does not exist. They run only in the oracle gate (`cargo test -p ti4-engine-legacy`), no longer in `t-web` after D2-9.
* U 3 353 tests stay (they test string module internals; the oracle's corpus is what makes them useful). S 138 stay. G 657 are the D2-8 exit criterion ("run unchanged against the flipped wrapper"): they cannot run against the thin
  crate from inside legacy; see 2.6.

### 2.3 `ti4-engine` (thin; new directory `crates/ti4-engine-next`)

**At the split (facade stage):** `Cargo.toml` with `ti4-engine-legacy` as a normal dependency and the forwarded features (`default = ["typed-core"]`, `typed-core`, `ts`, `observed-count`, `native-subject`, the eight `string-*`);
`src/lib.rs` = `pub use ti4_engine_legacy::*;`. An explicit `pub mod x;` in the thin crate shadows the glob re-export of the same name, so the crate can be populated module by module while the rest still comes from legacy.
Shadow only modules whose types do not cross into legacy functions (`generated`, `render`, `live`, `core_sync`, `kernel_sync` first); the wire types (`choice`, `decision_context`, `fingerprint`) change owner only with the `Game` (D2-8).

**End state (after D2-8/D2-9):** the crate holds, and nothing else:

| part | content | source |
|---|---|---|
| wrapper `Game` | owns `CoreState`; `step`/`answer`/choices from the core driver; `state()` = cached materialisation; checkpoint = `CoreState::clone` + decision log; `Unmapped` is an error | new (D2-8) |
| glue | `core_sync` (codec, `materialize`, `unmapped`, `unsupported`, `guard`, `texts`, `footprint`), `render/*`, `live/*`, `resolver_sync`, `kernel_sync`, `product`, `seen_load` | **copied** out of legacy at D2-8b (a `cp`; history is in git; the two copies diverge on purpose) |
| wire and save | `choice` (Choice, ChoiceOption, Decider, Table, DecisionLog, Observed), `decision_context`, `fingerprint`, `event`, `rng`, `dice`, previews, `registry`, `fixtures`; the `ts` feature (`ts-rs` derives; wire TS must stay byte-identical) | copied, then owned |
| tables | `generated/` (string tables of the core ids) | one source file set, see 2.7 |
| surface for host / server / wasm / bots | the 48 modules of 1.5, each as a delegation to the core's query layer or to the materialised `GameState`, no string fallback | new, D2-8c/d + D3 |
| `lend-string` | `reverse_lend`, `lend_table`, `Game::with_string` (leftover string bodies on a materialised state) | gated by the feature |

Dependencies: model, content, core, factions, serde, serde_json, sha2, rand (no default features: no `getrandom`), rand_chacha, thiserror, `ts-rs` (optional), and **optional** `ti4-engine-legacy` (only through `lend-string`).

Features: `default = []`; `ts = ["dep:ts-rs", "ti4-model/ts"]`; `observed-count = ["ti4-model/observed-count"]`; `lend-string = ["dep:ti4-engine-legacy"]`. No `typed-core`, no `string-*`, no `native-subject` (that one is a legacy accessor feature,
enabled by the bridge on `ti4-engine-legacy`). `lend-string` is **off in every production build**; server and wasm may enable it temporarily only while the lent census is non-empty (phase a of `D2-prereqs.md` D2-9), the user's
decision makes the end state "never". The bridge and the oracle enable it.

### 2.4 What the thin crate must give back (the real size of the flip, outside D2-7)

Because legacy is one block (0, item 2), each of the 48 modules / 309 paths of 1.5 needs an owner in the thin crate **before** `ti4-engine` can stop re-exporting legacy: constants and kinds are `const` copies (cheap, ~50 items);
pure queries over a `GameState` are either core `query::*` delegations (many already exist behind `*_view.rs`: `board_view`, `production_view`, `technology_core`, `objectives_core`, `leaders_view`, `strategy_view`, `agenda_view`,
`action_cards_view`) or small copies; the mutating helpers go through core entry points. Setup (`setup`, `seating`, `deck`, `opening`, `starting_tech`, `map_feasibility`, `fixtures`) is the other large piece: `native_play` still starts a game
through the string setup ("the only place the legacy `Game` is used to start a game: bridge only"). Under "legacy never in production" that setup needs a core/wrapper owner too. **D2-8 should list these per path** (the appendix is the
starting list; regenerate with `boundary_inventory.py` + the grep of the appendix) and `D2-remaining.md` 3.4's 22-35 h (B) / 53-81 h (A) do not include a per-path surface sweep beyond `game.state` (item 7 of D2-8 in `D2-prereqs.md`).

### 2.5 Legacy with or without the core (L1 vs L2), and the maintenance tax

* **L1 (the split as is):** legacy keeps `typed-core`, the wiring and its own copies of the glue. Cost: the legacy glue (`live`, `dual_window`, `core_factions`, `reactions_core`, ... ~2 500 `ti4_engine_core` references in 216 files) compiles against the
  **live** core. Whenever a core lane changes a type the glue builds (INTEGRATION15 B: a new field `Out { held }` needed `held: false` in every literal) or `cargo xtask gen-cards` adds a variant, legacy must be repaired too, in a crate that is
  supposed to be frozen. Also every `RULES_VERSION` fix that deliberately changes the core makes an L-test of legacy fail (allow-list / `oracle` feature handle that per version: cited `D4_TEST_DISPOSITION.md`).
* **L2 (D2-7b, optional):** legacy depends on nothing of the core. Mechanics (**estimated 10-16 h**, mechanical but wide): (1) redefine the five macros so the `$core` argument is not expanded (unused macro arguments are never type-checked, so
  the ~440 sites compile without touching them); (2) gate the ~45 adapter modules and the glue behind `typed-core` (default OFF in legacy); (3) fix the remaining ~145 direct `ti4_engine_core::` lines and the non-macro adapter references of
  the 14 rule files (`factions`, `invasion`, `d1_pending`, `production_window_dual`, `leaders`, `combat`, `strategy_cards`, `exploration`, `production`, `game`, `relics`, `technology`, ...); (4) the L / C tests become `cfg(feature = "typed-core")` and are
  deleted from the default run (the oracle in the bridge replaces them); (5) `cargo check -p ti4-engine-legacy` with default features must not list `ti4-engine-core` in `cargo tree`. The oracle then compares a pure string engine with the thin crate and
  the T-G trap disappears. Edits to the frozen crate are limited to cfg gates and the macros (no rule body).
* **Recommendation:** L1 at the split; schedule L2 inside D2-9 (it is the same operation as "strip the dual wiring" for legacy) unless the tax of one real core API change shows earlier that it is worth doing first.

### 2.6 Tests: where which kind lives

| kind | home after D2-7 | after D2-9 |
|---|---|---|
| U 3 353, S 138, L 358 | `ti4-engine-legacy` (in-source) | same, not in `c/l/t-web` (alias `t-legacy` and the oracle gate) |
| G 657 (Game-driven) | legacy (as today) | **hoisted** into thin `ti4-engine` tests as integration tests over the public API (D2-8 exit criterion): list them from `tools/engine_core/d4/test_disposition.tsv` (column 4 = `G`); many use `pub(crate)` helpers: expect a rewrite of part of them |
| C 38 (codec, kernel_sync) | legacy | move with the glue copy to the thin crate (`tests/decision_replay.rs`, `tests/reaction_corpus.rs` are glue tests, they move with `render` / `kernel_sync`) |
| `tests/content_ids_resolve.rs`, `tests/decision_delivery_inventory.rs` | legacy (they scan their own `CARGO_MANIFEST_DIR/src`; unchanged and green) | the thin crate gets its own copy of the delivery inventory for its `Choice` producers when they exist (trap T3) |
| core, factions, core-tests | unchanged | unchanged; `wrapper_props.rs` uses the thin crate |
| bridge bins and tests | bridge | bridge |

### 2.7 `generated/` has two consumers

`crates/xtask` writes the string tables to `crates/ti4-engine/src/generated` (`adapter_out`), `ti4-engine-core/tests/adapter.rs` includes that file set by `#[path]`, legacy (L1) and the thin crate both compile it. **Do not emit two copies.**
Default: keep `adapter_out` where it is and let the thin crate include it with the same `#[path = "../../ti4-engine/src/generated/mod.rs"] pub mod generated;` (the repo already does this in `adapter.rs`). Alternative: a tiny crate `ti4-engine-names`
(generated tables only, depends on the core) used by legacy, thin, bridge and `adapter.rs` as a dev-dependency; better compile parallelism, one more crate, `xtask` `adapter_out` and `--check` change once. Pick at D2-8b, not now.

---

## 3. The move: replayable script plans

### 3.1 Plan P (recommended): package rename + facade, no file moves

Branch `core-d2-7-split` in `.claude/worktrees/core-d2-7`, cut from the then-latest `core-integration-N`. Commit stack (each commit compiles):

| # | commit | edits |
|---|---|---|
| 1 | `D2-7: ti4-engine-legacy package + thin ti4-engine (facade)` (one commit: renaming the package alone leaves every dependent unresolved) | `crates/ti4-engine/Cargo.toml`: `name = "ti4-engine-legacy"` (lib name becomes `ti4_engine_legacy`; nothing else). New `crates/ti4-engine-next/{Cargo.toml,src/lib.rs}` (below). Workspace `Cargo.toml`: `members += "crates/ti4-engine-next"`; `[workspace.dependencies]` `ti4-engine = { path = "crates/ti4-engine-next" }` (was `crates/ti4-engine`) and `ti4-engine-legacy = { path = "crates/ti4-engine" }`; `Cargo.lock` via `cargo metadata --offline` |
| 2 | `D2-7: the bridge names legacy` | `ti4-engine-core-bridge/Cargo.toml`: `ti4-engine-legacy = { workspace = true, features = ["typed-core", "native-subject"] }` next to `ti4-engine = { workspace = true }`; feature `observed-count = ["ti4-engine-legacy/observed-count", "ti4-model/observed-count"]`; sources (mechanical): the sed below |
| 3 | `D2-7: aliases and tools` | table 3.4 |
| 4 | `D2-7: crates/ti4-engine/README.md` | five lines: this directory is the package `ti4-engine-legacy`; the product crate `ti4-engine` is `crates/ti4-engine-next`; why; pointer to this file |

Thin crate (facade stage):

```toml
# crates/ti4-engine-next/Cargo.toml
[package]
name = "ti4-engine"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
ti4-engine-legacy.workspace = true   # facade stage: the engine itself; becomes optional (`lend-string`) in D2-8/D2-9

[features]
default = ["typed-core"]
typed-core = ["ti4-engine-legacy/typed-core"]
ts = ["ti4-engine-legacy/ts"]
observed-count = ["ti4-engine-legacy/observed-count"]
native-subject = ["ti4-engine-legacy/native-subject"]
string-movement = ["ti4-engine-legacy/string-movement"]
# ... string-objectives, -factions, -trade, -exploration, -strategy, -action-cards, -agenda (same pattern)

[lints]
workspace = true
```
```rust
// crates/ti4-engine-next/src/lib.rs
//! The product-facing engine crate. Facade stage: everything is re-exported from `ti4-engine-legacy` (crates/ti4-engine, docs/todo/D2-7-legacy-split.md).
pub use ti4_engine_legacy::*;
```

Bridge source rewrite (commit 2; behaviour-neutral in the facade stage, makes the final dependency shape explicit). Mapping by module of the first path segment:
legacy-only -> `ti4_engine_legacy`: `dual, dual_window, core_factions, *_dual, *_core, *_view, *_check, core_adapter, window_driver, menu_*, d1_*, objectives, seating, transactions, action_cards, promissory, tactical, synergy, strategy,
secrets, production, movement, entropic_scars, transit, technology, space_stations, planets, laws, leaders, faction_abilities, reach_check, game, lend_table`;
thin -> `ti4_engine`: `core_sync, generated, live, render, product, seen_load, kernel_sync, rng, fingerprint, choice, decision_context`.
```bash
# in crates/ti4-engine-core-bridge/{src,tests}: one sed per module, e.g.
sed -i -E 's/\bti4_engine::(dual|dual_window|core_factions|objectives|seating|transactions|action_cards|promissory|tactical|synergy|strategy|secrets|production|movement|entropic_scars|transit|technology|space_stations|planets|laws|leaders|faction_abilities|reach_check|game|[a-z_]+_(dual|core|view|check)|core_adapter)\b/ti4_engine_legacy::\1/g' $(git ls-files 'crates/ti4-engine-core-bridge/*.rs')
```
`choice` is the ambiguous one (both engines have a `Choice` after D2-8): leave it on `ti4_engine` and let the oracle compare through serde/ids (it already compares `DecisionContext::canonical` and hashes). **VERIFY** with `cargo check -p ti4-engine-core-bridge --all-targets`; a miss is a compile error, never a silent change.

### 3.2 Plan D (fallback): directory rename, as the old plan

Only if the user wants `crates/ti4-engine-legacy` at once. `git mv crates/ti4-engine crates/ti4-engine-legacy` (375 files, 100 % similarity, no content edit in the same commit), then the package name, then the new `crates/ti4-engine` (facade) in a second commit, then the path fixes
(34 files / 4 793 refs; of which 4 544 are the rows of `test_disposition.tsv` and 131 `curation.json`: `sed -i 's#crates/ti4-engine/#crates/ti4-engine-legacy/#g'` on the files of the table 3.4 column "path"; do **not** edit generated manifests under
`web/e2e/screenshots/*/manifest.json`, `docs/`, `plans/`, `web/plans/`, `dirty-snapshot.patch`). Merge consequences (3.5): the two files that exist on both sides (`Cargo.toml`, `src/lib.rs`) conflict for every branch that edits them; added files in the old directory
are NOT moved by git (the old directory still exists); everything else follows the rename.

### 3.3 Optional last step (P2): the cosmetic rename when legacy is frozen

After D2-9, when nobody edits legacy: `git mv crates/ti4-engine crates/ti4-engine-legacy` (pure rename commit, then the path fixes of 3.2), then `git mv crates/ti4-engine-next crates/ti4-engine` (the active crate: branches editing it follow
the rename because the old path no longer exists on that side). Two pure-rename commits, `git merge -X find-renames=40` and `-c merge.directoryRenames=true` for the branches still in flight. Skip it if the directory/package mismatch is acceptable: it only costs a README.

### 3.4 Aliases, scripts and tools (edit list, plan P)

| file | edit |
|---|---|
| `.cargo/config.toml` `c-web`, `l-web`, `t-web`, `b-web`, `wire-web` | add `-p ti4-engine-legacy` (facade stage: it is built anyway, `-p` keeps its 4 544 tests and `--test content_ids_resolve` in the set; `b-web` needs it for the test target). The "one fixed package set" rule of the comment: change all five together |
| `.cargo/config.toml` `t-web-slow` | add `profile.dev.package.ti4-engine-legacy.opt-level=1` (**silent perf trap**: the `ti4-engine` key would now apply to the 1-line facade), plus `ti4-engine-core` and `-factions` if they are in the slow set |
| `Cargo.toml` `[profile.*]` | no package keys for the engine (measured): nothing |
| `tools/engine_core/clippy_core.sh` | `-p ti4-engine` -> `-p ti4-engine-legacy -p ti4-engine`; the engine baseline (41 lines) is keyed `crates/ti4-engine/src/...` and stays valid (legacy keeps the directory); add `crates/ti4-engine-next` to `core_re` (zero tolerance) |
| `tools/engine_core/verify_package.sh:96,105`, `verify_stage.sh:40`, `end_of_d1.sh:84-85`, `target_dir.sh:29`, `test_parallel.sh:4` | `-p ti4-engine` where it means "the string engine's tests" -> `-p ti4-engine-legacy`; add `-p ti4-engine` where the product set is meant |
| `verify_stage.sh:31` | `--features ti4-engine/string-movement` still works through the forwarded feature |
| `tools/nightly-smoke/prewarm.sh:48`, `test_prewarm.sh:43` | `-p ti4-server -p ti4-engine --no-run`: add `-p ti4-engine-legacy` (prewarm builds the alias set) |
| `tools/card_audit/card_audit.py:20`, `engine-rules-audit.md`, example docs | `cargo run -p ti4-engine --example ...` -> `-p ti4-engine-legacy` (the examples live in legacy) |
| `tools/engine_core/{check_layers.py,boundary_inventory.py,lent_sites.py,marks_census.py,d4/coverage.py,d4/test_inventory.py}`, `tools/engine_profile/module_graph.py`, `scripts/make_faction_reference.py` | path `crates/ti4-engine/src` stays correct (legacy). No edit |
| `tools/engine_core/mutants.sh`, `d4/mutation_modules.tsv`, `mutation_ratchet.sh`, `mutation_floor.json` | none (core and factions only; the script reads the package name from `Cargo.toml`) |
| `crates/xtask/src/content_gen/mod.rs:56`, `ti4-engine-core/tests/adapter.rs:11` | none now (2.7); touched at D2-8b |
| `deploy/public/deploy.sh:178`, `README.md`, `check-headers.sh`, `web-shared/scripts/build-wasm.mjs` | none (`-p ti4-server`, `-p ti4-wasm`) |
| `crates/ti4-engine-core/Cargo.toml:13`, ~40 doc comments naming `ti4-engine/src/render/...` | leave; fix when the glue copy lands in the thin crate (D2-8b) |

### 3.5 History and later merges

* **Plan P:** no tracked path changes, so `git log --follow`, blame and every merge are unchanged. Only `Cargo.toml` (workspace), `Cargo.lock`, `crates/ti4-engine/Cargo.toml` (one line), `.cargo/config.toml` and a handful of tools change. A branch
  that edits one of those lines conflicts trivially. Measured: of the in-flight branches only `core-integration-16` (1 engine file), `core-q-research` (2), `core-n-waiver` (1), `core-p-shadow2` (3), `nightly-fixes-2026-10-26` (1, `promissory.rs`) touch
  `crates/ti4-engine/`; **none touches `Cargo.toml` or `src/lib.rs` of the engine, the workspace `Cargo.toml` or `.cargo/config.toml`** (`git diff --name-only <merge-base> <tip>`). Expected conflicts: 0.
* **Plan D:** files that exist on both sides conflict (lib.rs, Cargo.toml); everything else follows the rename if `merge.renames` is on and the files stay >= 50 % similar (pure rename: 100 %). Use `git merge -X find-renames=40 -c merge.directoryRenames=true`;
  a file a branch ADDS under the old directory lands in the new thin crate (the directory still exists) and is not compiled: the build fails on the legacy `lib.rs` line that refers to it, so it is noticed.
* The live branch `nightly-fixes-2026-10-26` is not touched by this work. When the flip finally reaches it, engine-rule fixes made on it after the flip hit legacy, i.e. dead code in production: that is the policy of `ENGINE_CORE_STATE.md` 6.5 (re-apply in the core), and the merge of live into the integration line must happen right before and right after the flip.

---

## 4. Order inside the split and what the thin crate gets next (not D2-7)

D2-7 ends with the facade. The next packages fill `crates/ti4-engine-next` in an order that keeps the workspace green at every commit (shadowing, 2.3): (1) `generated` (via `#[path]`, 2.7), (2) glue copy `core_sync`, `kernel_sync`, `render`, `live`, `resolver_sync`, `product`, `seen_load` with the
`dual` ties cut (12 + 2 + 8 references in non-test code of `live` / `render` / `core_sync`: `dual_window::complete_nested`, `nested_position`, `NestedPos`, `exploring_dual::codes/words`, `dual::note_bail`, `render/events::Pushed`; **measured**), (3) constants and kinds,
(4) wire types with the wrapper `Game` (D2-8), (5) the per-path surface (2.4), (6) `lend-string` split out, (7) `legacy` becomes optional, (8) L2 and the strip (D2-9). Items (2)-(8) are D2-8b..9, unchanged in intent.

---

## 5. Risks

| # | Risk | Where | Answer |
|---|---|---|---|
| R1 | serde / `RULES_VERSION` / fingerprints change | `RULES_VERSION` is `ti4-model/src/state.rs:1232` (= 35); no `.rs` body of legacy changes at the split | `git diff --stat <base>..HEAD -- 'crates/**/*.rs'` shows only `crates/ti4-engine-next/**` and the bridge sources; `cargo wire-web` + `git diff --exit-code web-shared/src/wire/generated fixtures/` empty; `cargo xtask gen-cards --check`; bench hashes 200/200 (`tools/engine_profile/verify_benches.sh`). With "only v35 supported" the `bench-v1..v5` (rules versions 1-5, 23) rows of G7 and the golden v1..v5 configs drop out of the new engine's gate; the oracle corpora are restricted to 35 |
| R2 | `pub use ti4_engine_legacy::*` misses something | `#[macro_export]` macros (`dual!`, `dual_window!`, ... are used by the bridge), `pub(crate)` is not re-exported (intended), items shadowed by name | **VERIFY**: `cargo check --workspace --all-targets` (excluding the torch crates, as INTEGRATION15 does); if a macro is missing add `pub use ti4_engine_legacy::{dual, dual_typed, dual_query, dual_window, dual_hook};` |
| R3 | wasm size and compile time | facade = zero code; LTO sees the same crate graph | wasm size report before / after must be identical to the byte (`scripts/wasm-size-gate.mjs`, budget 2.4 MB gzip / 1.8 MB brotli, `size-budget.json`). The size and build-time **gain** (legacy out of `c/l/t-web`, out of the wasm) arrives at D2-9, not here. The thin crate adds one trivial crate to the critical path (<1 s) |
| R4 | aliases lose coverage / the slow tier silently slows | `.cargo/config.toml`, `t-web-slow` package keys | table 3.4; the cheap check "list the tests": `cargo t-web --no-run` and `cargo test -p ti4-engine-legacy --lib -- --list | wc -l` = 4 544 (+ the 4 integration tests) |
| R5 | confusion: directory `crates/ti4-engine` is the package `ti4-engine-legacy`; `crates/ti4-legacy` is the unrelated Python-converter stub; thin = `crates/ti4-engine-next` | agents, docs, CLAUDE.md | README in the directory; always write the full package name; ask the user to add one line to CLAUDE.md (I do not edit it); plan P2 removes the mismatch |
| R6 | feature unification pulls legacy into production builds | `lend-string` (end state) and `native-subject` are enabled by the bridge; `cargo build --workspace` unifies | production builds use `-p ti4-server` / `-p ti4-wasm` (measured: `deploy.sh:178`, `build-wasm.mjs`); end-state check: `cargo tree -p ti4-server -e normal | grep -c ti4-engine-legacy` = 0 and the same for `-p ti4-wasm` |
| R7 | legacy glue rots against the evolving core (L1) | 2.5 | L2, or accept the tax: a legacy compile break after a core API change is fixed in the same commit, with the core lane |
| R8 | CI / script drift | `tools/engine_core/*.sh`, `native_play.sh` (`-p ti4-engine-core-bridge --bin native_play`: unchanged), `end_of_d1.sh`, `mutants.sh` (unchanged) | table 3.4; after the commit stack run `grep -rn "\-p ti4-engine\b" tools .cargo` and decide each hit |
| R9 | clippy baseline (engine 41) | `clippy_core.sh` | stays keyed to `crates/ti4-engine/`; thin crate at zero; baseline only shrinks |
| R10 | the nightly rule | `CLAUDE.md` Time | the integration line is not the live checkout; a facade split is not a sweep input. When the flip merges into the live branch, it must be in before 20:30 Berlin on a night one wants it tested, and never in the last hour (`D2-prereqs.md` T13) |
| R11 | a lane adds a feature to `ti4-engine` between the split and D2-8 | `crates/ti4-engine/Cargo.toml` | tell the lanes: new features go on `ti4-engine-legacy` AND are forwarded by the thin crate; a compile error in a dependent that uses `--features ti4-engine/x` shows it at once |

---

## 6. Ordered work list for the implementation agent (D2-7, plan P)

Branch `core-d2-7-split`, worktree `.claude/worktrees/core-d2-7`, private `CARGO_TARGET_DIR=/root/deploytmp/split-d27` (delete afterwards), `CARGO_BUILD_JOBS=2`; heavy runs under `flock /root/.ti4-heavy.lock`.

0. **Baseline (15 min).** Record on the base commit: `cargo metadata --offline --format-version 1 > base.json`; `cargo tree --offline -p ti4-server -e normal --prefix none | sort > tree-server.base`, same for `-p ti4-wasm`; `cargo test -p ti4-engine --lib -- --list | wc -l` (expect 4 544 + `--test` lists); the wasm size (`wasm-release` build, `scripts/wasm-size-gate.mjs`, one build).
   **VERIFY** `cargo check --offline -p ti4-engine --no-default-features` fails (claim 3 of section 0) and `AreaMask::build_default()` at `dual.rs:172`.
1. **Commit 1 (package + facade).** Section 3.1. Check: `cargo metadata --offline` resolves; `cargo check -p ti4-engine -p ti4-host -p ti4-server -p ti4-wasm -p ti4-rule-bot-typed` (the `c-web` set).
2. **Commit 2 (bridge).** Section 3.1. Check: `cargo check --workspace --all-targets` minus `ti4-review, ti4-replayer, ti4-mlp, ti4-tensor, ti4-advisor` (the exclusion list of INTEGRATION15 section 1) is green; `cargo test -p ti4-engine-core-bridge --no-run`.
3. **Commit 3 (aliases, tools).** Table 3.4. Check: `cargo t-web --no-run` and `cargo l-web` (the five aliases parse and select the same test targets as before: compare `--list` counts); `bash -n` on every edited script; `tools/engine_core/test_verify_benches.sh` (it has tests); `python3 -I tools/engine_core/check_layers.py core` unchanged.
4. **Commit 4 (README).**
5. **Definition of done (all cheap):**
   * `git diff --stat <base>..HEAD` lists only: `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, `crates/ti4-engine/Cargo.toml` (1 line), `crates/ti4-engine/README.md`, `crates/ti4-engine-next/*` (2 files), the bridge `Cargo.toml` + sources, the tool edits of 3.4. **No other `.rs` file.**
   * `cargo tree --offline -p ti4-server -e normal --prefix none | sort` equals `tree-server.base` plus the line `ti4-engine-legacy` and the `ti4-engine` path/dir change; `git grep -n "crates/ti4-engine/" -- . ':!docs' ':!plans' ':!web/plans'` unchanged (all refs still mean legacy and are right).
   * `cargo tree --offline -p ti4-engine-core -p ti4-engine-core-factions -p ti4-engine-core-tests` contains no `ti4-engine`, `ti4-engine-legacy` (rule 2.1a), and `cargo tree -p ti4-engine-legacy -e normal | grep -c ti4-engine-core` is 1 or 2 (L1: the core and the factions, as today) and `grep -c "ti4-engine "` = 0 (rule 2.1b).
   * `cargo wire-web` leaves `git status` clean; `cargo xtask gen-cards --check`; `cargo test -p ti4-engine-core -p ti4-engine-core-factions` (core is untouched, so only a sanity run); wasm size equal to the baseline (R3).
   * One heavy run, the bench hashes: `tools/engine_profile/verify_benches.sh` with `selfplay` built from the facade (`bench` 200/200, `bench-v1..v5` only if still wanted): identical to `tools/rulebot/baseline*.json`. Optional `native_play.sh --quick` fingerprint `2e93df9fa00fa982` / 1 198 steps (cited: INTEGRATION15 6.1).
6. **Hand-back note:** the new aliases, the crate directory facts (R5), "new features go on legacy and are forwarded" (R11), and the list of what D2-8b will copy (1.4, 2.3).
7. **D2-7b (L2, optional, separate branch, after D2-8b so the glue copy exists):** 2.5. Cheap checks: `cargo check -p ti4-engine-legacy --no-default-features`; `cargo tree -p ti4-engine-legacy | grep -c ti4-engine-core` = 0; oracle smoke: `golden check` and 2 games of `oracle`; the bench hashes again.

Cheap-check summary (each under a few minutes, no gate): `cargo metadata`, `cargo tree`, `cargo check` of the `c-web` set and of the workspace, `cargo test --no-run`, `--list` counts, `bash -n`, `python3 -I` of the tool scripts, `git diff --stat`.

---

## 7. Effort and conflict risk

| item | estimate (agent-h) | conflict risk with lanes in flight |
|---|---:|---|
| D2-7 plan P (steps 0-6) | **3-5** (+ ~1 h wall for the checks and one bench run) | none: no file moves; `core-integration-16`, `core-q-research`, `core-n-waiver`, `core-p-shadow2` touch 1-3 files each in `crates/ti4-engine/src` (`strategic_live_tests`, `combat_live`, `strategy_research_tests`, `kernel_sync/shim`, `reactions_core`) and none of the files plan P edits. Merge in any order |
| D2-7 plan D (fallback) | 6-10 | lib.rs / Cargo.toml conflicts only; renames follow; needs the dir-rename merge flags; recommend merging `core-integration-16` and `core-q-research` first |
| P2 cosmetic rename | 1-2 | none if legacy is frozen; active thin-crate lanes follow the rename |
| D2-7b (L2) | 10-16 (estimated) | edits to the frozen crate's adapter files: do it when `combat_live`, `reactions_core`, `strategic_live_tests` etc. have no open lane (after the q / p / n lanes) |
| surface sweep of 2.4 (inside D2-8, unestimated by the old plan) | add to the 22-35 / 53-81 h of `D2_8_0_AUDIT.md` 6; **estimated** 8-16 h for the 309 paths and the setup | the thin crate is a new directory, so no string lane conflicts |

**Freeze.** Plan P needs none. If plan D is chosen, freeze `crates/ti4-engine/` only for the duration of the commit stack (about 2 h): announce to the coordinator, merge the open lanes first, then the split. The live branch needs no merge in either plan.

---

## 8. Recommendation

1. **Do D2-7 now, as plan P** (package rename + facade, new directory `crates/ti4-engine-next`, no `git mv`). It satisfies the user's decisions as written: the package is `ti4-engine-legacy`, the product-facing `ti4-engine` exists and every
   production crate already depends on it; the core, factions and core-tests do not depend on either; legacy reaches production only through the thin crate's dependency, which becomes optional (`lend-string`) and then off.
2. **Keep legacy-with-core-wiring (L1) at the split** and put L2 (cut the cord) on the D2-9 list; the first real core API change that breaks the legacy glue moves it forward.
3. **Do not try to extract a thin crate from legacy by moving modules** (one 118-module block); grow `crates/ti4-engine-next` by shadowing re-exports module by module, starting with `generated`, the glue copy and the constants.
4. **Add the per-path surface sweep (2.4, appendix) and the setup owner to D2-8's scope and estimate;** it is the largest item not yet counted.
5. **Hoist the G tests (657) into the thin crate as the D2-8 exit criterion,** keep U / S / L in legacy.

**Questions for the user (ask once; defaults stated):**
(a) Plan P (directory/package mismatch until D2-9) or plan D (rename at once, 6-10 h, short freeze)? *Default: P.*
(b) The setup (`setup`, `seating`, `opening`, `starting_tech`) is the one string piece the host calls at game creation: native in the thin crate by D2-8, or acceptable behind `lend-string` until zero-bail? *Default: native, same deadline as the rest.*
(c) Is `lend-string` allowed in the server / wasm builds at the flip (phase a) or must the first flipped build already be free of legacy? *Default: allowed only until the lent census is empty, then removed.*

---

## Appendix A. Production paths of `ti4_engine::` (host + server + wasm + rule-bot, non-test, top counts)

`choice::*` and `decision_context::*` (wire types; host 74, server 59, rule-bot 72, wasm 17) are not listed. The rest, by module, as `path(count)`:

constants and kinds: `strategy_cards::card_kind(10)`, `seating::LOBBY_FACTIONS(9)`, `game::CONTINUE_ACTION_ID(8)`, `game::CONTINUE_ACTION_KIND(5)`, `invasion::COMMIT_KIND(6)`, `production::{AGENT_OPTION_KIND(5), PRODUCE_KIND(4), PLACE_KIND(4), PAY_KIND(4), BUILDABLE(2), AGENT_OPTION_ID(2)}`,
`movement::NOVA_SEED(5)`, `tactical::{MOVE_KIND(4), MAX_ROUTES, ACTIVATE_KIND}`, `transit::{LOAD_KIND(3), RIFT_DESTROYS_ON}`, `timing::ACTIVE_DETAIL(3)`, `reactions::ACTION_CARD_KIND(3)`, `promissory::SUPPORT_PREFIX(3)`, `relics::SHARD(2)`, `secrets::HAND_LIMIT(2)`,
`transactions::{OPEN_KIND, FREE_FORM_PREFIX}`, `combat::{SUSTAIN_KIND, CASUALTY_KIND}`, `invasion::GROUND_CASUALTY_KIND`, `objectives::VICTORY_TARGET`, `dice::SIDES`, `game::SECONDARY_PREVIEW_COMPLETE`, `strategy::STRATEGY_KIND`, `deck::UNOFFICIAL_ACTION_CARDS`.

types: `game::Game(14)`, `production::Spend(8)`, `fixtures::Hub(8)`, `objectives::{Imagined(6), Cost(5), Position(3)}`, `dice::Roll(5)`, `tactical::Movable(3)`, `observation::ExecutionObservation(3)`, `timing::Relation(2)`, `secrets::Position(2)`, `movement::MapEdit(2)`,
`strategy::SecondaryResolution`, `secondary_preview::SecondaryPreview`, `rng::RngPositions`, `reach_query::ReachOrigin`, `preview::Outcome`, `factions::AssetKind`, `event::Event`, `fingerprint::CanonicalHash`.

queries over a `GameState` (candidates for core `query::*` delegation or small copies): `technology::{prerequisites(5), colour_type(5), faction_of(4), replaced_for_faction(2), name(2), active_aliases(2), unit_form_violations, is_unit_upgrade, gravity_drive_available}`,
`planets::{in_system(4), tech_specialties_now(3), traits_now(2)}`, `objectives::{leader(5), cost_of(4), points_for(3), family_token(3), remaining_position_progress(2), counting_progress(2), bought_progress_at(2), neighbour_planet_counts, home_planets_not_controlled, cost_family_token}`,
`production::{placements(2), trade_good_worth, price_of_under, planet_value_now, planet_value_boosts, planet_value, native_payment_planets, capacity, buildable_for, available}`, `tactical::{ship_routes(2), routes_of, preview_moves, movement_options, activation_options_with}`,
`valuation::unit_value(4)`, `strategy_cards::{card_name(3), INFLUENCE_PER_TOKEN(2)}`, `supply::{remaining(2), plastic}`, `secrets::{progress(2), ship_systems_beside_anomaly, scored_count}`, `seating::seat_faction(2)`, `promissory::{owner_of(2), faction_name(2), deal(2), note_id, held_by, may_receive}`,
`map_feasibility::{unsatisfiable_secret(2), census(2), apply(2), unsatisfiable_public}`, `fleet::standing(2)`, `combat::{cancellable_hits(2), space_cannon_guns, opponent_with_ships}`, `reactions::window_table(2)`, `action_cards::{insider_peek_key(5), name_of, is_insider_peek_key, insider_peek, hand_limit}`,
`fleet_strength::{matchup, hit_chance}`, `thunders_edge::{breakthrough_for, available_actions}`, `legendary::ionian_available`, `invasion::bombardable`, `fracture::systems`, `entropic_scars::abilities_usable`, `faction_abilities::has`, `factions::{hooks_economy, assets, mahact(2)}`, `diplomacy::candidates`,
`exploration::{traits_of, frontier_systems(2), attachment_for, resolution}`, `deck::action_deck_len(2)`, `reach_query::who_can_reach`, `secondary_preview::preview_secondary`.

mutating helpers and setup (need a core entry point or an owner): `promissory::{take, support, receive}`, `exploration::{explore, gain_fragment, mark_legacy_device_ids(2)}`, `objectives::adjust_victory_points`, `movement::{apply_map_edit(2), replay_map_edits}`, `laws::apply_to_galaxy`, `technology::apply_unit_upgrades`,
`action_cards::apply_movement_effects`, `deck::build_starting_decks`, `setup::{start_game, start_game_seeded_with_card_set, strategy_card_setup_for_set}`, `starting_tech::open(2)`, `seeded_speaker(6)`, `core_sync::guard(2)`, `fixtures::{seated_game(6), put_on_planet(5), game(2)}` (test fixtures used by host / rule-bot tests: ~40 sites).

## Appendix B. Method

* Edges: `cargo tree --offline` (`-p` for core / factions / tests; `-i ti4-engine` for the reverse set); `grep -rn "ti4[-_]engine(?![-_a-z])"` over the core crates; `Cargo.toml` of each dependent.
* Module graph: `crate::<module>` references of every top-level module of `crates/ti4-engine/src`, text before the first `#[cfg(test)]` only, reachability closure from the 48 production-named modules (a script in the session scratchpad, not committed; text-based like `tools/engine_profile/module_graph.py`: an upper bound).
* Counts: `find ... -name '*.rs'`, `wc -l`; the test split from `docs/D4_TEST_DISPOSITION.md`; the production-use split from `tools/engine_core/boundary_inventory.py` plus a grep that skips `tests/`, `benches/`, `examples/`, `*_tests.rs` and `#[cfg(test)]` tails.
* Branch overlap: `git diff --name-only $(git merge-base core-integration-15 <tip>) <tip> -- crates/ti4-engine/` for each unmerged branch.
