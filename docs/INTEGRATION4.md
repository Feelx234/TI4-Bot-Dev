# core-integration-4: merge record (2026-10-30)

Branch `core-integration-4`, cut from `core-integration-3` @ `1e55ab28`. Cheap checks only: the user deferred every combined heavy
gate until nearly everything is done. Not pushed. The live branch `nightly-fixes-2026-10-26` and `disable-xf123` were not touched.

## 1. Merge order and result

Dependency check (`git merge-base --is-ancestor`): `core-compute` (`cad8f451`) is an ancestor of `core-d1b`, so it needs no merge of
its own; `core-soak` (`6389525c`) contains `core-compute`; `core-d1b` (`604aa2ed`) contains core-p1, core-d1a, core-a2b,
core-a4-objectives, core-b8c, core-b9-agenda, core-b4-trade, core-dual-shards. All four contain `core-integration-3`.
`core-d1b` and `core-b6-action-cards` meet at `96f7df66` (core-b4-trade); neither contains the other.

| # | Merge | Tip merged | Result | Merge commit |
|---|---|---|---|---|
| 1 | `core-d1b` | `604aa2ed` | clean (no conflict) | `7e69c80e` |
| 2 | `core-b6-action-cards` | `82266226` | 5 conflicted files, resolved | `004552e0` |
| 3 | `core-soak` | `6389525c` | clean (brings `core-compute` through `core-d1b`: already there) | `e1593c0a` |

`core-b12` (combat/invasion) is NOT merged: it is int-5 (section 5).

## 2. Conflicts and how they were resolved (merge 2, B6 into D1b)

Everything else (game.rs line shifts of D1b/P1/A2b, `state/pending.rs`, `dual.rs`, `verify_package.sh`, `Cargo.toml` profiles, the
delivery-inventory registry) merged without a textual conflict.

| File | Conflict | Resolution |
|---|---|---|
| `crates/ti4-engine-core/src/driver/mod.rs` | module list: D1b added `aftermath`, `menu`, `session`, `turn_flow`; B6 added `children` | union, alphabetical |
| `crates/ti4-engine-core/src/kernel/mod.rs` | module list: D1b added `fx_agenda`, `fx_move`, `fx_relic`; B6 added `children`, `fx_explore`, `fx_ground` (git had merged `children` and the shared `fx_*` lines) | union of all ten `fx_*` / `children` modules, alphabetical; every file exists |
| `crates/ti4-engine/src/dual_window.rs` | B6 added `pub(crate) struct Lent` (used by `action_cards_view.rs`) right after `DiceSeeds`; the D1b side had nothing there (the lines around it moved) | kept `Lent` (B6 side) |
| `crates/ti4-engine/src/render/mod.rs` | doc comment of the `shapes` field: D1b "strategy waivers, trade deals", B6 "waivers" | one comment naming both users; the field is one `RefCell<Vec<ChoiceOption>>` and compiles for both |
| `docs/ENGINE_KNOWN_ISSUES.md` | D1b's 168 added lines vs B6's 6 | integrated copy = `core-integration` working copy (committed `66f1b6a7` plus its 3 uncommitted A2b-gate-01..03 lines; 552 lines) with the B6 delta applied by a three-way merge (base int-3 `1e55ab28`): the B6c block plus B6-bail-15 kept side by side, `R-bail-02` took B6's newer text (Fire Team / Scramble Frequency have a play window since b6d-rerolls), `B6-copy-07..09` and the amended `B6-bail-13` are carried. `R-bail-01` / `R-bail-03` kept the `core-integration` text (B6 left them as in the base). 556 lines in all; every line B6 or D1b added is present |

The two uncommitted hunks of `ENGINE_CORE_STATE.md` in `.claude/worktrees/core-integration` (a D4 row, the wave-D line, section D4 brief)
were not asked for and are NOT in this branch.

## 3. Checks run (cheap) and results

All with `CARGO_TARGET_DIR=/root/deploytmp/core-int4`.

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (without ti4-review, ti4-replayer, ti4-mlp, ti4-tensor, ti4-advisor) | ok; only warnings that already exist on `core-d1b` and `core-b6-action-cards` (unused imports, `Game::push_agenda_record` never used) |
| `cargo test -p ti4-engine-core -p ti4-engine-core-factions` | ok: 307 + 26 + 3 + 87 passed, 0 failed |
| `tools/engine_core/check_layers.py core` | 397 files, 0 violations |
| `tools/engine_core/lint_core.sh` | 399 files, 0 hits |
| `cargo test --profile lockstep -p ti4-engine --lib --no-fail-fast` (one run, scratch worktree at `e1593c0a`) | see section 4 |

## 4. Tests failed by the combination, and fixed

LOCKSTEP_RESULT_PLACEHOLDER

## 5. NOT run (the deferred gates)

`tools/engine_core/verify_package.sh` (all of it: wasm tests, immutable surfaces, clippy, dual suite, benches), the sharded golden dual
(`dual_run` on `profile-20.json`), the wide dual, `core_equivalence` (templates, golden positions), `board_equivalence`, all benches
(`bench`, `bench-v1..v5`), the CPU A/B, the wasm size report, `state_size`, the soak runner (`soak_corpus.py` / `dual_run --specs`),
`mutants.sh`, `cargo clippy`, the web build and TypeScript tests. Also not run: the torch-sys crates (`ti4-review`, `ti4-replayer`, `ti4-mlp`,
`ti4-tensor`, `ti4-advisor`) were excluded from check.

## 6. Open overlaps for int-5 (merge of `core-b12` @ `851c69cb`)

`git merge-tree --write-tree` of this branch with `core-b12` (nothing written to a worktree) reports five conflicted files:

* `crates/ti4-engine-core-factions/src/cards/mod.rs`: B6d's `register_after_play` (announces what a played card staged, e.g. Courageous To The End's
  `pending_destructions`) against B12's `t.add_card_play(CardPlay { covers: FireTeam, effect: fire_team })` in the same registration function.
* `crates/ti4-engine-core/src/rules/production/pay.rs`: the `use` line (B6d `Child`; B12 `TimingError`): keep both.
* `crates/ti4-engine/src/dual_window.rs`: the fields of the window report: P1's `plain`, `mirrored`, `order` against B12's `adjacent`: keep all.
* `crates/ti4-engine/src/objectives_write_tests.rs`: a test body B12 extended (price of the scoring question with Bastion / Crimson / Deepwrought oceans) where this line
  has none: check which version of the test fits the scoring window as it is now.
* `crates/ti4-engine/src/reactions_core.rs`: the unit test of "a card the core does not play": B6d asserts Fire Team AND Scramble Frequency are still unported when
  nested; B12 asserts only Scramble Frequency (it ports Fire Team): the test must drop the `fire_team` line and keep the `scramble` one, if B12's Fire Team
  registration stays.

Semantic overlaps beyond text (decide by hand at int-5, the text merge will not catch them):

* **Fire Team, Scramble Frequency, Courageous To The End**: B6d has play windows (`Fx::Reroll` / `Fx::Scramble`, `Fx` of the play window
  with child windows from `kernel/children.rs`); B12 registers Fire Team for the ground roll's tail and handles Courageous in the
  invasion's casualty question. Two owners for the same card: keep exactly one registration per card, and recheck `R-bail-02`,
  `B6-bail-15` and the B12 lines of `ENGINE_KNOWN_ISSUES.md`.
* **Soak signatures of B12**: another agent is still fixing them on `core-b12` (not final at the time of this merge).
* `decision_delivery_inventory` registry entries: renames on both sides may need a recount.
* The soak corpus (this branch) plays Technological Singularity, Nekro and combat lines that B12 ports: run the soak (`dual_run --specs`)
  on the merged int-5 as part of its gate.
* `ENGINE_KNOWN_ISSUES.md`: `core-b12` adds its own lines (B12-bail-*); it merged cleanly in the dry run, but `R-bail-01` carries the B12 wording
  already (from `core-integration`): do not add it twice.
