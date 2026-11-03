# Lane S (`core-s-shadow4`): the whole-game shadow differences, round 4, resume note (2026-11-03)

Branch `core-s-shadow4`, cut from `core-r-shadow3` (`92e78d186`). Never pushed; `nightly-fixes-2026-10-26` untouched. Read `docs/R_RESUME.md` (what was left),
`docs/STRING_ENGINE_BUGS.md` (S1 extended once more) and this note. Reproduce one game as in R: `TI4_NATIVE_PLAY_TRACE=1 TI4_NATIVE_PLAY_DUMP=<dir>
tools/engine_core/native_play.sh --factions F --players N --first-seed S --seeds 1 --shadow`. New trace aids: `TI4_NATIVE_PLAY_TRACE=2` prints every question the
legacy follower meets with the answer it wanted and the options (`follower:` lines); `=3` prints the presentation phase of the core and the string after every
native step; a legacy error now prints the offers and events of the steps that ran before it.

## Result

| sweep | games | whole game, no difference | first difference of the core | known string bug (S1, S2) | gap |
|---|---|---|---|---|---|
| `--factions all --players 3,4,5,6 --seeds 1 --shadow` (4 parallel partitions by `--players`, final binary) | 96 | **83** (R: 79) | **0** (R: 3) | 13 (R: 14) | 0 |
| `--first-seed 2 --seeds 3 --shadow` (same) | 288 | **247** | **0** (R: 13) | 40 (R: 32) | 1 (`StrategyAbility`, l1z1x 4p seed 3: lane N) |

(The known-bug counts grew with the games that now play on past an old stop: each of them stops at the first S1/S2 it meets; a game that stopped earlier for a core
difference now reaches one.) The two sets of R are clean of first differences of the core. The Z-gate command itself (`--factions all --players 3,4,5,6 --seeds 8
--shadow`) goes beyond them (the seeds 5 to 8): result in the next section, **NOT met**.

## The Z-gate command (`--seeds 8`): NOT met

Run as four parallel processes, one per `--players` value (the same 768 games; the exit code of the single command is the conjunction), final binary:
**768 games, 761 finished with zero gaps, 7 gaps, 7 first differences, 104 stopped at a known string bug (S1, S2)**. The first run of the command on the first binary of this lane
(seeds 5 to 8 were never swept before) found 13 first differences and 7 gaps; the fixes 8 to 12 of the table took 8 differences away (6 of the 7 yin games: cause 12).

| class | games | what | owner |
|---|---|---|---|
| gap `TurnGap(StrategyAbility)` | l1z1x 4p s3, xxcha 4p s5, jolnar 4p s8 | Jae Mir Kan's waiver | lane N |
| gap `UnportedAftermath(CombatUnported)` | hacan 6p s8, arborec 6p s8, saar 6p s8, arborec 4p s7 | the machine's `run` returns `None` at a site that is no `return None` line (a `?` on an `Option`: `flow_facts`, `start_roll`, ...); hacan 6p s8: a Captain Mendosa question precedes the fight. Not traced (instrument `Machine::run` with `line!()` at every `?`) | next shadow lane |
| staged `STRATEGY_TOKEN_SPENT` of a movement-window ability (naalu 4p s8, ghost 4p s6) | 2 | Foresight spends a strategy token in the window of `MOVEMENT_FINISHED`; the string's step ends at the commit question (`INVASION_BEGAN` is the last event of its step) and announces the token at the start of the NEXT step, i.e. BEFORE the answer to the commit question; the native step asks the commit question and applies the bot's answer in one step, so the token comes after `GROUND_COMMITMENT_FINISHED`. Harness (a native step that holds a question and its answer cannot announce between them) or a reconcile between the question and the answer of such a step | next shadow lane |
| `lockedmallice` production site (yin 4p s8) | 1 | the core offers `82a|lockedmallice` in a production list the string does not (the string's production planets skip the locked Mallice, or the planet is not controlled in the string): not traced | next shadow lane |
| event feats (empyrean 6p s5) | 1 | `players[4].event_feats`: the core records `won_against_a_note_holder`, the string does not. Not traced | next shadow lane |
| nomad 6p s6 | 1 | a question's option `11` against the string's `23` (a system list at the action menu). Not traced | next shadow lane |
| nomad 3p s5 | 1 | a sustain question: the core offers `sustain|1` + `decline`, the string two sustains (`sustain|1`, `sustain|2`). Not traced | next shadow lane |
| yssaril 3p s5 | 1 | the mirror of cause 7 (the string asks the Ssruu copy of seat `a` where the core asks the Mentak seat's Suffi An first): S1 in the legacy-error form (`script wanted "exhaust" but a was offered [leader:mentak:...]`), which `order_in_error` does not recognise because the wanted answer is no ability id | harness: extend `order_in_error` to the ability's own answers |

## Causes, who was right, the guard

"Fails before" = the new test run on a tree with the fixes of the cause taken out (scratch worktree `core-s-pre-wt`, base files or only the one fix reverted): the tests of causes 1 to 6 and
8 to 11 failed there (list at the end); the guard of cause 12 and of the scoring-pause half of cause 8 are the shadow games, seen failing in the sweeps of the binary before the fix.

| # | class (games) | cause | right | fix | guard |
|---|---|---|---|---|---|
| 1 | A, B: a reaction question the core asks that the string never asks, the Ambuscade `[cruiser, decline]` question (xxcha 4p s2, arborec 4p s2, letnev 5p s4, yin 4p s4, sardakk 4p s2, sardakk 4p s1 of the 96 set, winnu 4p s4, yin 6p s3, empyrean 6p s2; also argent 4p s2, cabal 6p s2, xxcha 5p s4 of D) core | **Skilled Retreat did nothing in a live game.** The card's destinations are "the map's part": the adapter of the string noted them (`facts.set_withdraw`); a native game has no adapter, so `skilled_retreat` found no note and returned: the fleet stayed, the combat went on (a barrage, a Scramble Frequency window, an Ambush question) where the string withdrew the fleet and asked "withdraw to which system". All the "extra reaction" and "Ambuscade" differences are downstream of it (the string had no combat left to ask them in) | string | `cards::open_destinations`: with no note and a table that has a play provider (a live game holds the galaxy, as the Argent agent's `destinations`) the destinations are `combat::open_retreat_destinations` over the position's own map (Mahact hero bar, plain adjacency, the movement rules, no other player's ship) | factions lib `cards::tests::skilled_retreat_in_a_live_game_finds_its_destinations_on_the_position_map` (a free neighbour, a neighbour with a ship, a neighbour with only a ground force, a table without a provider does nothing); bridge `the_cards_of_a_combat_round_window_play_in_a_live_game_as_the_string_plays_them` (xxcha 4p s2 step 392) |
| 2 | (winnu 4p s4 after 1) core/engine | the answer to the "withdraw to which system" question was the destination id; `render::combat::parse` turned it into `Opt::Withdraw { system, there: 0 }`, which is no option of a question that lists the ships already there (`there: 3`), and the live step refused it (`aftermath: Illegal`, a defect) | string | `parse` of `Ask::Withdraw` returns `None` like `RetreatTo`: `live::pick_of` finds the option the question lists by its rendered id | engine `live::tests::the_answer_to_a_withdraw_question_is_the_option_the_question_lists`; bridge (winnu 4p s4 step 416) |
| 3 | C: Mallice `82a` (sol 5p s1, ghost 4p s2, yin 4p s4) core | **the core's map never learned a wormhole token the game placed.** `Galaxy::token_wormholes` is rebuilt by the string from the position at the start of every step (`laws::apply_to_galaxy`: the gamma tokens of Gamma Wormhole / Gamma Relay, the Creuss tokens, the open Nexus, the ion storm's face, the Creuss flagship's delta). The adapter baked the tokens present at the load into the printed kinds and nothing updated them in a native game: Signal Jamming's reach lacked `82a` (a gamma token in `74`), a route through a gamma wormhole was not found | string | `CoreGalaxy::{token_wormholes_are, replace_token_wormholes}`; `driver::turn::prelude::sync_wormhole_tokens` in the reconcile job every step starts with (tokens of `explore().wormhole_tokens`, the unlocked Nexus, the ion storm, the hooks' extra wormholes); the adapter now loads the map's own kinds (`Galaxy::map_wormhole_kinds`) and the tokens apart | D4 `turn_jobs::the_wormholes_the_tokens_give_join_the_map_at_the_reconcile_and_leave_with_the_token` (gamma token joins and leaves, the locked and the open nexus, the ion storm turning off); bridge `a_wormhole_token_the_game_places_joins_the_map_of_the_core` |
| 4 | the staged Reflective Shielding hits (sardakk 4p s1 of D) core | `Machine::answer_live` emitted the use of SUSTAIN DAMAGE but never drained `pending_reflective_hits` (only the dual's `step_awaiting` did): the two hits of a Reflective Shielding played in the window stayed on the position and never landed on the producer | string (`emit_sustain_used` drains after every use) | `answer_live`: `reflective_hits`, the producer's losses are tape questions | D4 `combat_machine::the_live_machine_absorbs_the_hits_a_reflective_shielding_staged_when_the_sustain_is_answered`; bridge (sardakk 4p s1 step 919) |
| 5 | D: presentation phase (jolnar 5p s1) core | after the head's space cannon offense paused for a scoring window, the string's `AftermathWindow::new` does not settle the combat window it built (already done: fewer than two sides) and nothing is presented, whether a side still holds the system or not; the core presented `complete` when a side held it (R's rule `winner.is_some()`, measured only on synthetic positions) | string | `AftermathChain::fighting`: no presentation after a head pause | D4 `aftermath_chain::an_empty_system_presents_the_finished_fight_unless_the_head_paused` (renamed; the case `(1 ship, paused)` flipped from `true` to `false`, see the edits) |
| 6 | D: cabal 6p s4 core | the Naalu sheet of the generated data names the Thunder's Edge mech; in a game without that source `printing_in_scope` found no printing and the roster fell back to the plain `mech` (the Stillness of Stars offered `mech`, the string `naalu_mech`) | string | `printing_in_scope`: the oldest printing of the same faction and base type that is in scope (the string names the unit by the id of the faction record) | factions lib `cd_registry::yin::timing::tests::a_sheet_that_names_a_printing_the_box_does_not_hold_gives_the_oldest_one_that_it_holds`; bridge `the_naalu_mech_is_the_printing_of_the_box_and_two_abilities_of_one_event_are_the_known_s1` |
| 7 | D: yssaril 6p s2 harness | two seats react to one event, the core asks them in the ruling's order, the string in its seating order (S1); the string's question of the second seat is the ability's own prompt (Suffi An: `exhaust` / `decline`), not the window's offer, so the harness' same-prompt test did not recognise it | ruling | `s1_other_seat_first` (harness) and `docs/STRING_ENGINE_BUGS.md` S1 | bridge `s1_is_recognised_only_for_a_reaction_and_only_when_the_two_orders_differ` (extended), `the_naalu_mech_is_the_printing_of_the_box_...` (the game itself) |
| 8 | found by the seeds 5 to 8: the scoring step after a pause (sol 6p s8) harness | the retreat token an aftermath step staged is announced by the scoring step that follows a pause (the string's next step starts with the announcement of what is staged); the harness' `after_glue` skipped the reconcile of that step | string | `after_glue` is false when the step left an event-scoring window open | bridge `the_strategy_phase_of_a_later_round_orders_from_the_speaker_and_a_scoring_pause_announces_what_was_staged` |
| 9 | found by the seeds 5 to 8: the strategy phase of a later round (l1z1x 6p s5, arborec 6p s5) core | the flip into a strategy phase of round 2 and later emitted `STRATEGY_PHASE_BEGAN` without telling the resolver the new phase (the status phase's flip does): the windows went out in the initiative order of the status phase (the Naalu "0" card first) instead of from the speaker (LRR 1.20); S2's "the core is right" held only where the initiative order of the moment happened to be the speaker's | core (LRR 1.20) | `Moved::RoundEnded`: `sync_clock` first | the same bridge test (l1z1x 6p s5 step 812) |
| 10 | found by the seeds 5 to 8: the Nekro agent (xxcha 6p s6, arborec 6p s5) core | the discard of Nekro Malleon is a staged card move; the flush of the staged events (`component::flush_staged_events`) announced it after each event, i.e. between the gain and the Pillage of the gain; the string emits the Pillage's gain inside the first event and the card moves follow the outer event | string | `flush_staged_events`: what the windows of an event staged is announced first, then the card moves | bridge `the_nekro_agents_discard_follows_the_gain_and_the_pillage_of_the_gain` |
| 12 | found by the seeds 5 to 8: the removals after a retreat (yin 4p s6, 5p s6/s7/s8, sardakk 4p s8; yin 4p s7) harness | the removals over the capacity after a retreat are native steps of their own, the string's step goes on through them; the native reconcile at the start of the first removal announced the retreat's staged command token, which the string announces at the next step. (A first version of the glue skipped the reconcile between any two questions of an aftermath and broke 4 + 4 games of the two sets: the string DOES announce at the start of the commit step; now it is skipped only up to a removal question.) | string | `after_glue` is true when the next pending question of the aftermath is a `Remove` | bridge `the_removals_after_a_retreat_do_not_announce_what_the_retreat_staged` |
| 11 | found by the seeds 5 to 8: a fight with more than ten unit types (sol 4p s7, letnev 6p s6) core/engine | `combat_live::combat_texts` names no unit type at all when a fight has more than `MAX_TYPES` (10) of them (the roll groups are every subset); a seat that can research in the fight (Reveal Prototype) got the other forms of EVERY base type of its units in the system, ground forces and structures included, which pushed a fight over the limit: the machine stood at the first barrage roller (`CombatUnported`) | n/a (a limit of the core's texts) | the forms are those of its SHIPS only (a ground force or a structure rolls no die in a space combat) | engine `combat_live::tests::a_seat_that_can_research_in_the_fight_names_the_forms_of_its_ships_only` |

Not a cause of the core: **the status-phase window with no active player** (mahact 6p s1, card plays at `STRATEGY_CARDS_WOULD_RETURN`): the core takes the initiative order
(LRR 1.19), the string the seating order from `Game::new`. Kept as it is, reported by the harness as the known S1 (the S1 extension of R is recorded in
`docs/STRING_ENGINE_BUGS.md`).

## Edits of assertions (reasons)

| Where | Edit | Reason |
|---|---|---|
| D4 `aftermath_chain::an_empty_system_presents_the_finished_fight_unless_the_head_paused_and_nobody_holds_the_system` (renamed `..._unless_the_head_paused`) | the case `(1 ship, paused)` expected `presented = true`, now `false`; the message of the last assertion | cause 5: the string never presents after a head pause (jolnar 5p s1); the case was derived from the core, not from the string |
| bridge `s1_is_recognised_only_...` | extended (new cases), nothing removed | cause 7 |

No assertion was loosened.

## What is left, by class (owner)

First differences and gaps of the Z-gate command (`--seeds 8`): see the section below. In short:

* **Gap `UnportedAftermath(CombatUnported)`, hacan 6p s8** (a Hacan seat; a Captain Mendosa question ("raise which of the activating player's ships") precedes the fight):
  the machine's `run` returns `None` at a site that is not one of the `return None` lines (a `?` on an `Option`); not traced further. Owner: the next shadow lane
  (instrument `Machine::run` with the line of the `None`).
* **Gap `TurnGap(StrategyAbility)`** (l1z1x 4p s3, xxcha 4p s5, jolnar 4p s8): lane N's (Jae Mir Kan's waiver).
* known string bugs S1 / S2 are counted apart and left out of the exit code.

## Checks (cheap), private targets `/root/deploytmp/core-s` (native_play), `core-s-t` (tests, Haiku runner and scripts on committed snapshots), `core-s-mt` (mutants); all deleted at the end

| Check | Result |
|---|---|
| D4 crate (`ti4-engine-core-tests`, profile `lockstep`, debug assertions on for the three core crates, `--no-fail-fast`) | 154 binaries: **1963 passed, 0 failed** (R: 1961) |
| `ti4-engine-core --lib`, `ti4-engine-core-factions --lib` | 477, **153** passed (R: 477, 151) |
| `ti4-engine --lib` (plain, the whole suite) | **5289 passed**, 0 failed (R: 5287; +2 new) |
| bridge `--lib native_play` | **54 passed** (R: 48; +6 new, 178 s) |
| `clippy_core.sh`, `check_layers.py core`, `lint_core.sh` | ok (engine 41 = baseline 41); 574 files 0 violations; 576 files 0 hits |
| the two shadow sets, final binary | 0 first differences of the core in both (table at the top) |
| the Z-gate command | not met (section above) |
| new tests on a tree without the fix | `skilled_retreat_in_a_live_game_...`, `a_sheet_that_names_a_printing_...` (factions); `the_wormholes_the_tokens_give_...` (turn_jobs), `the_live_machine_absorbs_...` (combat_machine), `an_empty_system_presents_..._unless_the_head_paused` (aftermath_chain); `the_answer_to_a_withdraw_question_...`, `a_seat_that_can_research_in_the_fight_names_the_forms_of_its_ships_only` (engine); bridge `the_cards_of_a_combat_round_...`, `a_wormhole_token_the_game_places_...`, `the_naalu_mech_is_...`, `the_strategy_phase_of_a_later_round_...` (l1z1x game), `the_nekro_agents_discard_...`: all FAIL there |
| not run (user rule) | `TI4_AREAS=all=dual` whole-lib run, golden / wide dual, benches, soak, census, ratchet, workspace clippy |

## Mutation (`tools/engine_core/mutants.sh`, single ranges, `D4_COVERS_FILE=/dev/null`)

| range | mutants | killed | survivors |
|---|---|---|---|
| `cards/mod.rs` 147-175 (`open_destinations`, the match in `skilled_retreat`) | 9 | 8 (first run 7: the `foreign_units` flag had no test, a ground force of another player now stands on the free neighbour) | `SeatReach::new(.., effects: false)` -> true: `// mutants:skip` with the reason (the string builds its reach without the effects; no position the test can build holds an effect that changes the neighbours) |
| `yin/timing.rs` 75-92 (`printing_in_scope`) | 9 | 9 | none |
| `driver/turn/prelude.rs` 140-175 (`sync_wormhole_tokens`) | 9 | 5 | 4 are `spec_ops` (pre-existing, in the range); the new function: none |
| `galaxy.rs` 266-295 (`token_wormholes_are`, `replace_token_wormholes`) | 10, then 6 | 6 of 10 / 5 of 6 | `token_wormholes_are` -> false (`// mutants:skip`: only saves a clone), the return value of `replace_token_wormholes` (removed); `add_token_wormholes` is pre-existing |
| `combat/machine.rs` 1222-1245 (`answer_live`) | 2 | 0 | the new lines have no mutation point (calls and `?`); the two points are pre-existing `false` literals. The guard is the D4 test and the bridge game (sardakk 4p s1) |
| `windows/chain.rs` 170-190 (NoFight presentation) | 5 | 2 | 3 pre-existing (`parked = true`, `Some((winner, 0))`, the Combat window test); the new condition: none |
| `combat_live.rs` 226-236 (ships only) | 6 | 6 | none |
| `turn/component.rs` 164-190 (`flush_staged_events`) | 8 | 5 | the refusal / halt branches (2 pre-existing lines, now `// mutants:skip` with reasons: a halted step is rolled back) |
| `native_play.rs` 795-815 (`s1_other_seat_first`) | 41, then 14 | 39, then 14 | none after the two assertions added (a question without a seat, the strategy phase) |
| `native_play.rs` 498-506 (`after_glue`) | 11 | 3 | 8: the pre-existing terms of the chain (R recorded two); the lines of this lane (`parked_for_scoring`, `removal_next`) have no mutation point the tool rewrites and are guarded by the bridge games |

## The string's behaviour recorded (not copied)

* `docs/STRING_ENGINE_BUGS.md` S1: the ability's own prompt (yssaril 6p s2), and the status-phase window with no active player (mahact 6p s1) stays recorded as R left it.
* The string names the Naalu mech `naalu_mech` (the id of the faction record) although `ContentStore::resolve_id` resolves it to `naalu_mech_omega` once the Codex source is in
  scope: its own lookups disagree (`unit_type` does not resolve reprints, `resolve_id` does). The core follows the string's choice (cause 6).

## Tip

`git log -1 core-s-shadow4`. Private targets and the scratch worktrees (`core-s-t-wt`, `core-s-pre-wt`) are deleted at the end.
