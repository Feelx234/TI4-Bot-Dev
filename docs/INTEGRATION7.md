# core-integration-7: merge record (2026-10-30)

Branch `core-integration-7`, cut from `core-integration-6` @ `c519216e`. Round 1 (sections 1 to 5): the lent-bodies lanes `core-w`, `core-w-l2`, `core-w-l4`.
Round 2 (section 6): `core-w-l1`, `core-w-l3`, `core-d4a2`, `core-d4a3`. Round 3 (section 7): `core-w-l5`, and the barrage-scoring bug of the combat machine. `core-w-l6` is merged in round 4 (section 8), `core-w-l7` in round 5 (section 9), `core-w-l8` in round 6 (section 10: the dual finding, the remaining gates, the end-of-D1 dry run). Cheap checks only: heavy work is deferred to the
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

## 8. Round 4: `core-w-l6`

### 8.1 Merge

`core-w-l6` @ `7aff9f83` (8 commits on `core-w-l5`): Nekro C4R PDS, Yssaril Mageon Implants, the Arborec agent, Naaz Fabrication and Black Market Forgery (`relic_windows::Fabrication`), Ghost Wormhole Generator, Mahact Vaults of the Heir, Sol Orbital Drop; `structure_allowed` moved to `query::laws` (re-exported from `rules::production`); `fx::take_action_card_staged`. **No conflicts** (auto-merged: `leader_windows.rs`, `promissory_windows.rs`, `ENGINE_KNOWN_ISSUES.md`).

Break of the combination: the delivery registry named `arborec.rs::agent_use` and `faction_abilities.rs::perform_component` (the Sol Orbital Drop entry); l6 renamed them `agent_use_string` and `orbital_drop_string`. Rename only (two Producer rows, two tuples); no count changed. The Arborec `perform_component` row is untouched.

### 8.2 Checks (cheap), runner chain `w7` at `2bf5b06e` (the merge) and a rerun of the inventory at the tip

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` | ok |
| `cargo clippy` of core, factions, engine, bridge, core-tests, rule-bot-typed | ok; **no warning** in the core crates or in `dual`, `dual_window`, `core_sync`, `render`, `combat`: the three round-3 clean-ups build |
| `cargo test -p ti4-engine --lib` | 4842 passed, 0 failed |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` | 4842 passed, 0 failed |
| core + factions + rule-bot-typed + bridge `--lib` | 605 passed |
| `decision_delivery_inventory`, `content_ids_resolve` | 4 + 4 passed (after the rename; the first run at `2bf5b06e` failed on it) |
| `ti4-engine-core-tests` with the D4 flags | 1105 passed, 0 failed |
| `check_layers.py core`, `lint_core.sh` | 432 files, 0 violations; 434 files, 0 hits |

Not run: heavy work. Not pushed.

## 9. Round 5: `core-w-l7` (fast-forward)

`core-w-l7` @ `fedbbd57` was cut from `core-integration-7` @ `3b97d529` (8 commits, 36 files): Muaat component actions as core bodies (`rules::muaat_component`: Star Forge, The Nucleus, The Inferno, Fires of the Gashlai), the Arborec hero and the Muaat agent with the production window as a child (`rules::leader_production`, `Spawn::Produce`), Chaos Mapping's single unit and Freelancers as `rules::production::one::ProduceOne` (`Spawn::ProduceOne`; Chaos Mapping is the third stage of the start-of-turn window), more tests, `docs/W_L7_RESUME.md`. Merged by fast-forward: no conflicts, no merge commit, and no fix was needed (the delivery registry already matched).

Checks (cheap), runner chain `y7` at `fedbbd57`:

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` | ok |
| `cargo clippy` of core, factions, engine, bridge, core-tests, rule-bot-typed | ok; no warning in the core crates or in `dual`, `dual_window`, `core_sync`, `render`, `combat` |
| `cargo test -p ti4-engine --lib` | 4851 passed, 0 failed |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (full lib) | 4851 passed, 0 failed |
| core + factions + rule-bot-typed + bridge `--lib` | 623 passed |
| `decision_delivery_inventory`, `content_ids_resolve` | 4 + 4 passed |
| `ti4-engine-core-tests` with the D4 flags | 1133 passed, 0 failed |
| `check_layers.py core`, `lint_core.sh` | 435 files, 0 violations; 437 files, 0 hits |

Not run: heavy work. Not pushed. `core-w-l8` (heroes) is not merged.

## 10. Round 6: `core-w-l8` (the heroes)

### 10.1 Merge

`core-w-l8` @ `e8d934fb` (10 commits on `core-w-l7`): the six hero windows (`rules::hero_windows`: Nekro Flayesh, Yssaril Kyver, Cabal It Feeds on Carrion, Naaz Hesh and Prit, Winnu Mathis Mathinus, Argent Flock Migration) with their adapters (`hero_dual.rs`), `kernel::fx_perform` (the body of `rules::action_cards::perform` lifted to the kernel), `Spawn::Seeing` / `SeeingLimits` / `Report::Removed`, `capture_from_board` moved to `kernel::fx` (re-exported from `rules::production::supply`), `docs/W_L8_RESUME.md`. **No conflicts** (the brief's predicted ones in `leaders.rs`, `rules/action_cards/perform.rs`, `kernel/children.rs` merged automatically). The two registry rows `use_leader_strategy_primary` to `hero_primary_string` are l8's own and already match.

### 10.2 What the full dual found (lane 8 had not run it)

The first full `TI4_AREAS=all=dual` engine lib run on the result failed **4 Cabal hero tests** (`factions::cabal::tests::{a_ship_that_keeps_enough_capacity_keeps_its_cargo, a_captured_carrier_takes_the_fighters_and_ground_forces_that_no_longer_fit, the_hero_captures_on_a_one_to_three_and_not_on_a_four_or_more_then_is_purged}`, `faction_effects_tests::d5::cabalhero_2_...`): `DUAL DIFFERENCE factions::cabal::hero` (questions string 1 or 4, core 0; a ship captured on one side only). Cause: these tests roll **test faces** (`Dice::from_faces`); the core window draws its dice from the state's stream, so the two sides rolled different dice. Every other dual call site that rolls checks `product::reproducible(..)` (XF-7) and bails; `carrion_dual` did not. Fix (`factions/cabal.rs`): compute `reproducible` before the window; without it the core half returns no window with the counted bail `irreproducible dice (test faces or a stream restore)`. Regression test `a_hero_rolled_with_test_faces_has_no_core_window_and_the_dual_run_stays_silent` (mask `LeadersAbilities=dual`, test faces: zero differences, window counts `(0, 1)`); on the old code the core window ran and the same position differs. The lane-8 lockstep tests use real seeds and were never affected. Also cleared: a `vec!` warning in `tests/hero_windows.rs` and the `type_complexity` warning in `tests/leader_production.rs:80` (lane 7's test code).

### 10.2b Checks (cheap), runner chain `a8` at `5127eaca` (the first chain `z7` at the merge found the Cabal tests and the `vec!` warning)

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` | ok |
| `cargo clippy` of core, factions, engine, bridge, core-tests, rule-bot-typed (`--message-format short`) | ok; no warning in the core crates, the core-tests, the typed bot or in `dual`, `core_sync`, `render`, `combat` |
| `cargo test -p ti4-engine --lib` | 4860 passed, 0 failed |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (full lib) | 4860 passed, 0 failed (4855 + 4 failed before the fix) |
| core + factions + rule-bot-typed + bridge `--lib` | 623 passed |
| `decision_delivery_inventory`, `content_ids_resolve` | 4 + 4 passed |
| `ti4-engine-core-tests` with the D4 flags | 1172 passed, 0 failed |
| `check_layers.py core`, `lint_core.sh` | 437 files, 0 violations; 439 files, 0 hits |

Not run: heavy work (`end_of_d1.sh` itself, only its `--dry-run`).

### 10.3 Remaining string-lent gates (what the string still does inside a dual or core run)

Counted, each under its own id or label, each with a test. Sources: `docs/W_L1_RESUME.md` ... `docs/W_L8_RESUME.md`, `ENGINE_KNOWN_ISSUES.md` (`W2-bail-02`, `W12-*`, `W14-*`).

* **Action leaders** (`leaders::use_leader [gate: <id>: ...]`, `leaders::string_only_leader`): `empyreanhero` (no window that explores a frontier token; the exploration runs under a whole-state rollback), `yinhero` (each chosen planet goes through the eventful ground combat in the timing resolver, then `finish_control_gain`), `mahacthero` (the fleet's move and the `Enforce` child exist, but the fight is `combat::resolve_resolving`; the core `Machine` is not a window that runs a whole combat as a child). Dropped by decision, not ported: Last Bastion, Crimson Rebellion, Keleres, Deepwrought, Firmament, Obsidian, Ral Nel.
* **Warfare and Imperial inside the Naaz and Winnu heroes** (`fx_perform::begin` returns `Began::Unsupported`): the Warfare secondary (home production: `production::resolve_timed` adds the Hacan hero offer, the Keleres agent window and the use counters around the window), the Imperial primary (scoreable objectives, award tail, winner), the Thunder's Edge Construction and Warfare primaries; also a Politics draw a hook changes, a research a faction waiver opens, a Leadership payment the production kernel does not follow, a table where a replenishment causes events (Cabal, Firmament siphon, Trade Agreement), a Mahact-held command token. Tests `the_warfare_secondary_is_not_carried_and_makes_the_window_unsupported`, `the_imperial_primary_is_not_carried_and_no_cards_means_no_question`.
* **Thunder's Edge pieces**: Saar Deorbit Barrage and Arborec Psychospore (breakthrough component actions: undecided, nothing ported, documented bails as ordered), the Heart of Ixth in the combat opening, the staged Yin breakthrough event in `objectives::award_tail`, the Thunder's Edge Construction / Warfare primaries above.
* **Combat openings pinned by lane 1** (`W2-bail-02`, all rare in the golden census, test `the_start_hits_and_the_barrage_tail_stay_string_with_their_reasons`): the Jol-Nar commander's reroll question in the barrage, Waylay played in the barrage, the Heart of Ixth, Assault Cannon against a neutral side, Munitions Reserves paid at the opening below rules version 8, diplomacy, hits cancelled by a card in the window, destructions staged for an announcement (the Courageous and Reflective Shielding child flows), a roll that ends the fight inside the roll step, a Last Bastion retreat, X-89 Bacterial Weapon (`roll_tail_unported`), the Hacan flagship / Munitions Reserves with goods only a Keleres agent could turn into spendable ones (`roll_why`).
* **Counted bails inside the new hero windows**: Nekro, a galvanized ground force on the devoured planet; Yssaril, blank laws or a faction-changed hand limit; Cabal, test faces (above); Naaz and Winnu as the Warfare/Imperial row.
* **Component actions still gated** (`faction_abilities::perform_component [gate: ...]`): the Thunder's Edge ones above; the Inferno and Fires of the Gashlai placement under fleet supply and capacity where the string asks which units go (a single unit standing over the supply or the capacity bails); a Nekro or Valefar copy of a card; a Naaz mech staged in a use; Hacan's breakthrough, the Prophecy of Ixth, the Last Bastion / Crimson Rebellion, Keleres / Deepwrought seats.

### 10.4 `tools/engine_core/end_of_d1.sh --dry-run`

Run on this tree (prints, takes no lock, touches nothing): 20 steps, in this order: `layers`, `lint`, `build`, `clippy`, `tests`, `dual-all`, `dual-drv`, `dual-wide`, `board-t`, `board-g`, `sync`, `alpha`, `bench-off`, `bench-resolver`, `bench-state`, `bench-resolver+state`, `census`, `soak`, `d4gate`, `ratchet`. The commands, flags, argument orders (`dual_run CONFIG GAMES AREAS --shard`, `board_equivalence golden CONFIG GAMES STRIDE`, `core_sync_gate CONFIG GAMES STRIDE --strict`, `alpha_roundtrip DIR --strict`, `golden check --mask`, `verify_benches.sh BIN WORKERS`, `soak.sh run` env, `mutation_ratchet.sh` env), the binaries (all six bridge bins and `selfplay` exist and are built by the `build` step), `/root/heavy2.sh` (re-exec holds the lock slot through `exec`, the child steps inherit it), `rsync`, `/var/lib/ti4-public`, `profile-20.json`, `mutation_floor.json`, `marks_census.py`, `d4/coverage.py` and the append-mode of the bail probe (`TI4_BAIL_PROBE` shared by the three shards) were checked against the sources and match. `check_layers.py core`, `lint_core.sh` and `git diff core-integration-6 -- web/ docs/RULES_VERSION.md crates/ti4-model/src/state.rs` (the `lint` step's immutable-surface check) are clean on this tree, and `cargo tree -p ti4-engine-core-tests -e normal,dev` shows 0 engine crates (`d4gate` step 3).

Things that look wrong or weak (none changed; for the person who runs it):

1. `d4gate` runs `d4_gate.sh 3 4 5`, and **step 5 is a silent SKIP**: it calls `tools/engine_core/golden_check.sh`, which does not exist ("SKIP (not built yet)" counts as ok). The golden replays are covered by the `census` step (`golden check --mask all=core` and `all=string`), so nothing is missing, but the title of the step promises more than it runs. Step 4 (`coverage.py --check`) reads the covers file from `$CARGO_TARGET_DIR/d4-covers.txt`, which only exists after the `tests` step ran the D4 crate with the same target dir (it does, in order); it was not run here (it needs that file).
2. The `clippy` step's filter (`crates/ti4-engine/src/(dual|core_sync/|render/)`) does not include `combat.rs`, `hero_dual.rs`, `leaders_core.rs` and the other engine files, and the cargo exit status is ignored: a warning in those files, or a build error in a crate outside the list, passes this step (the `tests` step would still catch the error). Suggested pattern: `crates/ti4-engine/src/` with an allow list of the old warnings (22 in the lib today), or compare against a stored baseline.
3. `lint` compares against `BASE_REF=core-integration-6`, a local branch: on another clone or after a branch deletion the step fails with an unknown revision (set `BASE_REF`).
4. `tests` runs `cargo test -p ... ` with a second package set next to the lockstep ones (see the comment in `.cargo/config.toml`): it builds the engine twice, which is slow but correct. `cargo test -p ti4-wasm --features ts` runs the TS wire test; it exports to a string (`export_to_string`), so it does not dirty the checkout.
5. `census` needs `bail-probe.tsv` from `dual-all`; `--only census` alone fails at `test -s` by design, and `--from census` after a skipped `dual-all` too.
6. The step `dual-all` sets `TI4_BAIL_PROBE` for three shards that append to one file: fine (append mode), but the lines of the three shards interleave, so `bail_table` must not rely on line order (it merges by label).

Nothing was run: no step, no lock, no target dir.
