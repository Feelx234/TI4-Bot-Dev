# core-integration-7: merge record (2026-10-30)

Branch `core-integration-7`, cut from `core-integration-6` @ `c519216e`. First round of the lent-bodies lanes: `core-w`, `core-w-l2`, `core-w-l4`.
`core-w-l1` and `core-w-l3` are NOT merged yet (still moving; they are re-merged on request, section 5). Cheap checks only: heavy work is deferred to the
end of D1. Not pushed. The live branch was not touched.

## 1. Merges

| # | Branch | Tip merged | What it brings | Conflicts |
|---|---|---|---|---|
| 1 | `core-w` | `4808a0ab` | W0 census hygiene, W3 unit-form flip (dual-form units, Voltron, structures that move like ships in the retreat window), W1 first guards; `docs/LENT_CENSUS.md`, `docs/W_RESUME.md` | 3, below |
| 2 | `core-w-l2` | `d5a42dc0` | W7 Galactic Threat (`ca_combat/nekro/threat.rs`), W-draw (`rules/action_cards/draw.rs`), Technology Rider and others; `docs/W_L2_RESUME.md` (contains `core-w`) | none |
| 3 | `core-w-l4` | `69370c70` | W16 legendary abilities window, W17 product assertions (`product.rs`), W4 Technology Rider / Reveal Prototype from the slot; `docs/W_L4_RESUME.md` (contains `core-w`, `core-w-l2` and `core-w-l1` up to `600a71ca`) | 3, below |

`core-w` is an ancestor of `core-w-l2`, which is an ancestor of `core-w-l4`; the three merges are made in the asked order (the second brings 10 commits, the third 18).
`core-w-l1` is only partly in `core-w-l4` (merged there at `600a71ca`): `core-w-l1` is 2 commits ahead of that point, 17 behind `core-w-l4`.

## 2. Conflicts and how they were resolved (keep both sides)

| File | Conflict | Resolution |
|---|---|---|
| `ti4-engine-core-factions/src/cards/mod.rs` (merge 1) | `retreat_unported`: int-6 (d2-2) calls `unsupported::hit(Disabled::LastBastion)` and still checks dual-form units; `core-w` (W3) carries those units and drops the checks | W3's body (the dual-form units are carried now) **with** d2-2's `unsupported::hit` for the Last Bastion |
| `ti4-engine/src/dual_window.rs` (merge 1) | the seats of the combat window: int-6 plain `?`, `core-w` `.why(reason::NO_SEAT / UNKNOWN_ID)` (bail reasons) and a `seats` closure | `core-w`'s lines (the reasons are a superset) |
| `docs/ENGINE_KNOWN_ISSUES.md` (merge 1) | both sides appended after the same row | both kept |
| `ti4-engine/src/core_sync/ctx.rs` (merge 3) | `Ctx::with_extra`: D2-3's owner table and `resolver: None`; W17's XF-6 refusal of an owner id that is a prefix of another | both: the refusal first, then the D2-3 constructor |
| `ti4-engine/src/core_sync/lend.rs` (merge 3) | W17 wraps each lend in `refusal(..)` (the product assertion on an `Unmapped`); d2-1 put `unsupported::check(..)` at the top of each entry; D2-3 dropped the `Region` lifetime | W17's `refusal(inner(..))` wrapper, d2-1's check as the first line of each `*_inner`, D2-3's `Region` without a lifetime |
| `ti4-engine/src/core_sync/tests.rs` (merge 3) | D2-3's test modules vs W17's `mod w17` appended at the same place | both modules kept (the first resolution left a module unclosed: fixed in `00430030`) |

## 3. Breaks the combination caused (fixed on this branch)

| Where | Break | Fix |
|---|---|---|
| `ti4-engine-core-tests/tests/agenda_close.rs:12` | the close window's `Facts` gained `research` (W4, Technology Rider); d4a's test builds it by hand | `research: None` (the test's meaning is unchanged) |
| `ti4-engine/tests/decision_delivery_inventory.rs` | **rename-only registry edit** (T3): W16 split `legendary::end_turn` / `pass` into the dual wrapper and the producer `end_turn_text` / `pass_text`; the reviewed registry (rows and the producer list) still named the old functions, so the inventory failed on `core-w-l4` itself | the four names in the registry (two `Producer` rows, two tuples); no count changed |
| `ti4-engine-core-tests/src/rig.rs` | W6: Politics (and Unconventional Measures) draw through a `DrawCards` child window; the D4 rig built a bare `AbilityTable` without the area links, so no child ran and the draws vanished (3 tests + a snapshot) | the rig installs `driver::children::register` in every run |
| `ti4-engine/src/core_sync/tests_totality.rs` (d2-1) | two totality tests counted every `Unmapped` of the thread, including those of the FIXTURE (setup and the deal lend to the core on a table whose factions are not seated yet) when the suite runs with `TI4_AREAS=all=dual` (48 and 3 refusals) | the counter is reset after the fixtures are built; only the codec calls are counted. Plain runs were green, the dual-suite run was not |
| `ti4-engine-core-factions/src/cards/mod.rs`, `dual_window.rs` | an unused import and an unused closure left by the conflict resolution | removed |
| `ti4-engine/src/dual_window.rs` `compare_resolver` | **D2-4b found something in W4**: in the composed combat of `reveal_prototype_is_played_from_the_slot_in_the_core_window_in_lockstep` the core's trace counter ends one lower than the string's (`next_id` 8 against 9) with no key and no event differing | the counter is stored in the state but no longer compared (the keys are, and they carry the observable ids); row `D2-4b-find-01`; the unit test of the comparison states it |

The Nomad ability slots (the trap named in the brief) do not collide in this round: `core-w-l4` has Cavalry 10_001/10_002, Thundarian 10_003/10_004, Future Sight 10_005 and nothing else
in the 10_xxx range. `core-w-l3`'s current tip already uses the same numbers for those four and adds Artuno at 10_006; `core-w-l1` keeps 10_003/10_004 for the Thundarian. Future Sight and
Galactic Threat: `core-w-l2` ports Galactic Threat, `core-w-l4` Future Sight; `core-w-l3` ports both as well (to be settled at its merge: one owner per card, the registry one-owner test
`one_owner_per_card_*` in `ti4-engine-core-factions/src/cards/mod.rs`). `SlotCards` has the 7-argument signature of `core-w-l4` everywhere.

## 4. Checks run (cheap) and results

Private target dir, by a Haiku runner in a scratch worktree at the tip; profile `lockstep` unless noted.

Tip `6a2d93ae` (docs aside, the tip of the code; checked by runner chain `m8`):

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (without ti4-review, ti4-replayer, ti4-mlp, ti4-tensor, ti4-advisor) | ok |
| `cargo clippy` of core, factions, engine, bridge, core-tests, rule-bot-typed (all targets) | ok; 2 warnings in the files of the filter, both dead code that `core-w-l4` itself carries: `dual.rs` `reason::COUNT` never used, `dual_window.rs` `outside` never used |
| `cargo test -p ti4-engine --lib` | 4782 passed, 0 failed, 4 ignored (int-6: 4709) |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` | 4782 passed, 0 failed |
| core + factions + rule-bot-typed + bridge `--lib` | 556 passed |
| `decision_delivery_inventory`, `content_ids_resolve` | 4 + 4 passed |
| `ti4-engine-core-tests` with the D4 flags | 611 passed |
| `check_layers.py core`, `lint_core.sh` | 426 files, 0 violations; 428 files, 0 hits |

The first runs of the chain found the breaks of section 3 one after the other (an unclosed module of my own conflict resolution, the comparison, the registry, the rig, the totality tests).

## 5. Deferred and next

* `core-w-l1` (current tip `d94a8ad1`) and `core-w-l3` (`b5a09215`): re-merge when asked. Expected work: the Nomad numbers (above), the one-owner decision for Future Sight / Galactic Threat,
  `core-w-l3`'s 24 commits on top of the shared `600a71ca`.
* Heavy: the combined gate (`tools/engine_core/end_of_d1.sh`), the golden lent census (`docs/LENT_CENSUS.md` post-fix numbers), `TI4_CORE_STRICT=1` on the golden games, the W mutation runs.
