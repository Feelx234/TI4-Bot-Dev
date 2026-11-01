# Lane J (`core-j-shadow`): the whole-game shadow differences, resume note (2026-11-01)

Branch `core-j-shadow`, cut from `core-integration-13` (`52f3ae0ef`). Never pushed; `nightly-fixes-2026-10-26` untouched. Read `docs/INTEGRATION13.md` section 6 (the matrix),
`docs/STRING_ENGINE_BUGS.md` (new: S1 and two harness limits) and this note.

## Result

`native_play.sh --factions all --players 3,4,5,6 --seeds 1 --shadow` (96 games): first differences **35 -> 1** (the one left is S1, the string's: see below). Reproduction of one
game: `TI4_NATIVE_PLAY_TRACE=1 TI4_NATIVE_PLAY_DUMP=<dir> tools/engine_core/native_play.sh --factions F --players N --seeds 1 --shadow` (trace prints every step's questions, the
legacy step's answers / questions / events and the native events; `TI4_NATIVE_PLAY_DUMP` (new) writes the legacy position before (`base-<step>.json`) and after (`after-<step>.json`) each
compared step).

## Causes, who was right, the guard

| # | games | cause | right | fix | regression test (fails on the pre-fix code) |
|---|---|---|---|---|---|
| 1 | 21 of 27 (the other 6 show a second difference behind it) | The follower window closes in the call of the last answer; the string closes in a step of its own when the answer is not the last PLACE of the list (`is_complete` counts places; the followers left in the list are skipped by the next step). The events the follow staged (`STRATEGY_TOKEN_SPENT`, `UNITS_PRODUCED`) are announced at the start of that step, BEFORE `STRATEGIC_ACTION_COMPLETE` | string | `Held::holds` (new, default false) + `Live::Done` with `Out::held` (new): `SecondaryWin` ends the call with the window open when `cut` (the answer left places) and nobody who may follow is left; the next call (no question) skips them, closes and runs the `Finish` job. `live_step_inner` treats a held call as a step that is not finished | `strategic_arms::{an_answer_that_is_not_the_last_place_of_the_list_ends_the_call_and_the_next_call_closes_the_window, the_events_a_follow_staged_come_before_the_close_...}` |
| 2 | 4 (`PRODUCTION_RESOLVED` against `UNIT_ABILITY_ROLLED`: xxcha 5p, yssaril 5p, mahact 5p, arborec 6p) | The space cannon offense of the head (`TacticalAction` `Heading`) emitted `UNIT_ABILITY_ROLLED` / `SPACE_CANNON_HITS` to the resolver only; nobody mirrored them into the event queue (the aftermath chain mirrors its sub-windows, the tactical head did not). Every shadow game with a gunner who scored a hit in the activated system lost them | string | `journal_mark` / `mirror_journal` around `head.step` and `head.answer` in the `Heading` arm (the string's `mirror_timing_log`) | `tactical_composer::the_space_cannon_offense_of_the_head_is_part_of_the_steps_events_even_with_no_ship_of_the_active_seat_to_hit` |
| 3 | 2 | A free tactical action (Thunder's Edge Warfare's primary, Z'eu) wrote no Winnu / Sardakk-agent activation mark (`hooks::marks` ran in the hook chain of the ordinary activation only); the string's abilities answer the typed `SYSTEM_ACTIVATED` of the free action | string | `hooks::marks` is called in the `Free` stage and in `begin_free` | `tactical_composer::a_free_tactical_action_leaves_the_marks_of_the_activation_the_string_leaves` |
| 4 | 1 | The aftermath chain lost the events its earlier sub-windows emitted in a call that ends with a question from inside an emission (`INVASION_BEGAN` before the Winnu agent's reaction to `PRODUCTION_USED`): the resumed call starts with an empty list | string | `mirror_recs` of the pending list at a nested question (the aftermath's plain log stays in the window) | `aftermath_chain::the_events_an_earlier_step_of_the_chain_emitted_survive_a_reaction_question_of_a_later_one` |
| 5 | 1 | A staged card move (the Scheming discard of a Yssaril follower) was announced only at the places that called `announce_staged_cards`; the string's `Live::emit_typed` ends with it for EVERY typed event, so the discard follows `ACTION_COMPLETED` | string | `inline::emit_typed` announces the staged cards (marks slice loaded); `emit_typed_bare` for the announcement itself | `turn_jobs2::a_card_move_staged_earlier_in_the_action_is_announced_right_after_the_typed_event_that_ends_it` |
| 6 | 4 | HARNESS (and the loader of a never-stepped game): the string applies `laws::apply_to_galaxy` (the printed lanes of the hyperlane tiles from rules version 22) at the start of its steps; the core was loaded from the unstepped galaxy with the lanes off and found no route through a lane (`done_moving` alone) | string | `NativeGame::from_game` loads the galaxy with the laws applied | `native_play::tests::a_core_loaded_from_an_unstepped_game_knows_the_printed_lanes_of_the_hyperlane_map` |
| 7 | 1 | HARNESS: the string takes a step that asks and says nothing (the scoring window of an event nobody scores) ahead of the phase flip where the native loop has a step of its own; the shadow compared the native group (Arborec's Mitosis question) with the empty step | n/a | the catch-up loop of `shadow_play` steps the string again over an empty step (up to 4) | covered by the sweep (the game plays on past step 98) |
| 8 | 1 | The string offers the reactions of an action-phase window in the seating order from the active seat (its resolver's initiative order is fixed at `Game::new`); the core in the live initiative order | neither: S1 | not copied: `docs/STRING_ENGINE_BUGS.md` S1 | -- |

The first differences of the 96-game set by sweep: 35 (int-13) -> 14 (after 1) -> 10 (after 2) -> 8 (after 3) -> 2 (after 4, 6, 7) -> 1 (after 5).

Checks, mutation numbers and what is left: see the sections below (filled in at the end of the lane).

## Checks (cheap), `CARGO_BUILD_JOBS=2`, private target `/root/deploytmp/core-j` (deleted at the end), Haiku runners on committed snapshots

| Check | Result |
|---|---|
| D4 crate (`ti4-engine-core-tests`, profile `lockstep`, debug assertions on for the three core crates) at `91fe3e394` | 146 binaries: **1843 passed**, 0 failed (int-13: 1837; +6 tests of this lane) |
| `ti4-engine-core --lib`, `-factions --lib`, `-rule-bot-typed --lib`, bridge `--test kernel_env --test play_facts --test menu_sources` | ok |
| bridge `--lib` | 2 pins moved: the quick game is **93** native steps (was 90: three follower windows now close in a step of their own) and 60 legacy steps compared (was 57), same fingerprint `29576de02c9e6ff1`; green after the edit (29 `native_play` tests) |
| `ti4-engine --lib` (whole), plain and with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` | 5233 passed, 2 failed both times: `silent4_live_tests::{spy_net_over_the_hand_limit_asks_the_discard_like_the_string, the_holder_of_spy_net_takes_a_card_of_the_yssaril_hand_...}`; their helper counted the card moves the native side left STAGED in the position and the string's announced events; the native turn flow now announces them itself (cause 5), so the helper compares the announced events of both sides and requires nothing staged. Green after the edit |
| `check_layers.py core`, `lint_core.sh`, `clippy_core.sh` (tip) | 568 files 0 violations, 570 files 0 hits, ok (engine 41 = baseline) |
| `native_play.sh --quick --shadow` | 93 native steps, 60 legacy steps compared, NO difference, then the same gap (`PeaceAccords`); `--twice`: same fingerprint |
| `--factions all --players 3,4,5,6 --seeds 1 --shadow` (final binary) | **1 first difference** (the S1 one: saar 6p); 5,837 native steps (int-13: 5,823); gaps as before: Peace Accords 64, status `tokens` close 29 (+1 game is the S1 stop), Umbat 2 |

## Mutation (`tools/engine_core/mutants.sh`, single ranges, committed snapshot `521d14085`, two sets: A the core tests `strategic_arms tactical_composer aftermath_chain turn_jobs turn_jobs2 turn_end_job status_phase`, B the engine lockstep filters `strategic_live tactical_live combat_live turn_live_tests silent4_live_tests status_live`, `D4_COVERS_FILE=/dev/null`)

| range | mutants | A killed | B killed | survivors in both |
|---|---|---|---|---|
| `strategic/mod.rs` 140-155 / 180-190 / 196-200 / 236-250 (cut / held / holds / advance_secondary) | 3 / 8 / 2 / 5 | all | 2 / 6 / 1 / 4 | none (A kills every one) |
| `driver/live.rs` 214-232 (`Status::Done if holds`) | 3 | 3 | 2 | none |
| `tactical/mod.rs` 185-200 (`marks`) and `tactical/live.rs` 36-42 | 0 points | | | -- |
| `tactical/mod.rs` 238-268 (the mirror around the head) | 4 | 2 | 2 | 2 pre-existing, now `// mutants:skip` with reasons: `halted(sx)` after the head's ask (the driver rolls a dry step back), `answered.is_err()` (the head's own question is never refused) |
| `chain.rs` 470-490 | 2 | 1 | 2 | none; A's survivor (`a.nested.is_none()` negated) is now also killed by A: the test checks the queue AT the suspension (typed mirrored, the aftermath's plain log still in the window) |
| `inline.rs` 100-125 | 5 (+2 not compilable under the switch) | 5 | 5 | none |
| engine `live/mod.rs` 268-310 | 4 | -- | 4 | none |
| bridge `native_play.rs` 294-300 (the galaxy) | 1 | -- | 1 (bridge tests) | none |
| bridge `native_play.rs` 788-795 (the catch-up) | 8 | -- | 4 | 4 survivors: the counter and its bound (now `// mutants:skip` behind `catch_up_allowed`) and `||` -> `&&` (a test of the letnev 5p shadow now kills it) |

## What is left

* **S1** (`docs/STRING_ENGINE_BUGS.md`): saar 6p seed 1, the order of the reaction windows of the action phase. Owner: the user (ruling + a `RULES_VERSION` gate in the string or in `sync_clock`).
* Gaps behind the differences (unchanged): Peace Accords (lane E), the status `tokens` close (lane C), Umbat (F2d). The shadow can only compare what plays: once those open, a new sweep will meet new differences behind them.
* Integration 14 merge notes: `Live`/`Out` gained `Out::held` (struct literals of `Out` in other lanes need `held: false`), `Held::holds` (default false), `inline::emit_typed_bare`, `hooks::marks` is `pub(super)`, `SecondaryWin` has two more fields (`cut`, `held`; not in the payload, no serde change). The two native_play pins (93 / (60, 93)) move with the held windows.
