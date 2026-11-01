# Lane R (`core-r-shadow3`): the whole-game shadow differences, round 3, resume note (2026-11-01)

Branch `core-r-shadow3`, cut from `core-p-shadow2` (`f800ceb2f`). Never pushed; `nightly-fixes-2026-10-26` untouched. Read `docs/P_RESUME.md` (what was left), `docs/STRING_ENGINE_BUGS.md`
(S1 extended, S2 new) and this note. Reproduce one game exactly as in P: `TI4_NATIVE_PLAY_TRACE=1 TI4_NATIVE_PLAY_DUMP=<dir> tools/engine_core/native_play.sh --factions F --players N
--first-seed S --seeds 1 --shadow`. The trace now also prints, per compared group, the script the legacy game was given (`legacy script: [..] + ahead [..]`), what it offered
(`legacy offers`), the staged events at the start and the end of every native step, and, at a legacy error, the next native steps (`the native step after the group`);
`TI4_NATIVE_PLAY_DUMP` writes `native-<step>.json` (the core's position, materialized) next to `base-` and `after-`.

## Result

| sweep | games | whole game, no difference | first difference of the core | known string bug (S1, S2) | gap |
|---|---|---|---|---|---|
| `--factions all --players 3,4,5,6 --seeds 1 --shadow` | 96 | **79** | **3** (P: 27) | 14 (P: 9) | **0** (P: 2) |
| `--first-seed 2 --seeds 3` | 288 | **242** | **13** (P: 69) | 32 (P: 24) | 1 (`StrategyAbility`, lane N) |

The Z-gate (`--seeds 8 --shadow` exits 0) is NOT met: 16 games still show a first difference of the core (classes at the end). The quick game (sol, 4 players, seed 1) plays its whole
game with no difference now (808 legacy steps compared; P stopped at step 448). The game itself changed (the pinned fingerprint moved with the Technology secondary, the marks,
the barrage dice): 1,165 native steps, fingerprint `74d5675ac754ac81`, `--twice` the same (was 1,269 / `24b0d67462f4772b`).

## Causes, who was right, the guard

"Fails before" = seen failing on the code before the fix: the shadow itself (the first difference of the baseline sweep at the named game and step, binary of `f800ceb2f`), or the
D4 test run on the base snapshot (table at the end).

| # | class (games of 96 / of 288) | cause | right | fix | guard |
|---|---|---|---|---|---|
| 1 | OT marks (7 / 14) core | the string registers the Sardakk agent's activation mark and the Winnu's note as ABILITIES of the `SYSTEM_ACTIVATED / after` bucket (`leader:*:sardakkagent_mark`, `ability:*:reclamation_tactical`); the core wrote the two marks from a hook after the activation, so a seat with a second eligible ability in the window (a reaction card) was offered `decline` only, where the string offers `[the card, the mark, decline]`. A pass over the window skips the mark in the string: copied | string | `g3::tro_marks::MARK` (slot 42019) and `winnu::RECLAMATION_TACTICAL` (43008) as mandatory `Spec`s, registered in `activation::register` in the string's module order (saar, sardakk, titans, winnu, yssaril: the registry-order test pins the bucket); `hooks::marks` and its three call sites deleted; the two audit rows (`Carried`) deleted | D4 `g3_abilities::tro_activation_mark::{the_activation_of_any_player_is_remembered_as_a_registered_ability_of_the_agents_owner, with_another_ability_in_the_window_the_mark_is_one_of_the_offers_and_a_decline_skips_it}`, `g2_abilities::the_winnu_note_that_a_tactical_action_began_as_an_ability_of_their_own_activation`; the three tests of `tactical_composer` that wrote the marks through the hook now register the specs in their `env`; bridge `the_activation_marks_of_the_sardakk_agent_and_the_winnu_are_offered_in_the_window_the_string_offers_them` (sardakk 3p step 230, winnu 3p step 500) |
| 2 | TG staged at the activation (2 / 5) core | `Machine::emit` (the typed events of a tactical action) drained the position's staged events before its own event: a trade good Psychoarchaeology staged in the turn prelude came out between the two `SYSTEM_ACTIVATED`. `Live::emit_typed` announces nothing staged; the string announces at the start of its NEXT step | string | the drain deleted (a staged event waits for the reconcile of the next step, as `announce_staged_ground_events`) | D4 `tactical_composer::an_event_staged_before_the_activation_waits_for_the_next_step_and_is_not_announced_by_the_activation`; engine `tactical_live::the_hooks_of_the_activation_that_do_not_ask_run_as_the_string_runs_them` (its native run now does the reconcile every step starts with, `live_reconcile` before every step but the first); bridge `staged_events_wait_for_the_next_step_and_the_agents_extra_die_is_no_barrage_die` (muaat 3p step 514) |
| 3 | Harness Energy (0 / 3) core | the string's Harness Energy ends in the Trade Agreement hook (`promissory::trade_agreement_on_replenish`); the core wrote the commodities only, so the holder's `TRADE_GOODS_GAINED` was never staged (hacan 4p seed 4, naaz 4p seed 4) | string | `begin_harness`: `fx::replenish_announced` with the interned `trade_agreement` source (a table without the exploration texts keeps the plain write; a faction played by two seats declines the run) | D4 `action_card_effects::harness_energy_pays_a_held_trade_agreement_and_stages_the_holders_trade_goods` |
| 4 | the extra die is a combat die (2 / 2) core | the Letnev agent's mark (a Ssruu round-agent mark) was added to EVERY roll of the dice seam; the string reads it at the space combat and ground combat rolls only. A destroyer the agent named barraged with a third die (muaat 3p step 550, ghost 4p step 548) | string | `adapters.rs`: the agent term only for `RollKind::Combat | Ground` | D4 `combat_barrage::the_extra_die_of_a_round_agent_is_not_a_barrage_die` |
| 5 | hit flush (3 / 5) core | the invasion's hit batch flushed the position's WHOLE staged list; the string's flush is a log of the batch (sustained / destroyed ground forces) and every other staged event waits for the next step: a retreat's `COMMAND_TOKEN_PLACED` came out after `INVASION_BEGAN` | string | `kernel::hits::flush_staged` takes the two ground-force kinds out of the list and leaves the rest in order | D4 `invasion_windows::the_flush_of_a_hit_batch_announces_the_hit_events_only_and_leaves_the_other_staged_events_for_the_next_step`; bridge (jolnar 5p steps 369, 371) |
| 6 | Technology follower (3 / 3) core | the follower gate asked `production::available` (planets by their larger face, the Triad, War Machine's +5 of the activation); the string gates on `payment::affordable` (planets and trade goods). A seat that had played War Machine in the activation was asked to follow (sol 4p step 448, arborec 4p, cabal 4p) | string | `KernelEnv::technology_secondary_affordable`: the printed resources of the spendable planets plus the trade goods at their worth | D4 `strategic_arms::war_machine_does_not_pay_for_the_technology_secondary`; bridge `the_shadow_run_agrees_with_the_legacy_game_for_the_whole_quick_game` |
| 7 | S1 outside the action phase (2 / 8) harness | the harness recognised S1 for ids that start `reaction:`, `leader:`, `ability:`; a Technology offer (`technology:xxcha:it:..`) was not recognised, and a window with nobody active (the status phase) was never compared with the ruling | ruling / LRR 1.19 | `is_reaction_id` takes every ability prefix; `s1_orders_differ`: status phase, nobody active: the string's list is the seating order of `Game::new`, the ruling's the initiative order. **Open question for the user** (end of this note) | bridge `s1_is_recognised_only_for_a_reaction_and_only_when_the_two_orders_differ` (extended) |
| 8 | S2, the flip into a strategy phase (0 / 8) string | `STRATEGY_PHASE_BEGAN` of round 2 and later (and a card played in it) is opened before the string syncs its timing context: ordered like the phase before, not from the speaker | core (LRR 1.20) | not copied: `docs/STRING_ENGINE_BUGS.md` S2; the harness reports it as `known string bug S2` (`s2_orders_differ`, only in a group with the `phase` arm) | bridge `s2_is_recognised_for_a_reaction_in_a_phase_flip_only_when_the_speaker_is_not_the_first_seat` |
| 9 | one string step through many native steps (5 / 5) harness | every removal over the fleet supply or the capacity is a native step of its own, and the string's step goes on through the rest of the action (and, in a Warfare action, to its second redistribution): the legacy game was given the answers of 4 native steps ahead and ran dry (`script wanted "" but ..`); a dry run cannot be repeated (the string's `finish_tactical` restores everything but `secondary_after_tactical`) | n/a | `ahead_for`: 12 native steps for a group that ends in a tactical or aftermath step, 4 otherwise | bridge `a_string_step_that_runs_through_the_removals_and_the_warfare_question_is_given_the_answers_ahead` (letnev 3p step 684, 4p step 289) |
| 10 | the string stops after `RETREAT_STEP_STARTED` (3 / 5) harness | the string's step ends after the events that open the retreat and asks the announcement in the next step; the native step asks it behind the events at once | string | `events_lead`: a legacy step that asked nothing and pushed the head of what the group pushed is stepped again with the same answers, the events owed to the group | bridge `a_legacy_step_that_asked_nothing_and_pushed_the_head_of_the_groups_events_stopped_early` (unit), jolnar 5p / nekro 6p |
| 11 | staged events behind a hand-off, behind a left-over end job (5 / 7) harness | (a) a Ceasefire denies the movement: the string's step runs from the activation through the aftermath to the closing menu; the reconcile of the native aftermath step announced the activation's staged trade good ahead of the owed `PRODUCTION_RESOLVED`; (b) an end of turn a hook left open (Nullification Field) runs INSTEAD of the reconcile in `advance_job`: the staged `STRATEGY_TOKEN_SPENT` was never announced at the next turn's start | string | `after_glue` also after a finished tactical step that handed off to its aftermath; the reconcile loop runs again after a left-over job | bridge (yssaril 4p seed 3, empyrean 4p seed 3, naaz 6p seed 2; cabal 3p, winnu 6p seed 2: sweep) |

No cause is a rule change; the game of every table changes only where the string's behaviour was copied (1-6).

## Edits of assertions (reasons)

| Where | Edit | Reason |
|---|---|---|
| bridge `a_native_game_plays_a_whole_game_with_zero_gaps` | steps 1269 -> 1165, fingerprint `24b0d67462f4772b` -> `74d5675ac754ac81` | causes 1, 2, 4, 6 change the game |
| bridge `the_shadow_run_agrees_with_the_legacy_game_up_to_the_known_follower_window_of_the_technology_card` | replaced by `..._for_the_whole_quick_game` (808 steps, no difference) | cause 6; the pin said it would fail the day the follower window was fixed |
| bridge `s1_is_recognised_only_for_a_reaction_and_only_when_the_two_orders_differ` | `state.phase = Action` first; new cases (technology id, status phase) | S1 is now phase-aware (cause 7); the strategy phase never differs |
| D4 `tactical_composer` `env` | registers the two mark specs and their registry rows | cause 1: the marks are table abilities |
| engine `ability_audit_g3_tests::G3_EMITTERS` | + `SYSTEM_ACTIVATED` | slot 42019 listens to it |
| engine `tactical_live::native_run` | `live_reconcile` before every step but the first | cause 2: the string announces staged events at the start of a step; the reconcile is what every production step starts with |
| engine `reactions_core.rs`, `ability_audit.rs` | the two `Carried` rows of the marks deleted | cause 1: the table has the rules |

No assertion was loosened.

## Checks (cheap), private targets `/root/deploytmp/core-r` (deleted at the end), CARGO_BUILD_JOBS=2, one cargo run at a time

| Check | Result |
|---|---|
| D4 crate (`ti4-engine-core-tests`, profile `lockstep`, debug assertions on for the three core crates, `--no-fail-fast`) at the tree of `09ddfe177` (a clean target `/root/deploytmp/core-r-t2`) | 154 binaries: **1961 passed, 0 failed** (P: 1952; +8 tests of this lane, +1 from the registration tables) |
| `ti4-engine-core --lib`, `ti4-engine-core-factions --lib` | 477, 151 passed |
| `ti4-engine --lib` (plain) at the tip | **5287 passed**, 0 failed, 7 ignored (the first run found four failures: the registration order of the marks in the `SYSTEM_ACTIVATED` bucket (twice), `G3_EMITTERS` without the event, and the lockstep of `tactical_live` without the reconcile every step starts with; all four fixed, see the tables above) |
| bridge `--lib native_play` | **48 passed** (85 s) |
| `clippy_core.sh`, `check_layers.py core`, `lint_core.sh` | ok (engine 41 = baseline 41, core crates 0 warnings); 574 files 0 violations; 576 files 0 hits |
| `native_play.sh --quick --twice` | 1,165 native steps, fingerprint `74d5675ac754ac81`, the second run the same |
| `--factions all --players 3,4,5,6 --seeds 1 --shadow` (final binary) | 96 games: 96 finished with zero gaps, **3** first differences (sol 5p, jolnar 5p, sardakk 4p), **14** known string bugs (S1, S2), 79 games without a difference |
| `--first-seed 2 --seeds 3 --shadow` (final binary) | 288 games: 287 finished with zero gaps, 1 gap (lane N), **13** first differences, **32** known string bugs, 242 games without a difference |
| new D4 tests on the BASE snapshot (`f800ceb2f`, own target `/root/deploytmp/core-r-base-target`, the test files of the tip, the registration lines of the mark specs removed) | **8 of 8 fail** (`harness_energy_pays_a_held_trade_agreement_and_stages_the_holders_trade_goods`, `the_extra_die_of_a_round_agent_is_not_a_barrage_die`, `the_winnu_note_that_a_tactical_action_began_as_an_ability_of_their_own_activation`, `tro_activation_mark::{the_activation_of_any_player_is_remembered..., with_another_ability_in_the_window_the_mark_is_one_of_the_offers...}`, `the_flush_of_a_hit_batch_announces_the_hit_events_only_...`, `war_machine_does_not_pay_for_the_technology_secondary`, `an_event_staged_before_the_activation_waits_for_the_next_step_...`); the three `tactical_composer` mark tests pass there (the hook wrote the marks). The bridge shadow tests fail on the base by construction: the baseline sweep shows each first difference at the step the test runs past (sardakk 3p 230, winnu 3p 500, muaat 3p 514 and 550, jolnar 5p 369 and 371, letnev 3p 684, 4p 289, yssaril 4p seed 3 874, cabal 3p 766, sol 4p 448) |
| not run (user rule) | the `TI4_AREAS=all=dual` whole-lib run, golden / wide dual, benches, soak, census, ratchet, workspace clippy |

## Mutation (`tools/engine_core/mutants.sh`, single files / ranges, `D4_COVERS_FILE=/dev/null`)

Set A = the D4 tests (`combat_barrage invasion_windows strategic_arms g2_abilities g3_abilities action_card_effects tactical_composer`, `TEST_CRATE=ti4-engine-core-tests`), set B = the bridge `native_play` tests
(`TEST_CRATE=ti4-engine-core-bridge`, the filter names the tests of the part). The engine lockstep filters were run as plain `ti4-engine --lib`: 5287 passed.

| range | mutants | killed | survivors |
|---|---|---|---|
| `g3/tro_marks.rs` 20-70 (the mark: condition, effect, registration; A) | 12 | 12 | none |
| `silent/winnu.rs` 84-100 (the note; A) | 7 | 7 | none |
| `kernel/hits.rs` 60-80 (`flush_staged`; A) | 1 | 1 | none (the tool does not rewrite `matches!`; the test pins both kinds and the staged rest) |
| `rules/tactical/mod.rs` 204-214 (`Machine::emit`; A) | 1 | 1 | none |
| `seam/adapters.rs` 76-96 (the agent term; A) | 6 | 2 | 4 pre-existing: the Bastion term's `0` literals (78, 80) and the `to == acc` of the planet-spend seam (92). First run: 1 of 6 (the `bastion + agent` sum and the kind gate had no positive test; `the_extra_die_of_a_round_agent_is_not_a_barrage_die` now also asserts the combat roll has the die and the other kinds not) |
| `driver/kernel_env.rs` 112-135 (`technology_secondary_affordable`; A) | 16 | 4 | 12 pre-existing (`has_specialty_research_planet`, `secret_hand_all_scored`, `can_produce_in`); the points of the new function are all killed. First run: 5 of 18, the survivor of the new function was the unreachable `map_or` default of a seat that is not seated (now `// mutants:skip` with the reason) |
| `action_cards/effects.rs` 2046-2090 (`begin_play`'s Harness arm, `begin_harness`; A) | 14 | 13 | 1 pre-existing (`>=` of Plagiarize's rules version). First run: 11 of 14 (the decline of a faction played by two seats and the plain write of a table without exploration texts had no test) |
| `native_play.rs` 750-830 (S1 / S2 recognition; B) | 38 | 38 | none |
| `native_play.rs` 990-1050 (`events_lead`, `ahead_for`, `steps_idle`, `Owed`; B) | 16 | 14 | 2 pre-existing (`steps_idle` body, `Owed::settle`'s `0`; their tests are P's and were not in this run's filter) |
| `native_play.rs` 425-445 (the reconcile after a left-over job; B) | 2 | 2 | none |
| `native_play.rs` 495-503 (`handed_off`, the glue chain; B) | 12 | 10 | 2: `&&` -> `||` in `handed_off` (equal on every position the tests reach: a finished non-tactical, non-aftermath step with an aftermath window open), `==` -> `!=` of the pre-existing chain term |

The first mutation run of the harness ranges (all of `native_play.rs` 750-830 with the whole `native_play` filter, 16 mutants run before the run was stopped for its run time) killed 16 of 16.

## What is left, by class (owner: the next shadow lane unless stated)

First differences of the 96-game set (3) and of the 288-game set (13) that are not S1 / S2:

* **Scramble-Frequency-like reaction offered by the core only (about 7 in the 288 set: arborec 4p seed 2, xxcha 4p seed 2, letnev 5p seed 4, yin 4p seed 4, sardakk 4p seed 2; and
  sardakk 4p seed 1 of the 96 set at `ACTION_CARD_DISCARDED:after`)**: `legacy script wanted "decline" but d was offered ["stay", "retreat"]`: the native asks a
  non-combatant (xxcha 4p seed 2: seat `d`, Arborec, holds `scramble`, the roller is the Jol-Nar `b` with no hit) a `reaction:..:UNIT_ABILITY_ROLLED:after` question that the string
  never asks. The string's window is guarded by `actor_is_not` only; the cause is not traced (a hand-filter of the string's slot registration, or `reaction_slot::playable`).
* **Ambuscade / `[cruiser, decline]` question (3: winnu 4p seed 4, yin 6p seed 3, empyrean 6p seed 2)**: the string asks which cruiser or destroyer rolls an additional die
  (`UNIT_ABILITY_ROLLED`) where the native does not (or in another order). Not traced.
* **Mallice / `82a` (2: sol 5p seed 1 in the 96 set: Signal Jamming's system list lacks `82a`; ghost 4p seed 2: `move|82a|0`)**: the core's galaxy (`core.galaxy().adjacent`) lacks the
  wormhole adjacency of the Mallice tile that the string's effective galaxy has. Not traced.
* **Presentation phase (1: jolnar 5p seed 1, step 592)**: `combat_presentation.phase` is `complete` in the core and `resolving_hits` in the string after a combat that an event-scoring
  pause interrupted (the string leaves the phase of the last loop of `settle_stages`). Cosmetic, not traced further.
* **Card-play window (1: xxcha 5p seed 4)**: `when ACTION_CARD_PLAYED` asked of a Mentak seat by the native after an event-scoring pause; the string never asks.
* **Single games**: yssaril 6p seed 2 (Ssruu's copy of the Mentak agent offered before the payment `exhaust|..`), argent 4p seed 2 (a `decline` of the core against the string's
  menu), cabal 6p seed 2 (`stay` against a system choice), cabal 6p seed 4 (the Naalu mech's label `mech` / `naalu_mech` in a production list).
* The gap of the 288 set (`TurnGap::StrategyAbility`, mahact 3p seed 67 among them) is lane N's (Jae Mir Kan's waiver).

### Open question for the user (S1 for a window with no active player)

The ruling of 2026-11-01 (the ACTIVE player first, then the others in initiative order) covers the action phase. In the status phase nobody is active (card plays in
`STRATEGY_CARDS_WOULD_RETURN`, mahact 6p seed 1). The core offers such a window in the initiative order of the moment (LRR 1.19: every phase but strategy and agenda orders by
initiative); the string offers it in the seating order of `Game::new`. The harness reports these as S1. If the rule you want for a window without an active player is the
speaker's clockwise order, the core's `Resolver::player_order` changes for the status phase (one line) and S1 loses these cases.

## Tip

`git log -1 core-r-shadow3`. Private target `/root/deploytmp/core-r` and its logs are deleted at the end.
