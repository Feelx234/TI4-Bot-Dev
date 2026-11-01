# Lane P (`core-p-shadow2`): the whole-game shadow differences, round 2, resume note (2026-11-01)

Branch `core-p-shadow2`, cut from `core-integration-15` (`3ab5e6e64`). Never pushed; `nightly-fixes-2026-10-26` untouched. Reproduce one game:
`TI4_NATIVE_PLAY_TRACE=1 TI4_NATIVE_PLAY_DUMP=<dir> tools/engine_core/native_play.sh --factions F --players N --seeds 1 --shadow` (new trace lines: `native group:` per
compared group, `group has N questions, the legacy step used M`, and for native steps with no legacy step of their own the events against the owed legacy events).

## Result

| sweep | games | whole game, no difference | first difference of the core | known string bug S1 | gap |
|---|---|---|---|---|---|
| `--factions all --players 3,4,5,6 --seeds 1 --shadow` | 96 | **60** | 27 | 9 | 0 |
| `--first-seed 2 --seeds 3` | 288 | **194** | 69 | 24 | 1 (`StrategyAbility`, lane N) |

Int. 15 started with 96 of 96 games showing a first difference (PB 65, AC 10, NE 9, TG 4, SC 3, S1 3, OT 2). The Z-gate (`--seeds 8 --shadow` exits 0) is NOT met: the
remaining classes are in the last section. The run exits 0 only when every game is clean or stops at the known string bug (the binary prints
`KNOWN STRING BUG ... (known string bug S1)` and counts them apart; they do not fail the run).

## Causes, who was right, the guard

"Fails before" = seen to fail on the code before the fix (a D4/bridge test run on the pre-fix file by a Haiku runner, or the shadow itself).

| # | class (games of 96) | cause | right | fix | guard |
|---|---|---|---|---|---|
| 1 | PB (65) harness | the native `phase` step joined the answers of the last tokens group (cut tail) and was compared with the legacy step that ended at `STATUS_PHASE_ENDED`; the same for the forced last pick of the draft; and the string's idle step ahead of the flip (a step that asks nothing and says nothing) was not caught up when the native group asked nothing | string (its steps end there) | `opens_a_step_of_its_own` (held back once) in `shadow_play`; `steps_idle` in the catch-up loop; `ahead` when a step is carried | bridge `the_phase_flip_after_the_last_tokens_answer_is_a_legacy_step_of_its_own` (sol 3p: step 153), `the_forced_last_pick_of_the_draft_is_a_legacy_step_of_its_own` (jolnar 4p: 380), `a_phase_flip_or_a_forced_pick_opens_its_own_group_once` (unit) |
| 2 | NE (9) core | the aftermath chain mirrored `Out::emitted` (the top-level emissions) of its sub-windows; the events a reaction emits INSIDE another (War Machine's `ACTION_CARD_PLAYED`/`DISCARDED` at `after PRODUCTION_USED`, Naaz's card after `INVASION_BEGAN`) were in the resolver's journal only. The string mirrors every `emit` line of its timing log | string | `Out::journal` (new, every emission of the call's resolver in trace order, `fx::journal_recs`), filled at the four `Out` construction sites; the chain mirrors it. `Out::emitted` keeps its meaning (dual harnesses read it) | D4 `aftermath_chain::an_event_a_reaction_emits_inside_another_is_mirrored_after_its_parent_by_the_aftermath_chain` (fails on the pre-fix chain: `ActionCardPlayed in [ProductionUsed]`) |
| 3 | AC (10) harness | the string asks the reaction at `after ACTION_COMPLETED` inside the step that answers `done_moving`; the native step that asks it also puts the next menu question. The cut of such a step kept its events with the later half, the head was compared strictly | string | a cut group (`ahead`) owes the legacy surplus to the later half (`Owed`), as the non-strict groups do | `a_group_cut_between_two_questions_of_a_native_step_owes_the_legacy_surplus_to_the_later_half` (xxcha 4p: 282) |
| 4 | SC (3, 8 after the others moved) core | the live head's `unit_texts` were empty: a gunner's roll was staged under the roll kind `space_cannon`, the string stages `type_id` (`pds`, `pds2`): the label of a reroll question (Fire Team, Scramble Frequency) | string | `CombatTexts::guns` (every unit form on the board with a space cannon, minted by `combat_live::combat_texts`), passed to `head_facts` | `a_reroll_question_of_the_space_cannon_offense_names_the_unit_type_of_the_gun` (mentak 5p: 178) |
| 5 | TG (4, 20 after the others moved) core + harness | (a) the string announces the events a prelude question staged (Psychoarchaeology's trade good) at the start of its NEXT step; the native step after the prelude ran its reconcile first. (b) a component/leader action announces the staged supply events (`announce_staged_ground_events` also runs `supply::try_flush_staged_events`), the core's `component::announce_staged` did only the ground forces | string | (a) `NativeGame::after_glue`: the step after a finished glued step runs no reconcile; (b) `component::announce_staged` uses `prelude::announce_staged` (ground forces, then staged events) | `the_events_a_prelude_question_staged_are_announced_by_the_next_legacy_step_and_a_component_action_flushes_them` (sardakk 3p: 166, naaz 3p: 110) |
| 6 | (ghost, argent) harness | the same reconcile glue after a finished aftermath: the retreat's `COMMAND_TOKEN_PLACED` (staged) was announced by the reconcile of the tactical tail step, before `TACTICAL_ACTION_COMPLETE`; the string announces it at the next step | string | `after_glue` also after a finished `Aftermath` and through a question-less `Tactical` tail | `the_tail_of_a_tactical_action_after_a_finished_aftermath_announces_no_staged_event_of_its_own` (ghost 4p: 517) |
| 7 | (nomad, sol, muaat, winnu) core | AI Development Algorithm's question is a TAPE question of the production use (`run_on_tape`): inside the string's step. The chain treated it as a stage question (`Asking::nested` is `None` without display facts) and flushed the aftermath's own line (`INVASION_RESOLVED`) before `PRODUCTION_USED` | string | the chain also checks `Pending::nested` | D4 `aftermath_chain::the_production_used_event_follows_a_nested_question_of_the_use_before_the_aftermaths_own_line` (fails before: `[InvasionBegan, InvasionResolved, ProductionUsed, ..]`), bridge `a_tape_question_of_the_production_use_keeps_the_aftermath_line_behind_the_use` (nomad 4p: 285) |
| 8 | S1 (3) core + harness | user ruling: the ACTIVE player first, then the others in INITIATIVE order. The core rotated its initiative list from the active seat | ruling | `Resolver::player_order` (action-phase branch); the strategy and agenda phases keep speaker + seating clockwise. `docs/STRING_ENGINE_BUGS.md` S1 updated. The shadow recognises S1 (`Difference::known`, `s1_in_error`, `s1_in_questions`: a reaction question differing only by seat/order while the string's order differs from the ruling's) | D4 `resolver_basics::an_action_phase_window_offers_the_active_player_first_and_the_others_in_initiative_order` (fails before), bridge `a_reaction_window_the_string_orders_by_its_seating_is_reported_as_the_known_string_bug_s1`, `s1_is_recognised_only_for_a_reaction_and_only_when_the_two_orders_differ` |

The game itself changed with cause 8 (the quick game is 1,269 native steps now, fingerprint `24b0d67462f4772b`, `--twice` the same; was 1,198 / `2e93df9fa00fa982`).

## Edits of assertions (reasons)

| Where | Edit | Reason |
|---|---|---|
| `resolver_basics::players_are_asked_from_the_speaker_...` | action-phase expectation `[0,1,2]` -> `[0,2,1]` (initiative list `[2,0,1]`, active 0) | the ruling of cause 8 |
| bridge `a_native_game_plays_a_whole_game_with_zero_gaps` | steps 1198 -> 1269, fingerprint | the ruling changes the questions' order: a different game |
| bridge `the_round_the_asks_of_a_job_and_the_shadow_count_are_the_positions` | compared 66 -> 61 | the glue of causes 1 and 5 |
| bridge `..._up_to_the_known_nested_events_of_a_reaction_card_in_the_production_window` | replaced by `..._up_to_the_known_follower_window_of_the_technology_card` (293 steps, then step 448: native asks the follower `c`, the string `d`) | cause 2 fixed the pinned difference, as the pin said it would be; the next difference of the quick game is pinned the same way |

No assertion was loosened. The only comparison that became less strict is the harness' event match of a CUT group (cause 3), and the owed events are compared in the next group.

## Checks (cheap), private targets `/root/deploytmp/core-p` (gate) and `core-p-t` (lockstep, Haiku runners on committed snapshots)

| Check | Result |
|---|---|
| D4 crate (`ti4-engine-core-tests`, lockstep, debug assertions on for the three core crates) at `e0f3c3c53` | 154 result lines: **1952 passed, 0 failed** (int. 15: 1949; +3) |
| `ti4-engine-core --lib` | 477 passed |
| bridge `--lib native_play` | 41 passed (18 s) |
| `ti4-engine --lib` (plain, at `d749d40bc`) | **5286 passed**, 0 failed, 7 ignored (the first run at `e0f3c3c53`: 1 failed, `resolver_sync::random_play_of_a_seated_game_has_no_dual_difference`, the S1 order in the kernel dual: fixed by handing the dual's core resolver the string's rotated list, `kernel_sync/shim.rs`, `reactions_core::resolver_of`) |
| `ti4-engine-core-factions --lib`, `ti4-rule-bot-typed --lib` | 151, 1 passed |
| not run (user rule) | the `TI4_AREAS=all=dual` whole-lib run, golden / wide dual, benches, soak, census, ratchet, workspace clippy; `clippy_core.sh`, `check_layers.py`, `lint_core.sh` (see the end) |
| mutation | see below |

## Mutation (`tools/engine_core/mutants.sh`, single ranges, snapshot `d5e0e9c64` and `38e6cde0f`, `D4_COVERS_FILE=/dev/null`, Haiku-run queue plus a manual check)

Set A = the D4 tests (`aftermath_chain`, `resolver_basics`, `tactical_composer`), set B = the bridge `native_play` tests (the shadow tests of this lane; they ARE the lockstep of
the harness and of the core halves that only a whole game reaches: `TEST_CRATE=ti4-engine-core-bridge`). The engine lockstep filters (J's set B) were run as plain
`ti4-engine --lib`: 5286 passed.

| range | mutants | killed | survivors |
|---|---|---|---|
| `chain.rs` 405-420 (`sync_initiative`, `merge`; A) | 2 | 2 (after the new test `the_chain_syncs_the_resolver_with_the_initiative_order_of_the_position`; 1 survivor before it, a pre-existing body) | none |
| `chain.rs` 455-495 (journal extends, the nested condition; A) | 5 | 5 | none |
| `resolver.rs` 816-845 (`player_order`; A) | 8 | 8 | none |
| `fx.rs` 100-140 (`journal_recs`, `mirror_journal`; A) | 9 / 8 after the skip | 7 -> 8 after `..._with_its_own_payload` was added to the nested-events test | the fallback record of a failed emission (`cancelled: false`, now `// mutants:skip` with the reason: a trace line without a journal entry is the string's bare name) |
| `native_play.rs` 740-780 (S1 recognition; B) | 27 | 26 -> 27 (the survivor, the missing offer in `s1_in_error`, got an assertion) | none |
| `native_play.rs` 935-955 (`opens_a_step_of_its_own`, `steps_idle`; B) | 11 | 10 -> 11 (`steps_idle` got a unit test) | none |
| `native_play.rs` 400-430, 480-492 (`after_glue`; B) | 2 (+2 not compilable), 9 | all | none |
| `native_play.rs` 815-835, 868-905 (the shadow loop; B) | 8, 17 | 5, 9 | pre-existing lines (`native.steps >= max_steps`, the cut conditions `have > used && used > 0`, the trace print) and `flush_first = true` (now `// mutants:skip`: a defensive bound, a held-back step is never held back twice) |
| `component.rs` 60-90 (`announce_staged`; B) | 5 | 2 | 3 pre-existing lines (the destruction drain, `halted`) |
| `facts.rs` 108-130 (B) | 1 (+1 not compilable) | 0 | `announce: true`, pre-existing |
| `combat_live.rs` 170-200 (`guns`; B) | 2 | 0 with the first test (the game it reached at step 178 had moved on with the ruling); killed (checked by hand: the condition replaced by `false && ..`, the test fails) once the test runs to the native step 195 | none |

The cross-crate runs of `mutants.sh` (`TEST_CRATE` another crate) report the engine mutants of `combat_live.rs` as survivors when the test cannot reach the code; the manual
check above is the evidence for that range.


## What is left, by class (owner: the next shadow lane unless stated)

First differences of the 96-game set (27) and of the 288-game set (69) that are NOT S1. Games per class in the two sets (96 / 288):

* **Mark abilities the string offers as choices (OT, 6 / 6 + 2)**: `ability:winnu:reclamation_tactical:SYSTEM_ACTIVATED:after`, `leader:sardakk:sardakkagent_mark:...`: bookkeeping abilities (they only write a mark) that the string registers as real abilities; with another eligible reaction in the same window the string asks the player to choose, the core carries them as `hooks::marks` and offers `decline` only. Probably the "wanted `done_moving` but offered `reaction:...:SYSTEM_ACTIVATED:after`" family of the 288 set (about 12 games) too. The fix is to register them as `Spec`s in the core table (`reactions_core.rs` audit rows `Status::Carried`).
* **Staged `TRADE_GOODS_GAINED` at the activation (TG, 5 / 8)**: `native TRADE_GOODS_GAINED | legacy SYSTEM_ACTIVATED` (and `MOVEMENT_FINISHED`): the same family as cause 5 at the start of a tactical action (E-Res Siphons, Psychoarchaeology on the activation step); not traced.
* **Retreat command token in the aftermath (3 in the 288 set)**: `native COMMAND_TOKEN_PLACED | legacy SPACE_COMBAT_RESOLVED`.
* **Warfare redistribution after a removal question (4 / 4)**: `script wanted "" but c was offered ["0|2|4", ...]`: after the over-fleet-supply removal questions the string asks the "after the tactical action" redistribution, the native does not (letnev 3p, 4p, argent 4p, empyrean 5p). Not traced.
* **Reaction windows of `ACTION_CARD_PLAYED:when`** (5 / 5): unflagged because the active seat's order did not differ; the card player's window order may be a second S1-like rule (not the active seat). Check the string's `player_order` use for the card window.
* **Technology follower eligibility (quick game, step 448)**: the string does not ask seat `c` for the Technology secondary (`c` holds 3 strategic tokens), the core does.
* retreat announcements / hits (`announce a retreat from 62`, `assign a hit`, `HITS_TO_ASSIGN`, `munitions`), the Cabal secondary, the 82a option: one game each in the 96 set, not traced.
* The gap of the 288 set (`TurnGap::StrategyAbility`, mahact 3p seed 67 among them) is lane N's (Jae Mir Kan's waiver).
