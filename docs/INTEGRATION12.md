# core-integration-12: merge record (2026-11-01)

Branch `core-integration-12`, cut from `core-integration-11` @ `5f928861`. Six merges in the ordered list (G-silent, G2, G3, M6d, H2, F2c's dispatch2), cheap checks only (the user rule: heavy work waits for a
zero-bail playthrough). Not pushed. The live branch `nightly-fixes-2026-10-26` was not touched. NOT merged, by order (still running, they come in integration 13): `core-g4-abilities`,
`core-h3-leftovers`, `core-d2-c3-dispatch3`.
Pure refactor: `RULES_VERSION`, serde, fixtures and fingerprints are unchanged (no file of those was edited by a resolution; the native_play fingerprint of the quick game is `1ccb3be61bfa0cca`).

## 1. Merges

| # | Branch | Tip merged | Merge commit | What it brings | Files in conflict |
|---|---|---|---|---|---|
| 1 | `core-g-silent` | `1ef52a35` | `c5950928` | lane G: the registry audit (`ability_audit.rs`, every string ability is a rule, `Dropped`, `Carried`, `Lane` or a loud `Gap` row), `silent::gaps` (61 rows, `Unknown::AbilityGap(slot)`), Gift of Prescience, Mitosis, the Infantry II / Letani / Crimson Legionnaire returns, Scavenge, Pillage | 8 |
| 2 | `core-g2-abilities` | `959089b0` | `8f7acf29` | the Winnu (Reclamation, Reclaimer, HTP, Berekar Berekon), Muaat commander, Sai Seravus, Acamar, Voidwatch, Xuange, Stillness of Stars, Claire Gibson, Garv and Gunn, Dart and Tai (19 rows) | 2 |
| 3 | `core-g3-abilities` | `e5f33fa7` | `526e0c66` | Zeta, Ragh's Call, Tekklar, T'ro marks, Alastor, Exotrireme II, Z'eu, Acidos, Suffi An, S'ula Mentarion, Bioplasmosis, the Oracle, Scepter, IFF (20 rows), `silent3` questions | 9 |
| 4 | `core-m6d-tactical` | `44fa75a0` | `2f0a9591` | Awaken, Ouranos, Arborec commander / flagship (a PRODUCTION window as a child of an ability), So Ata, the Ssruu copy of I48S, Mercer, Nova Seed (map replay), Sai Seravus, Xuange, Acamar (14 rows), `choice::activation` | 17 |
| 5 | `core-h2-leftovers` | `5d2b13ce` | `460f19a3` | Strike Wing Alpha II, Strike Wing Ambuscade, the Argent commander (3 rows); **Thunder's Edge Warfare's free tactical action (the E-to-M6 join)**; the systems of the free action | 22 |
| 6 | `core-d2-c3-dispatch2` | `1234b33a` | `423f3737` | F2c: the shadow gives the string the answers of questions it asks inside a step, the aftermath mirrors its sub-windows' events, one code table, Ggrocuto Rinn and Carth from the menu | 2 |

Later commits on the branch are fixes found by the checks (section 3). The neutral-side bail of `dual_window::settle_run` was checked after EVERY merge
(`awk '/^fn settle_run/,/^}/' crates/ti4-engine/src/dual_window.rs | grep -ci neutral`): **0 each time** (it did not resurface). `cargo check --workspace --all-targets` (without ti4-review,
ti4-replayer, ti4-mlp, ti4-tensor, ti4-advisor) was green after every merge.

## 2. Conflicts and how they were resolved (keep both sides)

| File | Conflict | Resolution |
|---|---|---|
| **gap rows**: `silent/gaps.rs`, `ability_gaps.rs`, `ability_audit.rs` (merges 2-5) | positional rows (`41_000 + index`), `Gap(i)` numbers and `SUFFI_AN*` constants rewritten by every lane | for each merge: HEAD's three files, then `tools/engine_core/delete_gap_rows.py <ids>` with the ids the other lane deleted (computed as the difference of its `GAP_IDS` and its base's): G2 19, G3 20 (took G3's file, which also removes the `SUFFI_AN` constants and changes Foresight's condition, then G2's 19 by id), M6d 10 (it deleted 14, four were G2's), H2 3. **61 rows -> 9 left**: Foresight, Propagation, Titans commander, Spy Net, Stymie, `asn_clear`, Magen Defense Grid hit (`md_hit`), Yin Spinner Omega, Naalu's TE mech. The script was made tolerant of a lane that removed the `SUFFI_AN` constants. The audit test (`every_ability_the_string_registers_is_ported_or_accounted_for`) is green with no unlisted gap |
| `ability_audit.rs` `Lane(..)` entries (merge 1) | G's `Lane("core-m6c-tactical")` (15) and `Lane("core-m1-trade")` (1) entries: those rules are in this tree | the 16 lines deleted (the test demands it); the `Lane` variant stays with `allow(dead_code, reason)` for the next lane that carries a rule |
| `kernel/ability.rs`, `rules/trade.rs`, `native_subject.rs` (1) | `Unknown::{AgendaGap, AbilityGap}`; `Codes::{agenda, silent}`; G's own `KernelEnv` against M1's `kernel::trade_env` | both unknowns / both codes; M1's `KernelEnv` (G's notes say so); `KNOWN_SILENT` is `[StrategyAbility]` (Mitosis is a rule now) |
| `choice/{reaction,silent}.rs`, `render/{reaction,silent}.rs`, `reactions_core.rs` (1-5) | the question enums of every lane (`G(..)`, `T(..)`, `Act(..)`, `ExtraDie`, `StrikeWing`, `Terragenesis`, ...), `NAMED`, `PAIRS`, the codes, the factions list of the registry test | all variants / names / pairs; the factions list is the union of both lists |
| **slots `42_001..42_007` twice** (merge 3) | G2's Winnu rules and G3's Zeta, Ragh, Tekklar x3, T'ro marks used the same slots | G2's slots move to `43_000..` exactly as lane G4's own merge of the two did (so integration 13 meets the same numbers); Zeta is registered with the Saar module (`g3::register_saar`) before the Winnu's |
| **Sai Seravus, Acamar, Xuange ported twice** (merge 4) | lane G2 (`silent/{ghost,empyrean}.rs`, core tests in `g2_abilities.rs`) and lane M6d (`cc_ground/ghost/timing.rs`, `ce_movement/empyrean/timing.rs`, `activation::{Ask::SaiSeravus, Opt::Fighters, Subtype::SaiSeravus}`) wrote the same three rules under the same string ids | **one machine**: G2's stay (7 core tests pin them), M6d's three rules, their question type, their `NAMED` lines and `ActivationCodes::ghost_commander` are deleted. M6d's lockstep tests (`tactical_live`, `combat_live`) run unchanged against the survivor |
| registry order (merges 4, 5) | the combined table put So Ata's clear (`ACTION_COMPLETED`) before the Winnu's Reclamation, and Nova Seed (Muaat) before Voidwatch / Acamar in `MOVEMENT_FINISHED` / `SHIP_MOVED` | `cd_registry::register_last_abilities` (the Yssaril module is the last of the string order) after `silent`; `silent::register_before_muaat` (Empyrean, Ghost) before `ce_movement::register_late_abilities`. The registry tests found both |
| `tactical_live.rs` (4) | HEAD's exact `trigger.event_id` offset (1 or 2) against M6d's blanket `trigger.event_id` filter plus `without_seen` / `is_turn_flow`; `planet_hub()` against M6d's `planet_hub(n)` | M6d's `without_seen` and `is_turn_flow` with HEAD's exact offset (the blanket filter was NOT taken); M6d's helper renamed `centre_hub` |
| `strategic/begin.rs`, `tactical/tail.rs` (5) | H2 calls `begin_free` / `open_after`; the base called `resume_strategy` (`TacticalGap::{LeaderStrategy, WarfareSecondary}`) | H2's; `resume_strategy` and the two gap variants deleted (section 5) |
| `turn_jobs2.rs` test, `TurnCodes` (5) | H2 generalised the free-leader test (one seat, any action-phase agent) over HEAD's Carth; H2's side re-adds `TurnCodes::{production, units}` | H2's test; the fields stay deleted (`codes.production`, integration 11) |
| `native_play.rs` tests, `native_subject::without_staged_marks` (6) | the counts of the quick game; F2c's helper (int. 11 had deleted E's copy; F2c's shadow uses it again) | F2c's side (counts re-measured, they passed unchanged); the helper is back |
| docs (all) | the older `W14-design-04..06` rows came back beside the renamed `W14b-` ones | the renamed rows |

## 3. Bugs and breaks found, and how each is guarded

"Fails before": R = seen to fail on the code before the fix, B = by construction (the removed code is what the test pins).

| # | Where | What | Fix | Guard |
|---|---|---|---|---|
| B1 | slots (merge 3) | **two lanes' rules in one slot.** G2 and G3 both numbered `42_001..42_007`; with `AbilityTable::add`'s duplicate check a `debug_assert` only, the lockstep-profile lib run saw no panic, only seven string abilities that no rule answered (the audit), a wrong emitter table (`slot 42050 .. no entry in G3_EMITTERS`: G2's Stillness fell into G3's range) and two registry tests | G2 moves to `43_000..` | new `reactions_core::tests::no_two_rules_of_the_core_table_share_a_slot` (R: with `RECLAMATION_MARK` back on `42_001` it panics `slot 42001 is registered twice: (Promissory, UnitsCommitted) and (Ability, PlanetControlGained)`); before it: `ability_audit` (3 tests) and the two registry tests failed on the merge |
| B2 | registry order (merge 4) | Nova Seed (M6d) registered before the Empyrean / Ghost rules of G2 in `MOVEMENT_FINISHED` and `SHIP_MOVED`; So Ata's clear before the Winnu's Reclamation in `ACTION_COMPLETED` (string order: Empyrean, Ghost, Muaat; Winnu ... Yssaril) | `register_before_muaat`, `register_last_abilities` | `reactions_core::tests::{the_native_registry_equals_the_string_one_for_the_ported_abilities, the_registry_of_the_g3_rules_equals_the_string_one}` (R: both failed with "the order of the registry differs at 5 / 46", once per cause) |
| B3 | a rule ported twice (merge 4) | Sai Seravus, Acamar, Xuange (section 2) | one set | the same registry tests (a second registration shows as an extra entry of the bucket) and the G2 core tests (7) |
| B4 | the E-to-M6 join, whole game (checks) | none found in the code: H2 did the join; the whole game had no test (section 5) | -- | new `native_play::tests::a_thunders_edge_warfare_primary_plays_its_free_tactical_action_inside_a_whole_native_game` (R: with `begin.rs` turned back into a gap it fails `the Warfare primary of this game begins its free tactical action natively`) |
| B5 | `strategic_live_tests::a_thunders_edge_warfare_primary_runs_its_free_tactical_action_and_the_follower_window_like_the_string` (whole-lib run after merge 6) | H2's lockstep filtered `INVASION_BEGAN`, `PRODUCTION_USED`, `UNITS_PRODUCED` out of the STRING's events ("the native run does not write them to the log"); lane F2c's `chain::mirror_then_flush` writes them now, so the native list had three events the filtered string list lacked. The test passed on the H2 merge (178 filtered tests) and failed on the dispatch2 merge, in every order: a semantic break of two lanes that only the combination shows | the filter is removed: the whole list is compared | the test itself (R: left lacked the three events); see section 4 |
| B6 | `dual_window::event_of` (merge 5) | the `List::Produced` arm vanished in a clean textual merge (third time since integration 10, B7 there) | restored | the compile (non-exhaustive match) |
| B7 | `silent::Ask::carries_trigger` (merges 3, 5) | the `match` of G3 met the variants of G2 (merge 3) and of H2 (merge 5) | `false` for the G2 questions and for `ExtraDie` / `StrikeWing`, as their lanes had it (`matches!(.., Landing \| Mitosis \| Pillage)`) | the compile |
| B8 | unused imports / dead helper after the resolutions (`board`, `Faction`, `Leader`, `TacticalGap`, `Unknown`, `gaps::copies`) | -- | removed | `clippy_core.sh` |
| B9 | clippy (`clippy_core.sh`) | `g2_abilities`: `assert!(rig(..).is_done() || true, ..)` -- **a vacuous assertion** (clippy: "this boolean expression contains a logic bug"); lane G3's `ragh.rs` very complex type; `native_play::compare_group` used by tests only; an unneeded `mut`; a `to_owned` | the assertion made real (a Naaz seat holding its agent and no Ssruu: its own window is offered, asked of seat 1 -- it passes); the rest cleaned | `clippy_core.sh`; the strengthened assertion |
| B10 | D4 crate (`tactical_composer::the_tail_of_a_heros_free_action_opens_the_job_after_it_too`, found by the full D4 run) | **two lanes disagree on who opens `Finish` after a plain tactical action**: M6b's tail does (`turn::open_finish` in `tail::run`), H2 removed that and left it to the native loop's `after_arm` (which opens it only when no job is open) and pinned "the tail opens nothing" | the merged tail keeps M6b's call (a caller without the loop gets the end of the action too; `after_arm` skips when a job is open, so both agree); H2's assertion now pins `Some(TurnJob::Finish)` for a plain tail and still `AfterTactical` for a free action | the test itself (R: `left: Some(Finish), right: None` on the merge) |

Not a bug, a trip-wire that fired as written: `native_play::tests::the_shadow_run_agrees_with_the_legacy_game_up_to_the_known_exploration_difference` (integration 11) "fails the day the difference is
fixed": lane F2c fixed it (the difference was the shadow's, not the game's), so the pin is replaced by F2c's `..._agrees_with_the_legacy_game_up_to_the_gap`.

## 4. Edits of assertions, fixtures and harnesses (reasons)

| Where | Edit | Reason |
|---|---|---|
| `native_play` shadow test | the pinned difference of step 24 -> `first_difference == None` and `End::Gap(_)` (F2c's test) | the difference was the harness' (F2c: the string asks the exploration reward inside the step; the shadow now answers it there); the trip-wire fired |
| `strategic_live_tests` Thunder's Edge Warfare lockstep | the filter of three typed events removed (B5) | tightened, not loosened: the comparison now covers the events of the aftermath's windows that F2c's mirror writes |
| `ability_audit.rs` `ACCOUNTED` | the 16 `Lane(..)` lines deleted; the 9 `Gap(i)` renumbered by the script | the test demands it when the lane merges |
| `reactions_core.rs` `bail_every_family_that_stays_in_the_string_is_named` | the lines of Mercer, Temporal Command Suite, Ambuscade, `sardakkhero_turn` are not in the list | all four are rules of the table now (M6d, M1, H2, G3); the test's own premise ("the string registers it, the table has no slot") is false for them |
| `tactical_live` diff filter | M6d's `without_seen` and `is_turn_flow` added; the blanket `trigger.event_id` filter NOT taken (exact offset kept) | argued in section 2: M6d's harness-only fields; the blanket filter would hide a real id difference |
| `g2_abilities.rs` `table()` | also calls `silent::register_before_muaat` | the Empyrean and Ghost rules moved to that call (B2) |
| `turn_jobs2.rs` | H2's generalised free-leader test | H2 found Hacan's Guild Ships made a transaction partner a free option of its own |
| `tactical_composer.rs` hero's-tail test | a plain tail opens `Finish` (was: nothing) | B10: argued there; no test lost a property |
| `g2_abilities.rs` | `is_done() \|\| true` -> the window is offered and asked of seat 1 | B9: the old assertion could not fail |
| `reactions_core.rs` | new test; threshold 150 (the whole table has 176 rules) | B1 |

No assertion was loosened to make a test pass.

## 5. The E-to-M6 join (INTEGRATION11 section 8 item 4)

Lane H2 had written it (`strategic/begin.rs::run_begin` calls `tactical::begin_free(sx, active, system)` with the follower window held; `tactical/tail.rs::run` calls
`strategic::after::open_after` for `Then::LeaderStrategy | Then::Secondary`), with a core test (`tactical_composer::thunders_edge_warfares_free_action_begins_inside_its_job_...`) and a lockstep against
the string (`strategic_live_tests::a_thunders_edge_warfare_primary_runs_its_free_tactical_action_and_the_follower_window_like_the_string`: redistribution, the system question, the free action, the second
redistribution, the follower window, `STRATEGIC_ACTION_COMPLETE`). The merge deleted the seam it replaces: `tail::resume_strategy`, `TacticalGap::{LeaderStrategy, WarfareSecondary}` and their text.
**New here: the whole-flow test in a native game** (B4): sol, 4 players, seed 4 plays the card on its 47th step, in the string's event order (`FREE_TACTICAL_ACTION`, `STRATEGIC_ACTION_BEGAN`, movement, the
tail, the followers, `STRATEGIC_ACTION_COMPLETE`, `ACTION_COMPLETED`, `TURN_PASSED`), and no game of seeds 1-12 stops at the card.

**The quick game (sol, 4 players, seed 1) does NOT stop at the Warfare card any more.** Its stop at step 54 (`TurnGap::StrategyAbility`) is the **Technology primary** (`fx_perform::begin` ->
`Began::Unsupported`: `carried.research` false or no roster, `strategic::facts::research_asks_more`, Doctor Sucaban); a temporary print over all 96 games found 16 `StrategyAbility` stops, **all 16 the
Technology primary** (seats 0, 1, 2, 4 of the tables); no Warfare, no hero primary, no waiver. Owner: lane E (the research facts).
One harness limit, not a game difference: `--quick --shadow` of seed 4 reports a first difference at step 51 because the native secondary step glues the seat's after-redistribution question
and the first follower's question, the legacy step ends after the first (legacy answered `[done_moving, 1|2|3, no]` and asked two). The questions of a glued step should be owed to the next group like the events
(`Owed`); not done here.

**Scheming's two machines (`DrawCards` of l14 and `HookedDraw` + `SchemingDiscard` of lane D) are NOT unified**: 597 lines in two shapes (`DrawCards` inherits the hooks and is used by seven hosts: strategy
primaries, legendary, explore windows, promissory windows, hero windows, agenda effects, G3's `draw`; `HookedDraw` is the status phase's with Law's Order's blank turn), and no test pins the equality
beyond the lockstep ones. The plan for a later pass is in section 8.

## 6. The native playthrough (`tools/engine_core/native_play.sh`, gate profile, `te` card set)

`--quick` (sol, 4 players, seed 1): **54 native steps, then the first gap** `ask.action_menu [bail: TurnGap(StrategyAbility)]` (the Technology primary, section 5). It plays the draft, the announcement, the phase
flip, nine turn preludes, eight action menus (a Xxcha agent from the menu: Ggrocuto Rinn), a strategic action with its follower windows, and a tactical action with its aftermath. Integration 11: 43 steps, stop at a leader's component action.

`--quick --shadow`: 54 native steps, **36 legacy steps compared with NO difference** (integration 11: the first difference at step 24), then the same gap.

The gap matrix over `--factions all --players 3,4,5,6 --seeds 1` (96 games, all stop in round 1; **3,729 native steps** in all; integration 11: 3,306). First gap per game (steps then cause; owner below):

| faction | 3 players | 4 players | 5 players | 6 players |
|---|---|---|---|---|
| sol | 13 PA | 54 SA | 40 SA | 100 L5 |
| hacan | 96 F11 | 17 SA | 16 PA | 17 L5 |
| letnev | 13 SA | 77 SA | 27 SA | 19 L5 |
| xxcha | 14 PA(sec) | 115 SA | 56 L5 | 9 PA |
| jolnar | 13 PA | 17 PA | 27 L5 | 19 L5 |
| l1z1x | 13 PA | 17 PA | 12 PA | 19 L5 |
| arborec | 13 PA | 11 L5 | 40 SA | 44 L5 |
| mentak | 13 PA | 17 PA | 12 PA | 133 TOK |
| muaat | 28 F11 | 11 L5 | 40 SA | 44 L5 |
| naalu | 13 PA | 17 PA | 41 AG | 127 TOK |
| nekro | 26 L5 | 11 L5 | 13 L5(closing) | 14 L5(closing) |
| saar | 13 PA | 17 PA | 12 PA | 133 TOK |
| sardakk | 13 PA | 17 PA | 12 PA | 125 TOK |
| winnu | 84 TOK | 63 SA | 104 SA | 69 L5 |
| yin | 13 PA | 17 PA | 12 PA | 133 TOK |
| yssaril | 13 PA | 17 PA | 85 PA | 46 L5 |
| ghost | 13 PA | 17 PA | 12 PA | 140 TOK |
| argent | 27 PA | 26 SA | 33 SA | 107 SA |
| cabal | 13 PA | 17 PA | 12 PA | 46 L5 |
| empyrean | 13 PA | 17 PA | 12 PA | 45 L5 |
| mahact | 47 PA | 17 PA | 28 SA | 92 L5 |
| naaz | 13 PA | 17 PA | 63 PA | 133 TOK |
| nomad | 13 PA | 11 T6 | 40 SA | 44 T6 |
| titans | 13 PA | 17 PA | 12 PA | 133 TOK |

| code | first gap | games | owner |
|---|---|---|---|
| PA | `ask.action_menu`: `TurnGap::PeaceAccords` (a Xxcha seat uses Diplomacy / its ability: the annexation window) | 45 (+1 `secondary`: `PA(sec)`) | lane E (the annexation window; `docs/G_SILENT_RESUME.md` says what it needs) |
| SA | `ask.action_menu`: `TurnGap::StrategyAbility` = the Technology primary (research facts) | 16 | lane E |
| L5 | `Menu(Unrouted(5, key))`: a leader's component action (agent, commander, hero, Ssruu copy; keys 67108864 = 13, `..914`, `..924`, `..917`) | 13 + 2 + 2 + 2 (closing menu: Nekro) + 1 (`83886080`) | lane F2c: `leader::run`, one arm per leader (`docs/KM3_RESUME.md` section 4) |
| T6 | `Menu(Unrouted(6, key))`: technology (Sling Relay) | 2 | F2c / production (`Arm::Production` cannot be selected by `step_arm`) |
| F11 | `Menu(Unrouted(11, key))`: a faction component (Hacan, Muaat) | 3 | F2c (`docs/KM2_RESUME.md` section 3) |
| TOK | `tokens`: "close lent: the status steps 81.6-81.8 after the last token (KS5)" | 9 | lane C (status) |
| AG | `tactical`: `AbilityGap(41000)` Foresight (Naalu) | 1 | lane G (Foresight needs the fleet relocation) |

Native steps by arm (all 96 games): draft 600, starting technology 12, announce 96, phase 105, turn prelude 601, action menu 521, closing menu 20, tactical 874, aftermath 284, secondary 488, tokens 93,
status 9, scoring 9, event scoring 9, trade 8. By island: M1 trade 8, M5 aftermath 284, M6 tactical 874, B turn flow 1,434 (85 gaps), C status 120 (9 gaps), E strategic action 488 (1 gap), F menu and
leaders 521; M2, M3, M4, D: not reached. **Compared with integration 11: PeaceAccords 41 -> 45 games (more games get far enough to meet a Xxcha), StrategyAbility 31 -> 16, leaders 21 -> 20, and the first
games that play the whole round 1 into the status phase (9: `tokens`), the trade island played natively (8 steps) and the status phase (9).**

## 7. Checks (cheap), `CARGO_BUILD_JOBS=2`, private target `/root/deploytmp/int12` (profile `lockstep`, `CARGO_PROFILE_DEV_DEBUG=0`, no incremental; deleted at the end)

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (without ti4-review, replayer, mlp, tensor, advisor) after every merge | ok |
| `cargo test -p ti4-engine --lib` plain | **5199 passed**, 7 ignored (integration 11: 5150) |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (WHOLE lib) | 5199 passed, 7 ignored |
| `ti4-engine-core --lib`, `-bridge --lib`, `-factions --lib`, `ti4-rule-bot-typed --lib` | 479, 107, 145, 1 passed |
| bridge `--test kernel_env --test play_facts --test menu_sources` | 3 + 3 + 2 passed |
| `check_layers.py core`, `lint_core.sh` | 556 files, 0 violations; 558 files, 0 hits |
| `tools/engine_core/clippy_core.sh` | ok (core crates 0 warnings; engine 41, baseline 41). The first run found real findings of the lanes (B8, B9) |
| the D4 crate (`ti4-engine-core-tests`) with the debug flags of `tools/engine_core/d4/run_core_tests.sh` (profile `lockstep`, debug assertions and overflow checks on for the three core crates), built once, in my tree (same private target) | 143 test binaries: 1774 passed, **1 failed** (B10, `tactical_composer`); after the fix the six binaries that could be affected (`tactical_composer g2_abilities g3_abilities silent_abilities strategic_arms turn_jobs2`: 25 + 40 + 10 + 36 + 18 + 26 tests... as listed per binary) are green again (25, 40, 10, 36, 18, 26 tests), so 1775 passed, 0 failed |
| `native_play.sh --quick` / `--quick --shadow` / `--factions all --players 3,4,5,6 --seeds 1` | section 6 (run on the tip before the clippy / test-only commits; no engine or core code changed since but Ragh's type alias) |
| the usage: five-hour window 24 % at the start, 37 % at the end | -- |

Where the runs were done: all in this worktree with the private target (no scratch worktree: a second copy of the workspace would have doubled the 2.9 GB target; the runs were background jobs of
this session, one cargo at a time, not Haiku runners). The target dir and the logs under `/root/deploytmp/int12*` are deleted at the end.

Not run (user rule): `end_of_d1.sh` and its steps (golden / wide dual, `board_equivalence`, `core_sync_gate`, `alpha_roundtrip`, benches, soak, mutation ratchet, census, workspace clippy beyond
`clippy_core.sh`), `decision_replay`, mutation batches (the new lines of this integration were mutation-checked by hand only for B1 and B4), `TI4_CORE_STRICT=1`, Playwright.

## 8. Remaining gates, consolidated

Dropped by decision (not open): as in `docs/INTEGRATION11.md` section 8. Items 1 to 11 of `docs/INTEGRATION9.md` section 6 stand unless named below.

1. **Integration 13**: `core-g4-abilities` (it contains the G2+G3 merge this integration redid: expect conflicts in the same files; the slot numbers match), `core-h3-leftovers`, `core-d2-c3-dispatch3`.
2. **The next first gaps, by games stopped** (section 6): Peace Accords 46, the Technology primary 16, a leader's component action 20, the status `tokens` close 9, faction components 3, Sling Relay 2, Foresight 1.
3. **The 9 loud gap rows**: Foresight, Propagation, Titans commander, Spy Net, Stymie, `asn_clear`, Magen Defense Grid hit, Yin Spinner Omega, Naalu TE mech (`docs/G3_RESUME.md` says what each needs).
4. **`--shadow` harness**: a glued native step that asks two questions the legacy game asks in two steps (section 5) -- owe the extra questions to the next group.
5. **Scheming's two machines** (still): unify `DrawCards` and `HookedDraw` + `SchemingDiscard`: first move the Law's-Order blank turn into `DrawCards`'s host, then make `HookedDraw` a host of `DrawCards`; pin with the status lockstep (`status_live_tests`) and `draw_hooks.rs`.
6. Unchanged from integration 11 section 8: the dispatch of the menu (technology, relics, faction components), the tactical remainder (`CannonHits`, ship routes beyond five), turn flow (`ChaosMapping`, `ImperialArbiter`, `QuantumDatahub`, `Propagation`), production / invasion / aftermath leftovers, action cards played inside another window.
7. Heavy gates still to run once a zero-bail playthrough is possible: golden and wide dual sweeps, `board_equivalence`, `core_sync_gate --strict`, `alpha_roundtrip --strict`, the benches, `census`, `soak`, the mutation ratchet and the mutation batches of every resume note since integration 9 (this integration adds: `delete_gap_rows.py` merge handling, `register_before_muaat`, `register_last_abilities`, the new tests), workspace clippy, `TI4_CORE_STRICT=1`.

## 9. Tip

`git log -1 core-integration-12` (the last commit is this document's). The private target dir was deleted.
