# core-integration-5: merge record (2026-10-30)

Branch `core-integration-5`, cut from `core-integration-4` @ `42e69e3c`. Cheap checks only: the user deferred every combined heavy
gate until nearly everything is done. Not pushed. The live branch `nightly-fixes-2026-10-26` and `disable-xf123` were not touched.

## 1. Merge

| Merge | Tip merged | Result | Merge commit |
|---|---|---|---|
| `core-b12` (combat and invasion area; gate green: driver, combat, invasion PACKAGE OK, wide mask 0 differences, 17 soak repro lines) | `25ac6b55` | 5 conflicted files, resolved | `6f895d23` |

## 2. Conflicts and how they were resolved

| File | Conflict | Resolution |
|---|---|---|
| `crates/ti4-engine-core-factions/src/cards/mod.rs` | B6d `register_after_play(t)` (and the `pub fn register_after_play` split of `register`) vs B12's `t.add_card_play(.. FireTeam ..)` in the same function | both: `register` adds the Fire Team play, then calls `register_after_play`; `register_after_play` installs `after_play = announce_destructions` (the tail of the old function) |
| `crates/ti4-engine-core/src/rules/production/pay.rs` | `use` line | `use ...resolver::TimingError;` and `use ...stage::{Child, Illegal, Ran, Sx, Window};` |
| `crates/ti4-engine/src/dual_window.rs` | fields of the window report | all four kept: `plain`, `mirrored`, `order` (P1) and `adjacent` (B12) |
| `crates/ti4-engine/src/objectives_write_tests.rs` | B12 extended the old inline question block of `the_documented_bails_stay_with_the_string_side` (price at a Bastion / Crimson table is now followed); int-4 had already moved that block into `price::the_scoring_step_bails_stay_with_the_string_side` on `status_probe` (A4b-2) | int-4's version kept (the old block was deleted there on purpose: `score_one` has a new signature); B12's change applied to it: `probe_score` of `amass_wealth` / `dhw` at a Bastion or Crimson table is now `is_some()`; the two lists of the lockstep price tests already carried `bastion` and `crimson` (automerge). Comments of `score_one` and its bail label no longer name Bastion / Crimson (`price_is_string` is the ocean-card test alone, B12 5c) |
| `crates/ti4-engine/src/reactions_core.rs` | the unit test of "a card the core does not play" | Fire Team's line dropped (B12 registers it), Scramble Frequency's kept with a comment |

## 3. Owner per card (Fire Team, Scramble Frequency, Courageous To The End)

Both lines touched these three cards: B6d gave them a play window of the action-card area (`Fx::Reroll`, `Fx::Scramble`,
`Fx::Courageous`, run by `Announce::of` -> `Effect::Play` with the facts of the adapter's `play_window` / `announce_window`); B12
registered Fire Team as a `CardPlay` (`ti4-engine-core-factions/src/cards`, `fire_team`) played inside the ground roll's tail
(`Effect::Registered`, run by `play_inline`). The two never run for the same play: the registration is only reached by
`play_inline` (a card declared inside a core-followed window, `rt.has_registered_play`), the play window only by the string's
top-level `play_effect` / announcement (`Announce::of(.., registered = false)`), and `Announce::of` prefers `Effect::Play` only where
the play window's `Facts` exist, a registration elsewhere (test `a_registered_effect_wins_over_a_window_that_is_not_played_nested`).

| Card | Owner | Why |
|---|---|---|
| Fire Team | **B12's registered `CardPlay`** for every play inside a window the core follows (the ground roll's tail, the only place the card can be played in a real game, where the string runs `play_inline`); **B6d's `Fx::Reroll`** only for the string's top-level `play_effect` (invasion area on the string, `Area::ActionCards` on dual). One owner per window | B12's cannot be replaced: the reaction slot of the ground roll runs it, with the staged texts of `AdapterFacts::reroll` and the stream positions compared by `roll_tail`; B6d's cannot be replaced either: with the invasion on the string the slot's play is a top-level announcement and has no registration to run (the B12 test `fire_team_played_from_the_slot_of_a_real_ground_combat_agrees_with_the_core_window` is exactly this path and passes on both). Both are compared with the SAME string effect, so they agree transitively. The B6d test list and the B12 test list are both kept: every assertion of one is on a different path from the other's |
| Scramble Frequency | **B6d's `Fx::Scramble`** alone | B12 did not port it (documented bail: the barrage / space cannon / bombardment windows are not lent the dice stream and the staged texts); not registered, not `Fx::nested` |
| Courageous To The End | **B6d's `Fx::Courageous`** (play window, `Spawn::Destroy` child window) alone | B12 only counts it as a bail inside the invasion's casualty question (`bail_courageous_to_the_end_is_counted` stays true: it is not `Fx::nested`) |

New test `cards::tests::one_owner_per_card_fire_team_is_registered_and_scramble_and_courageous_are_not` (factions crate) pins the split:
with the registered table Fire Team is `plays_in` and announced as a registered play, Scramble Frequency, Courageous To The End and
Experimental Battlestation are neither registered nor played inside windows (a registered `Announce::of` for them is `None`).

## 4. Other checks of the worklist

* `ENGINE_KNOWN_ISSUES.md`: the file of int-4 was kept (it is the newest; `core-b12` itself changes nothing in it, the B12-bail-01..07,
  B12-copy-01/02, B12-design-01 rows were already in the int-4 line through `core-integration`, and `R-bail-01` / `R-bail-05` carry the
  B12 wording once). Compared with the `core-integration` worktree copy the only differences are int-4's newer B6 rows
  (`B6-bail-13` amended, `B6-bail-15`, `B6-copy-07..09`), so no row was added twice. One edit: `R-bail-02` named Fire Team as still
  string inside the roll windows and "core-b12 registers it"; it now says Fire Team is a registered play there (B12) and only
  Scramble Frequency stays string (the barrage / space cannon / bombardment windows are not lent the dice stream).
  `B12-bail-02` (Fire Team 6 / Courageous 2 / Scramble 1 counted bails) and `B6-bail-15` stay true: they count nested plays.
* Delivery inventory (`crates/ti4-engine/tests/decision_delivery_inventory.rs`, B12 changed 28 lines of it): no recount needed;
  `cargo test --profile lockstep -p ti4-engine --test decision_delivery_inventory`: 4 passed.
* `Spec` registration order (Ssruu copies, `Spec::leads`): B12's order tests (`ssruu_copies_*`, the registry order tests of
  `reactions_core`) pass in the lockstep run; nothing in D1b / P1 / B6d registers a spec, so the order is B12's unchanged.
* Two parallel `RerollFacts` types exist (B6d `rules::action_cards::RerollFacts`, `Copy`, fixed 12 groups, for the play window;
  B12 `kernel::ability::RerollFacts`, a `Vec`, for the reactions table). They are different inputs of different paths and do not
  clash; unifying them is a cleanup for later, not a combination bug.

## 5. Checks run (cheap) and results

All with `CARGO_TARGET_DIR=/root/deploytmp/core-int5`.

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (without ti4-review, ti4-replayer, ti4-mlp, ti4-tensor, ti4-advisor) | ok, only warnings that exist on the parents |
| `cargo test -p ti4-engine-core -p ti4-engine-core-factions` | ok: core 307 + 26 + 3, factions 109 (108 + the new owner test), 0 failed |
| `tools/engine_core/check_layers.py core` | 401 files, 0 violations |
| `tools/engine_core/lint_core.sh` | 403 files, 0 hits |
| `cargo test --profile lockstep -p ti4-engine --lib --no-fail-fast` (Haiku runner, scratch worktree at the merge commit `6f895d23`) | ok: 4589 passed, 0 failed, 4 ignored (int-4 had 4552; +37 from B12) |
| `cargo test --profile lockstep -p ti4-engine --test decision_delivery_inventory` | ok: 4 passed |

## 6. Tests failed by the combination, and fixed

None in the lockstep run. The one test that disagreed with the merged code was written against the old behaviour: int-4's
`the_scoring_step_bails_stay_with_the_string_side` expected the string to keep a priced objective at a Bastion / Crimson table; B12
ported that (5c), so it now asserts the core follows it (resolution above). No regression test failing on pre-fix code applies: there
was no combination bug, and the new `one_owner_per_card_...` test is a pin of the owner split.

## 7. NOT run (the deferred gates)

`tools/engine_core/verify_package.sh`, the sharded golden dual and the wide dual, `core_equivalence`, `board_equivalence`, all benches,
the soak runner (`soak_corpus.py` / `dual_run --specs`: the int-4 soak corpus plays Technological Singularity, Nekro and combat lines
B12 ports, and B12's soak signatures were still being fixed on `core-b12`: run it on this branch at the combined gate), `mutants.sh`,
`cargo clippy`, the web build, TypeScript tests, and the torch-sys crates.
