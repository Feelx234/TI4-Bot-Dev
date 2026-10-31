# D2 lane H (`core-h-leftovers`): the small leftovers of the lane hand-backs, resume note (2026-10-31)

Branch `core-h-leftovers`, cut from `core-m7-native` (`40081691`), merged with `core-m1-trade` (`a27144d2`, which carries status lane C), `core-d2-c1-agenda` (`c5cd9ac2`, lane D)
and `core-w-l16` (`ba7a543d`). Never pushed; the live branch is untouched. Lanes E (`core-d2-c1-strategic`), F, G and M6c are NOT merged here (see "Not done").

## The merges (what was reconciled, and how)

| conflict | resolution |
|---|---|
| Two `Window::EventScoring` machines: lane C's `ScoringWin<EVENT>` (status + event windows, `Seen`-based offers, the unlock tail of a public score) and M5's `EventScoring` (event windows of a fight, `CoreOffers` over `event_feats`) | Lane C's `driver/windows/scoring.rs` is kept (it is the superset: both timings, the leaders a public score unlocks, `GameFinished`); M5's entry points are shims in it (`event_scoring_window`, `open_event_scoring`, `feat_of`), M5's `advance_event_scoring` is C's `status::advance_event_scoring` plus `chain::sync_initiative`. `aftermath_chain` (M5's core tests) runs unchanged against it. |
| `Reason::Scheming` (M5) vs `Reason::SchemingDiscard` (C) | M5's name; C's tests/windows follow. |
| `COMMANDER_HOOK_MISSING` / `TurnGap::CommanderUnlock` (C) vs lane F2 (all hooks filled) | F2 wins: `turn::start::unlockable(sx, seat, mapless)` returns the list; `TurnGap::CommanderUnlock` is never raised (kept for the dropped factions). `status_phase::a_locked_commander_whose_condition_does_not_hold...` replaced the old gap test. |
| `native_subject.rs`: `close_is_native` / `closes_one_step_early` (C) with `prime` / `staged_marks` / `MenuSources` lending (M7) | both kept; the string-mark check of M7 stays. |
| registry lines, arm-set assertion in `driver/step.rs`, `windows/mod.rs`, `state/mod.rs` exports, `Unknown`/`AbilityTable` fields | union (`NativeArms::ALL` has 21 arms; the registered set lists C's, D's, M1's and M3/M5/M6's). |
| W16 vs M5: `combat_machine.rs` (tests moved), `ability.rs` (`JolNarCommanderReroll` and `ShipFormFlip`, `minister` and `shown_units`), `sustain_offer.rs` doc, docs rows | both sides; `live/plays.rs` stays lane 15/M5's version (W16's copy duplicated `seat_more` / `shared`). The neutral-side bail in `dual_window::settle_run` is absent (checked: no `is_neutral` in `settle_run`; the only ones are the documented per-window ones). |
| Fixes the merge needed in tests | `trade_tcs` (the Production payload has `answered` now), `turn_jobs2` (Hacan's Guild Ships makes every seat a transaction partner: the closing-menu tests drop the map slice, as a lend without a map does) |

## Done

1. **TCS announce sites.** Every site that exhausts an agent tells a listening Nomad: `fx_strategy::exhaust_ready_agent` (new) / `exhaust_agent` is used by the Keleres agent offer (was
   `Unknown::NomadAgentExhaustion`), the Berekar Berekon production option, Jae Mir Kan's waiver (`strategy_secondary`, the adapter's bail is gone and its kernel carries the Nomad hook),
   and Artuno the Betrayer. `Unknown::NomadAgentExhaustion` is deleted (a new site cannot bail silently any more). The agent-offer window (`production_window_dual::agent_offer`) lends and
   applies the staged event. Tests: core `fx_strategy::a_readied_agent_is_exhausted_and_a_listening_nomad_is_told_once`, `strategy_secondary::jae_mir_kan_tells_a_listening_nomad`;
   engine `production::the_keleres_agent_exhaustion_is_staged_for_a_nomad_that_listens_in_lockstep` and `strategy_secondary_dual_tests::jae_mir_kan_while_the_nomad_watches_is_paid_by_the_core_step_and_the_nomad_is_told` (replace the two `bail_*` tests).
2. **The cards whose hosts exist.** Two real defects found by the first lockstep, fixed:
   * the native tactical and invasion drivers built their ability table without the play facts (`env.table.clone()`): a card played inside them was `Unknown::PlayFacts`. Both use
     `live_common::table_at` now;
   * their resolvers had no initiative order: only the active seat saw a reaction window (Decoy Operation of another seat was never offered). Set in `live_common::driver_of` and the two drivers.
   Lockstep tests (string `Game` against `live_step`, bail as failure): `tactical_live::tests::{upgrade_is_played_after_an_activation..., decoy_operation_is_played_by_a_seat_whose_structure..., harness_energy_is_played_after_activating_an_anomaly... (4 anomaly kinds), experimental_battlestation_is_played_after_the_active_player_moves...}`,
   `invasion_live::tests::infiltrate_is_played_when_the_invader_takes_a_planet...`, `agenda_live_tests::bribery_is_played_in_the_vote_like_the_string`; Lie in Wait is M1's `trade_live_tests`.
   The display previews of the reaction question (`context.previews`, the string's `reaction_preview`) are computed natively for Harness Energy (`live::details::harness_preview`) and Infiltrate
   (`live::details::infiltrate_preview`); all other cards' previews are NOT produced by the native path (a general gap, see below). `Fx::nested` is flipped for Upgrade, Decoy, Harness, Infiltrate, Bribery, Lie in Wait and Battlestation.
   The tactical lockstep harness ignores exactly the event-id offset of the string's turn prelude (`TURN_BEGAN`, `FRONTIER_TOKENS_PLACED`).
7. **`open_arm` lines** for `Tactical` and `Aftermath` (the aftermath opens the observed `Invasion` / `Production` / `EventScoring` windows above it); `EventScoring` was C's. Test `the_tactical_and_aftermath_arms_open_the_observed_windows_and_their_sub_windows`.

## Not done (with reasons)

* Crash Landing (no combat window places the unit it saves), Exploration Probe on the exploration window (task 3), M5 leftovers (Spinner, Duranium repair queues, Devotion with a retreat, the Yin hero scoring hook; task 4),
  the turn-flow gaps of lane B (task 5), the hand-overs of task 6, the lane E merge with TE Warfare's free tactical action and Doctor Sucaban (the coordinator's two additions): not reached.
* The previews of every other reaction card are missing on the native path (`reaction_preview::preview_for` reads a `GameState`): a general item for the live adapter.
* The route alternatives of a movement question (`details.routes` for a ship with several equal routes) are not produced natively (found by the Battlestation test with two ships; M6's).

## OPEN FINDING (not resolved): three bridge shadow tests fail after the merge of `core-m1-trade`

`ti4-engine-core-bridge --lib native_subject::tests::{the_turn_flow_arms_agree_with_the_legacy_game_for_every_lobby_faction, the_status_phase_arms_agree_with_the_legacy_game_for_every_lobby_faction,
a_perturbed_native_position_is_reported_as_a_state_difference_at_its_path}` pass on `core-m7-native` and fail from the first merge commit (`1ac1e734`, m1-trade = lane C + M1) on, so the
cause is the interaction of lane C/M1 with the M7 shadow oracle, not the later merges and not lane H's code (checked: the Artuno edit of H1 reverted gives the same failure).
Symptoms: `4x turn_prelude Event native.staged[#]: string "TRADE_GOODS_GAINED" | core <absent>` for the table jolnar/l1z1x/arborec/mentak; at the Tokens step of the status phase
(`oracle::gain_command_token`, table mahact/naaz/nomad/titans) the string stages a `TRADE_GOODS_GAINED` and keeps 3 trade goods for the Nomad while the core leaves 2 and an Artuno pile
(`nomad:artuno:c`). Both are a staged trade-goods event the legacy side holds and the core does not (staging flag / the replaced `closes_one_step_early` and staged-mark handling of
`native_subject.rs`, whose hunks were merged by hand: `prime` and the string-mark lend of M7 are kept, lane C's `PrivateStagedEvent` check is dropped) -- first suspect for the next agent.
The tests were NOT loosened.

## Checks

Run (cheap): `cargo test -p ti4-engine --lib` whole in default areas, 5098 passed, 7 ignored (the whole lib under `TI4_AREAS=all=dual` was started and stopped: far too slow, ~600 tests per 45 minutes
at 2 jobs; the dual form was run on the filters `action_cards reaction lend_table b6d infiltrate harness bribery upgrade decoy battlestation lie_in_wait`: 433 passed, the three tests that named Upgrade as their
unported card were moved to Crash Landing); `-p ti4-engine-core --lib` 475; `-p ti4-engine-core-factions --lib cards`; `ti4-engine-core-tests`: the touched binaries (`fx_strategy`, `turn_jobs2`,
`status_phase`, `trade_tcs`, `aftermath_chain` compile; the whole crate is not run again: disk emergency); `-p ti4-engine-core-bridge --lib native_subject` 97 of 100 (the 3 failures above);
`check_layers.py core` 0 violations; `lint_core.sh` 0 hits.

Regression evidence: the lockstep tests of H2 failed on the pre-fix code with `Live(Unknown(PlayFacts))` (tables without play facts) and with no reaction question for the other seat (no initiative order).
Not done: single-file mutation checks of the new core halves (`exhaust_ready_agent`, `table_at` / initiative in the two drivers, the previews): not run for lack of budget.
Not run (standing rule): golden / wide dual sweeps, benches, soak, mutation ratchet, census, workspace clippy.
