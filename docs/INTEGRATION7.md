# core-integration-7: merge record (2026-10-30)

Branch `core-integration-7`, cut from `core-integration-6` @ `c519216e`. Round 1 (sections 1 to 5): the lent-bodies lanes `core-w`, `core-w-l2`, `core-w-l4`.
Round 2 (section 6): `core-w-l1`, `core-w-l3`, `core-d4a2`, `core-d4a3`. Round 3 (section 7): `core-w-l5`, and the barrage-scoring bug of the combat machine. `core-w-l6` is not merged. Cheap checks only: heavy work is deferred to the
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

* (round 1 note, done in round 2) `core-w-l1` and `core-w-l3`: merged, section 6.
* Heavy: the combined gate (`tools/engine_core/end_of_d1.sh`), the golden lent census (`docs/LENT_CENSUS.md` post-fix numbers), `TI4_CORE_STRICT=1` on the golden games, the W mutation runs.

## 6. Round 2: `core-w-l1`, `core-w-l3`, `core-d4a2`, `core-d4a3`

### 6.1 Merges

| # | Branch | Tip merged | Conflicts |
|---|---|---|---|
| 4 | `core-w-l1` | `8b02b7e6` | none |
| 5 | `core-w-l3` | `b5f85d66` | 4, below |
| 6 | `core-d4a2` | `f0bab516` | `ENGINE_KNOWN_ISSUES.md` (both appended) |
| 7 | `core-d4a3` (on d4a2) | `8bebd8a9` | none |

`core-w-l5` (from l3) is not merged, as asked.

### 6.2 Conflicts of `core-w-l3` (keep both sides) and the one-owner decision

| File | Resolution |
|---|---|
| `ti4-engine-core-factions/src/ce_movement/nomad/timing.rs` | Future Sight was ported by W7 (l4) and by W9 (l3). **One owner: the W9 module (`nomad/timing.rs`)**; W7's body is dropped. That module also owns Artuno, so taking l3's version of the file keeps the slots consistent. |
| `ti4-engine/src/render/mod.rs` | D2-3's code-table methods (`codes`, `with_codes`, `codes_snapshot`) and W15's free-form deal methods (`register_free`, `free_index`, `free_id`, `free_net`) both kept; the families `technology` and `legendary` are no longer uninhabited (`uninhabited!(tokens, promissory)` remains) |
| `ti4-engine/src/tactical_dual_tests.rs` | W17's and W9's tests both kept |
| `docs/ENGINE_KNOWN_ISSUES.md` | both appended; new row `INT7-owner-01` records the owner decision |

Galactic Threat: ported by l2, l4 (through l2) and l3 with identical diffs; one owner, `ca_combat/nekro/threat.rs`. No duplicate. The registry test `one_owner_per_card_*`
(`ti4-engine-core-factions/src/cards/mod.rs`) passes.

Nomad slots after the merge, all unique: Cavalry 10_001 / 10_002, Thundarian 10_003 / 10_004, Future Sight 10_005, Artuno 10_006 (also Winnu 15_001 / 15_002, Magen 16_001).

### 6.3 Dead code (the two clippy warnings of round 1)

`reason::COUNT` (`dual.rs`) and `outside` (`dual_window.rs`) removed. Removing `outside` made `use crate::dual_window::{outside};` in `strategy_cards_core.rs` unused,
and the first removal broke the build there (E0432): the import is removed too. `outside_why` and `outside_battle` stay.

### 6.4 Breaks the combination caused (fixed)

| Where | Break | Fix |
|---|---|---|
| `ti4-engine/tests/decision_delivery_inventory.rs` | **rename-only registry edit**: W renamed `production_biomes` to `production_biomes_string`, and in `technology.rs` `end_turn`, `start_turn`, `production_used`, `self_assembly_after_production` to `*_string` / `self_assembly_string` | the names in the Producer rows and tuples (about 12 lines); no count changed |
| `ti4-engine-core-tests/tests/production_mod2.rs` | the production asks gained `ability` (W5) | `ability: false` in `ask()` and the two `PlaceAsk` literals |
| `ti4-engine-core-tests/tests/*` (test-crate struct literals) | fields added by the W lanes after d4a2/d4a3 wrote their tests: combat_roll `round: 1`; trade_window, query_trade_deals `free: [None, None]`; combat_machine `Env` (`det`, `wormhole`, `ambush`, `cause_core`, `cause_devotion`, `raid`, `ac`, `ac_subtype`, `ac_cause`); invasion_windows (`can_pay`, `dunlain`); production_window(2) `ability: false`; combat_extra, combat_barrage `raid: None` | defaults that keep each test's meaning (false / None / fresh codes) |
| `agenda_table_effects2.rs` | W6: the For outcome of Unconventional Measures draws through a `DrawCards` child window, so it has no table effect | **assertion edit (necessary combination fix):** `Table::of(UnconventionalMeasures, For)` is now asserted `None`; the `Table::VotersDraw.draws_from_deck() &&` conjunct is dropped |
| `combat_machine.rs` | W2: Devotion is no gate any more (the fight runs to its end) | **assertion edit:** the Devotion test expects `Some(Stop::Over)` |
| `combat_machine.rs` `a_combat_whose_other_side_dies_to_the_barrage_alone...` | the lone-fighter kill by the barrage now runs straight to `Over` without a scoring pause in this position (all seeds: `Over`, defender 0 ships, outcome `(Some(Seat(0)), 0)`) | **(superseded by 7.2: the loosening hid a machine bug; the test is strict again and the machine is fixed)** |
| test crate clippy | 4 warnings: `contains()` x2 in `action_cards_simple.rs`, `type_complexity` x2 in `combat_machine.rs`, `invasion_rules.rs` | `contains`, type aliases |

Assertion edits to tests, all listed: the three rows above marked so. No other test assertion was edited.

### 6.5 Checks (cheap), runner chain `r7` at `cd6ab0b8` and `s7` at the tip

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (same exclusions as round 1) | ok |
| `cargo clippy` of core, factions, engine, bridge, core-tests, rule-bot-typed | ok; 0 warnings in the core, factions, bridge, core-tests, rule-bot-typed crates or in the files of round 2; the engine lib reports 22 older warnings in other files |
| `cargo test -p ti4-engine --lib` | 4823 passed, 0 failed |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` | 4823 passed, 0 failed |
| core + factions + rule-bot-typed + bridge `--lib` | 562 passed |
| `decision_delivery_inventory`, `content_ids_resolve` | 4 + 4 passed |
| `ti4-engine-core-tests` with the D4 flags | 1105 passed, 0 failed (runner chain `s7` at `d6600cf3`) |
| `check_layers.py core`, `lint_core.sh` | 430 files, 0 violations; 432 files, 0 hits |

Not run: any heavy work (the combined gate `tools/engine_core/end_of_d1.sh`, golden lent census, `TI4_CORE_STRICT=1`, mutation runs). Not pushed, live branch untouched.

## 7. Round 3: `core-w-l5`, and the string check of the barrage-alone test

### 7.1 Merge

`core-w-l5` @ `7085d776` (11 commits on `core-w-l3`: Military Support, Gurno Aggero, Riftwalker Meian, Lazax Gate Folding, Nekro repair, Terraform, Malleon, Stall Tactics, Vortex, the Nekro draw; the shared pieces `choice::promissory`, Vortex in `choice::technology`, additions in `choice::leaders`, the Promissory area in `render/mod.rs`). `core-w-l1` had not moved (still `8b02b7e6`, already merged); `core-w-l6` is not merged.

| File | Conflict | Resolution |
|---|---|---|
| `ti4-engine/src/render/mod.rs` | `uninhabited!` list and the registry: int-7 had `legendary` ported (W16) and `promissory` unported; l5 ported `promissory` and still had `legendary` unported | both families are ported now: only `tokens` stays uninhabited, `Legendary => legendary::Fam`, `Promissory => promissory::Fam` |
| `docs/ENGINE_KNOWN_ISSUES.md` | both sides appended after the same row | both kept |

Break of the combination: the delivery registry (`decision_delivery_inventory.rs`) still named `cabal.rs::vortex_perform` (W renamed it `vortex_string` in l5): rename only (the Producer row and the tuple), no count changed.

### 7.2 Is the machine wrong in `a_combat_whose_other_side_dies_to_the_barrage_alone_is_over_with_no_round_fought`? Yes: a bug in the core machine, fixed

Checked against the string engine, not a guessed default. New string test (`combat.rs`, `a_barrage_that_takes_the_last_ship_of_a_side_still_pauses_for_scoring_and_ends_the_fight_in_round_zero`): 3 destroyers against one fighter, all barrage dice hit. Result: the string records `BarrageTookTheLastFighters`, consumes the scoring pause (`asks_before_pause == Some(0)`), and ends with the attacker winning in 0 rounds. So the string DOES pause for scoring there.

The core machine recorded the same feat (probe: `feats [(BarrageTookTheLastFighters, 1)]`, 2 barrages, all 40 seeds) but stopped at `Over` with no pause. Cause: after the barrage the loop continued in the `Step::Opening` arm, whose first line is `if self.over(sx) { conclude(round - 1) ... Over }`; with the defender wiped, that fired before `open_round` could present the resumed round (the feat block). Fix (`rules/combat/machine.rs`): `if !self.barrage_done && self.over(sx)`, so the end of the fight is read after the pause. In a game this would have been a lockstep divergence (core ends the combat without the scoring pause the string offers); no golden game reached it.

Regression tests: the machine test is strict again (per seed: the feat recorded means `Stop::Scoring`, then `Over` after the pause; no feat means `Over`) and fails on the old code (it ended at `Over` with the feat recorded); and the string-side test above. The loosened assertion of round 2 (section 6.4, third row) is thereby reverted: it had hidden the bug. The earlier "W2 changed the machine" explanation in that row was wrong.

### 7.3 Checks (cheap), runner chain `u7` at `8922828e` and `v7` at the tip

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` | ok |
| `cargo clippy` of core, factions, engine, bridge, core-tests, rule-bot-typed | ok; 3 warnings in the files of the filter, all in test code, removed afterwards WITHOUT a rebuild (trivial edits: the unused `use crate::dual::{...}` line in `combat.rs` `roll_step_fights`, and `vec![..]` to an array in `leader_windows.rs` and `promissory_windows.rs` tests); none in the others |
| `cargo test -p ti4-engine --lib` | 4835 passed, 0 failed |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` | 4835 passed, 0 failed |
| core + factions + rule-bot-typed + bridge `--lib` | 586 passed |
| `decision_delivery_inventory`, `content_ids_resolve` | 4 + 4 passed (after the registry rename; the first run at `8922828e` failed on it) |
| `ti4-engine-core-tests` with the D4 flags | 1105 passed, 0 failed |
| `check_layers.py core`, `lint_core.sh` | 432 files, 0 violations; 434 files, 0 hits |

Not run: heavy work (combined gate, golden lent census, `TI4_CORE_STRICT=1`, mutation runs). Not pushed.
