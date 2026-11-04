# D2-8 lane D resume: the `Game` wrapper and the atomic swap

Branch `core-d28d-wrapper`, cut from `core-integration-18` (`1c11e5662`). Pure refactor: `RULES_VERSION`, serde, fixtures, `host_create.golden`, fingerprints and the generated wire TS are unchanged. Not pushed. Only `RULES_VERSION` 35 is supported (an older game is `GameError::Unmapped` at `try_*` / the first step).

## What is done

### The wrapper (`crates/ti4-engine-next/src/game/{mod,asker,rolls}.rs`, public as `ti4_engine::game::*`)

`Game` is the native loop of the bridge's `NativeGame::step_inner` with the game's `Table` instead of the bot (Architecture A: the core owns the loop, no string engine behind it).

* **Creation**: `new`, `with_seeded_random`, `with_table` (as before) and `try_new`, `try_with_seeded_random`, `try_with_table` (refuse an older rules version / a switched-off feature at creation: `GameError::Unmapped` / `GameError::Disabled(String)`); `check_playable`; `with_galaxy` (frontier tokens, Thunder's Edge Mecatol tile, laws onto the map); `with_sources`; `try_load` (the core is loaded lazily at the first step from `state` + map + random positions, so a preset may still write `state` before the first step).
* **`state` is a mirror** after the load (materialised over the opening position after every step); the core is the authority. A caller must not write to it after the first step (fork instead). `events`, `table`, `rolls()` (rendered from the core's dice ledger; `outcome` is not kept by the core, `reason` for an ability roll is the word "ability"), `steps()`, `galaxy()`, `sources()`, `content()`.
* **`step`**: reconcile hooks, arm, questions to the table through `TableAsker` (the public `Observed` of the position the question is asked in is materialised per question), commit. A failed step restores the core, the code table and the decision log (`GameError::IllegalChoice` carries the decider's own error: `Stop::Asker(IllegalChoice)` in the live loop). A panic inside a step is caught and reported as `GameError::Core`. `run`, `legal_options` (read-only, from `ask_arm_fn` of the arm), `fork`, `fork_keeping_log`, `snapshot`, `snapshot_unlogged`, `GameSnapshot::{instantiate, into_game, decisions}` (FORK RULE: both destructure without `..`), `fork_for_worker` (a `Send` closure over plain data), `install_table`, `rng_positions`, `set_rng_positions`, `open_windows`.
* **Side channels** (plan 2.4): `bind_observation` (randomness = a stream position moved during the step; hidden information = a `private:` mark changed -- an approximation of the string's three call sites, VERIFY against the server undo tests), `on_strategic_action_chosen` (called at the answer that chooses the strategic action, i.e. a little earlier than the string's "after the would-perform window"), `on_aftermath` (the handoff of the movement to its aftermath, with the position), the draft's lone card through `table.auto_resolve` ("only one strategy card left"), `open_secondary` / `secondary_resolutions` (from the core's follower window), `set_participation` (stored, **not enforced**), `product::gap_count()` (every `Stop::Live(Unknown)` is counted; `TI4_GAP_TRACE=1` prints it).
* **Hypotheticals**: `prepare_hypothetical_turn` / `prepare_hypothetical_secondary` reset the turn fields of the position as the string did and load the core again (the secondary opens the core's `Secondary { preview: true }` window).

### The swap (`lib.rs` lane D section)

`ti4_engine::{choice, game, decision_context, dice, event, fingerprint, observation, preview, rng, trigger}` and the root re-exports (`Choice`, `Table`, `Game`, `Roll`, ...) are the thin crate's. The pending list of `surface_paths.py` was exported in the shells (`objectives`, `secrets`, `production`, `tactical`, `timing::Relation`, `transit`, `strategy::SecondaryResolution`, `reach_query`, `secondary_preview`, `valuation::{planet_value, system_value, stranded_troops, ...}` copied into `queries/valuation.rs`). `trigger.rs` is copied (pure). Host, server, wasm, rule-bot, selfplay, policy, sim, training now run on the native `Game`. The call sites that changed: host `history/splice.rs` (`dice()` -> `rolls()`), `planning/{draft,fork}.rs` (one table; `set_participation`), `view/*` tests; training `rollout.rs` (the digest reads `events`).

The legacy engine is reachable from tests/oracles only: `ti4-engine-legacy` is a `[dev-dependencies]` entry of host, server, rule-bot, policy, sim, training (tests and examples that drive a string window). The bridge keeps the legacy game for the oracle/shadow behind `ti4_engine::oracle_adapter::ThinDecider` (a thin decider seated at a legacy table, through JSON) and `legacy_game::legacy_game(spec)` (the native position played by the legacy engine). The legacy crate got one visibility change: `Observed::parts` is `pub` (the adapter builds the thin view of the same position).

### Tests (all in `crates/ti4-engine-core-bridge/tests/wrapper_native.rs`)

* `the_wrapper_plays_the_quick_game_of_the_native_playthrough_to_the_same_fingerprint`: sol 4p seed 1 through `Game` with the rule bot as a table decider: 1165 steps, fingerprint `74d5675ac754ac81`, equal to `NativeGame`'s; zero gaps.
* `a_decider_that_fails_mid_step_leaves_the_position_and_the_journal_and_the_game_goes_on_identically` (regression for the rollback: seen failing with the decision-log truncation removed: `left: (76, 149), right: (75, 149)`).
* `a_fork_keeping_the_log_continues_like_the_original_and_the_worker_fork_is_send`, `the_side_channels_of_the_string_are_fed_by_the_native_loop`, `a_game_below_the_latest_rules_version_is_refused_at_creation`.

## Checks run on the tip (cheap; private target `/root/deploytmp/core-d28d`)

`cargo check --workspace --all-targets` (minus review, replayer, mlp, tensor, advisor) green; `cargo check -p ti4-wasm --target wasm32-unknown-unknown` green; `-p ti4-engine-core-tests --features wrapper --all-targets` green. `cargo test`: thin `ti4-engine` lib 362 + `observed_parity`, `surface_*`, `ties_parity`, `wire_parity` all green; bridge `wrapper_native` 6, `setup_parity` 28, `host_create_golden` (digests unchanged), `ti4-server --lib` 58, `ti4-selfplay --lib` 7 green.

**Red after the swap (unit tests, not run before the swap on this line's natives)**: `ti4-rule-bot --lib` 51 of 190, `ti4-policy --lib` 15 of 287, `ti4-host --lib` 6 of 299 (`planning::draft::tests::{a_draft_ends_where_a_landing_draws_a_card, a_draft_goes_through_activation_and_movement_and_leaves_the_game_as_it_was, a_draft_opens_while_the_strategy_cards_are_chosen, a_draft_goes_on_to_the_production_and_ends_with_the_action}`, `projection::tests::{project_table_view_calculates_objective_progress, project_table_view_names_the_lost_home_planets}`), `ti4-wasm --lib` 1 of 9 (`the_scenarios_run_with_random_deciders`). The rule-bot, policy and projection ones are the known trap of `docs/D28B2_RESUME.md` (lane D item 4): they build their position with the unseated fixture `fixtures::game(..)` (faction `generic`), the native `Observed` accessors answer the neutral value when the typed core cannot load the position (`Cards: faction "generic"`), so a test that expects a planet value, a route or a progress sees zero. The policy test `scored_games_stay_legal_and_deterministic_across_nested_windows` and the wasm scenario test fail with `the position cannot be played: Cards: faction "generic"` from `Game::step` itself. A blanket `real_game` alias in the rule-bot test modules took the rule-bot failures from 51 to 24 but broke 6 tests that passed (the deployed fleets of `real_game` change their boards) and was reverted: each test needs its own fixture (`seated_dealt` / `real_game` plus the pieces it places), module by module. The four `planning::draft` failures are NOT diagnosed (assertion `left == right` at `draft.rs:777/1117/1339/1399`: they exercise `prepare_hypothetical_turn` and the participation filter, both new and the latter not enforced). `ti4-server/tests/it` (73 files) was compiled but not run.

## What remains (honest list)

1. **Fix the red unit tests above** (fixtures per test; diagnose `planning::draft`), then run `ti4-server/tests/it`, `ti4-host`/`ti4-wasm` tests fully and the rule-bot lib again. Anything that read `Game` internals (`timing`, `dice()`, string windows) was moved to the wrapper's accessors or to the legacy crate in tests.
2. `secondary_preview::preview_secondary` is still the string flow on a throwaway position read through JSON (`queries/secondary_preview.rs`, a pure read, not the game's loop). The twin on the core exists in pieces (`prepare_hypothetical_secondary` + a scripted table) and `ti4-server/tests/it/secondary_preview_differential` is its contract.
3. `laws::apply_to_galaxy`, `action_cards::apply_movement_effects`, `movement::{Board, MovementRules}`, `tactical::ship_routes`, `transit::{CargoWindow, rifts_exited}`, `reactions::window_table`, `thunders_edge::breakthrough_for`, `promissory::deal` are still the glob's (legacy) items: pure functions over `GameState` / the map, no loop. They are the host's plan view onto the map (class M of the plan). Port them as natives before D2-9.
4. `Game::set_participation` is stored but not enforced by the core's resolver (a planning fork offers the optional reactions of other seats; the publication gate refuses what it did not audit).
5. Not in the wrapper: turn redo (`set_rng_sync` is stored, `rng_sync_pending` is false; lane E refuses turn redo at creation), `with_round_income` (a nonzero income stops the game with `GameError::Disabled`: `ti4-training` experiment), `Game::timing` (the string resolver), the window-skipped banner (`Table::window_skipped`) and `observed_offer` are fed only where the table is asked (the observer sees the position per question).
6. `Observed` per question materialises the position each time (performance item R2 of the plan: measure first; `Decider::wants_view` exists to skip it for human deciders).
7. Heavy gates (not run by rule): golden/wide dual, shadow Z-gate over the wrapper, benches, recovery tests, `cargo wire-web`, perf A/B.

## Join notes for lane E (product edges) and the later lanes

* `Game::try_*` return `GameError::Disabled` for what `core_sync::unsupported::find` refuses (diplomacy, deck reserve, the dropped factions); lane E's `ti4_host::disabled::check_playable` should call the same function and add the version and Thunder's Edge checks. `lib.rs` lane D section is self-contained (`native` module path attribute, `game` shell, `choice` shell, root `pub use` block, `from_legacy_wire`, `oracle_adapter`, `trigger`).
* D2-9 (strip): `ti4_engine::oracle_adapter` and `from_legacy_wire` are the only production-visible legacy contacts of the thin crate besides the glob shells; both are for tests/oracles.
* D3 (typed bot): `TableAsker` builds `Observed` per question; a typed decider should read the core directly (`Question.core` is the live core). `Stop::Asker` carries the decider's error.


---

# Round 2 (same branch): red tests, side channels, server

## Red unit tests: before -> after

| suite | before | after | remaining |
|---|---:|---:|---|
| `ti4-rule-bot --lib` | 51 of 190 | **1** | `tests::later_drafts_take_imperial_only_with_mecatol_rex_else_the_most_trade_goods` (round 3, no Mecatol, no goods: `picked` is `Some(0)` where it expects `None` on a table of real factions; not diagnosed: the old `generic` table answered neutral, check the legacy string engine on the same position before touching it) |
| `ti4-policy --lib` | 15 of 287 | **2** | `projection::tests::the_public_surface_carries_timing_economy_strategy_and_board_state` (`scoreable-secret` still `None`: the fixture's 4 PDS on a planet / the `eap` secret, the typed read is not diagnosed) and `bot::tests::scored_games_stay_legal_and_deterministic_across_nested_windows` (see "core gap" below) |
| `ti4-host --lib` | 6 of 299 | **0** | |
| `ti4-wasm --lib` | 1 of 9 | **0** | |

Causes found and fixed (each failed before):
* `generic` fixtures: new `fixtures::loadable_game(players)` (real lobby factions, notes dealt, NOT deployed) in the thin crate; used by rule-bot (the shared `game` import of every test module), policy (`watched_hub`, critic, reachability), host projection tests (`seated_dealt`). Fake ids replaced by real ones where the core refuses unknown ids (action cards, technologies `ac2`/`so2`, laws `travel_ban`..., strategy card `pok8imperial`, a real second planet).
* **Scenario defect** (`ti4-host/src/view/scenario/three_seats.rs`): the three-seat scenarios dealt the invented strategy card ids `leadership`/`trade`/`warfare`; the real ids are `pok1leadership`/`pok5trade`/`pok6warfare` (the string engine tolerated the aliases, and left the real cards also unclaimed). The core refuses them; fixed in the scenarios. Guard: `ti4-wasm` `tests::the_scenarios_run_with_random_deciders` (failed with `Players: strategy card "trade"`).
* The four `planning::draft` failures were three wrapper gaps, each now fixed: the hidden-information latch fired on the internal `private:#moved:` mark (now: `cards:reveal:`/`cards:staged:`/insider-peek marks, deck lengths changing, and questions that show cards); the participation filter was not enforced (now: an optional reaction question of a kept-out seat is declined without a question or journal entry, and `Table::window_skipped` is told); the native `load` and `commit` options carried no preview (`CapacityFree`, `GroundForcesOnPlanet`), which the planner's audit requires (`render/tactical.rs`, `render/invasion.rs`). Guards: the four host tests.

## Done in round 2 (items 4 and 5 of the list)

* Participation enforced (above). Hidden-information latch (above); NOT yet checked against the server undo tests (the `it` suite did not finish, see below).
* `window_skipped`: the core resolver now logs the seats a reaction window passed by (`AbilityTable::skipped`, `RulesHost::window_skipped`: a 10-line change in `ti4-engine-core/src/kernel/ability.rs`, requested from the core owner here), the wrapper tells `Table::window_skipped` once per (event, seat, relation) after the step. Guard: the 11 `bluff_reaction` tests of `ti4-server` (they failed with "no hold began" before, pass now).
* Production payment note: a payment question with one option is told with `table.auto_resolve("it was the only way left to pay")` and not journaled (`game/asker.rs`); the quick game fingerprint is unchanged and a test asserts no single-way payment is journaled.
* `GameError::Disabled` also for Thunder's Edge content (`ti4_host::disabled::check_playable` refuses the table switch): `the_wrapper_refuses_what_check_playable_refuses_with_disabled` (failed for TE before). The version check is `RULES_VERSION` equality (the lane E constant `MIN_SUPPORTED_RULES_VERSION` is not on this branch; with lane E merged, use it).
* `rolls()` `outcome` / ability `reason`: NOT done (the core's `RollRec` does not keep them; adding them is a core change).

## Server `tests/it`

Compiled; run once without the `slow_` tests, one named binary (`it-*`), killed after the first hours-long hang. Findings:
* `ti4_server::fixtures::create_sample_game` is a hand-built table with invented ids (kept: the wire fixtures hash it). A running session on it never produced a decision. New `fixtures::create_playable_sample_game` (same seats and economy, real ids, deployed); the session tests that step now use it: `session_deterministic_replay` passes.
* **Open**: `crash_recovery::durable_event_failure_stops_the_session` still hangs (the session starts and offers a decision, so the hang is after the storage fault: probably the worker `return`s on the persist error while `submit_choice` waits). `crash_recovery` also contains tests that were not run. `batch_commit_resume::*` is very slow in the dev profile.
* `batch_after_redo`, `batch_fork_differential` (both `slow_`, turn redo): fail as expected, turn redo is refused by lane E (they should be deleted or ignored when lane E merges).
* The remaining ~60 binaries of `it` were NOT run: that is the first job of the next round (run each module with `timeout 120`, one at a time).

## Core gap found by the wrapper (needs the core owner)

`bot::tests::scored_games_stay_legal_and_deterministic_across_nested_windows` (policy, `ScoredBot`, 6 seats, seed 7793, rotation 0) stops with `the core stopped: aftermath: MissingFact` (`LiveError::Driver(MissingFact)` in the `Aftermath` arm). Reproduce with `cargo test -p ti4-policy --lib scored_games_stay` (about 3 minutes in the dev profile). `product::gap_count()` counts it. Not diagnosed.

## Still legacy (item 3, NOT done; exact names)

`laws::apply_to_galaxy` (also called by `Game::with_galaxy` and after each step), `action_cards::apply_movement_effects`, `movement::{Board, MovementRules}`, `tactical::ship_routes`, `transit::{CargoWindow, rifts_exited}`, `reactions::window_table`, `thunders_edge::breakthrough_for`, `promissory::deal`, and `secondary_preview::preview_secondary` (JSON bridge to the string flow in `queries/secondary_preview.rs`). All are pure functions over `GameState` / the map reached through the facade glob. Port order that costs least: `apply_to_galaxy` (needs `laws::{wormholes_suppressed, wormholes_all_connected, nexus_wormholes_suppressed}`, `faction_techs::spatial_conduit_links`, `factions::hooks_movement::apply_extra_wormholes`, `seating::LOCKED_NEXUS`), then `window_table` (a table of constants), then `breakthrough_for` (delete: Thunder's Edge content is refused), `promissory::deal` (the thin `builders::promissory::deal` already exists: re-export it), then `Board`/`MovementRules`/`ship_routes`/`CargoWindow`, then the preview on the core (`prepare_hypothetical_secondary` + a scripted table).


---

# Round 3 (same branch)

## Fixed (each failed before)

* **`crash_recovery` hang, two causes.** (1) `Game::install_table` dropped the decision log of the table it replaced (the string's carried it over): a resumed session wires its deciders into a forked game and then replays "the rest of the prior decisions" against an empty log, so the worker asked a live human where the replay should have answered, and the submission never got a reply. Guard: `wrapper_native::installing_a_table_keeps_the_decision_log_of_the_game`. (2) A storage fault inside the `on_observed_offer` publishing left the waiting `submit_choice` open for ever: `Lock::fail_waiting` now answers the pending and in-flight submissions with `ValidationFailed` before the error is set (`ti4-server/src/session/worker.rs`). Guard: `crash_recovery::durable_event_failure_stops_the_session`. `crash_recovery` is 5/5.
* **rule-bot (51 -> 0)**: the last one, `later_drafts_take_imperial...`, was Sol's faction plan (Leadership first) now that the seat is a real faction: the test runs with `faction_plans_enabled = false`, which is what the unseated `generic` seat meant.
* **policy (15 -> 1)**: the `the_public_surface...` test used the invented planet alias `mecatol_rex`; the real alias is `mr`. This exposed a **production bug**: the feature `actor-controls-mecatol` of `ti4-policy/src/projection.rs` compared against `mecatol_rex` and so never fired in any real game; it now reads `mr`. The remaining policy test is the core gap below.
* Native ports (equivalence tests in `wrapper_native.rs` against the legacy functions on a played game and on constructed positions): `laws::apply_to_galaxy` (+ `elected`, `wormholes_suppressed`, `wormholes_all_connected`, `nexus_wormholes_suppressed`, `spatial_conduit_links`, `extra_wormholes`; the Ghost flagship test still asks the legacy pure `factions::flagship_has_text` because of the Nekro lend), `promissory::deal` (re-exported from `builders`), `thunders_edge::breakthrough_for`.

## `tests/it` per module (70 modules, one at a time, `timeout 120`, dev profile, `--skip` nothing)

Passing (run, ok): the modules not named below. After round 3: `crash_recovery`, `secondary_draft`, `ready_secondary` (6), `history` (11), `history_session_live` (4), `never_offer_window` (2), `seat_order` (7), `secondary_preview_differential` (2, includes the generated-game differential), `session_deterministic_replay`, `bluff_reaction` (11).

Red or timed out (not fixed), with what is known:
* `splice` (14 of 15): the fixture pins an OLD rules version (`RULES_LETNEV_FLEET_POOL - 1`) and four factionless notes so that recorded decision indices keep their meaning; the core refuses an old version. Needs the indices re-chosen at version 35.
* `batch_after_redo`, `batch_fork_differential` (`slow_`), `turn_redo` (11), `turn_redo_registry` (9), `random_bots::the_host_can_redo_a_bot_seats_turn...`, `fork_equivalence::slow_..forced_rng_positions`, `undo::slow_..`: turn redo (deck reserve, forced rng positions). Refused by lane E (`TURN_REDO_ENABLED = false`); they should assert the refusal when lane E is merged (`GameError::Disabled` for the deck reserve exists). I did not rewrite them here because the refusal lives in lane E's `ti4_host::disabled`.
* `planning::replay_handles_nested_choices...`: the test assumes the string's step granularity (one step ends exactly where the follower is asked). A native step differs: the follower's window is passed inside the step. Two `planning` tests were adapted to loop steps until the question stands; this one still fails (`ScriptDiverged`, the question is b's menu, the secondary was already declined by `AlwaysDecline`).
* `session_recovery_replay::recovery_replay_produces_identical_canonical_hashes_and_event_logs`: `StaleVersion { expected: 2, current: 3 }` on the first submit: the session's `game_version` is one higher than the test expects (an additional pending-decision broadcast); not diagnosed.
* `leadership_batch::a_plan_with_purchases_and_assignments_applies_atomically` (`no entry found for key`), `projection_redaction::secret_objective_progress_reaches_only_the_holding_seat` (own countable secret has no progress), `draft_last_card` (the draft stalled), `stale_offer_state::the_movement_facts_route_on_the_map_the_engine_has` (60 s timeout), `fixtures_verification::ensure_and_verify_fixtures` (a wire fixture differs from the generator: needs `cargo wire-web`), `batch_commit_resume` (recovery replays to a different state: the latent `private:#staged:event:...` mark mismatch lane E also noted), `lobby_all_factions::slow_tricky_factions...`, `history::` none. Not diagnosed.
* Timed out at 120 s in the dev profile (not necessarily failing): `random_seats`, `replay_export`, `smart_bots`, `undo`, `batch_fork_differential`.

## `aftermath: MissingFact` (policy `scored_games_stay...`, seed 7793, 6 seats): still open

Narrowed, not found. The `Unknown::MissingFact` is NOT raised through `CondCx::unknown`, `Eff::unknown`, `Rt::unknown` (stage and ability impls) or the action-card `play.unsupported()` site: I instrumented all five with a backtrace and the failing game printed none. The remaining producers of the value are `rules/reactions/l1z1x_agent.rs::faction_mech` (`Err(Unknown::MissingFact)` when the roster is absent and a faction holds several mech printings), `rules/production/pay.rs:121` and `driver/windows/production/live.rs:88` (both go through `Eff::unknown`, so they are excluded) and the Empyrean/Argent adjacency pairs. The most likely is `faction_mech` reached through `?` from a path that does not call `CondCx::unknown`. Reproduce: `cargo test -p ti4-policy --lib scored_games_stay` (10 minutes in the dev profile); a smaller reproduction needs the game of seed 7793 rotation 0 of `scored_campaign_pairs`. No core file was changed.

## Item 3, what is still legacy (exact names)

`action_cards::apply_movement_effects` (needs the faction rift-effect hook tables of `factions::hooks_movement::apply_rift_effects`), `movement::{Board, MovementRules}`, `tactical::ship_routes`, `transit::{CargoWindow, rifts_exited}`, `reactions::window_table` (a 2000-line table with guard functions over string `Event`s), `secondary_preview::preview_secondary` (JSON bridge). `rolls()` `outcome`/ability `reason`: not added to the core's `RollRec` (touches every roll site).


---

# Round 4 (same branch)

## Root causes found and fixed (each failed before)

* **Observed state at a question was the PRE-answer position.** `TableAsker` built the decider's `Observed` from `asking.at` (the position a stage question was put in, before its answer was applied) instead of the live core; the session published that as `latest_state`, so e.g. the tactic tokens showed 3 where the recovered game had 2 (`batch_commit_resume`, the "private:#staged" suspicion was wrong: nothing to do with marks). Only a nested question's own position (`asking.nested`) is a different position. Fix in `game/asker.rs`. Guard: `batch_commit_resume::a_batch_checked_and_resumed_from_step_copies_leaves_the_same_game_as_full_replays`.
* **The Leadership primary's pool question lacked `details.purchase`** (the string attached it to every pool question of the primary, `gain_tokens_offering(.., announce_purchase)`): `live/details.rs` now adds it for the seat in turn that holds Leadership. Guard: `leadership_batch` (5/5). A seat that holds Leadership and gains tokens by another route in its own turn also gets it (approximation).
* `session_recovery_replay` hard-coded game version 2 for the first question; the native session bumps the version once more (a step with no question precedes the first ask). The test now submits with the version of the `PendingChoice` message it received.
* Fixtures with invented ids or the `generic` faction: `projection_redaction` (playable sample), `draft_last_card` (real factions + notes), `stale_offer_state` (the stored game is an old rules version: the session must refuse it, which is now asserted; re-record the position at version 35 to run the movement checks again).
* `Game::edit_position(|state| ..)` (new): the state is a mirror after the first step, so tests that write to it between steps edit through this call (reloads the core, drops open windows). Used by `planning`.
* **splice fixtures re-chosen at rules version 35**: `setup()` no longer pins version 3 and factionless notes; decision indices re-chosen from the new 400-decision history of seed 7 (token pool 28, mech deployment 63, secret draw 44 "yes", action-card draw 51 "no", the action-card removal test now plays the same game with 51 = "yes"), `kept_later` 371. 9 of 11 non-slow tests pass.

## Tried and reverted: "a step runs on to the next question"

Looping native steps inside `Game::step` until a question is asked fixes the version bump and the granularity of two tests but breaks others (`finish_tactical` then asks one more question than the string's step did, `planning` recorded 5 decisions instead of 4): the string's steps stop at events, not at questions. The wrapper keeps one native step per `step()`; tests that assumed the string's grouping loop their steps (justified in the test comments; `planning::replay_handles_nested_choices...` also asserts the journal is empty after a failed step, the wrapper's whole-step rollback contract).

## `tests/it`, after round 4 (70 modules; turn-redo modules left alone)

Pass: `crash_recovery` (5), `session_recovery_replay` (2), `leadership_batch` (5), `projection_redaction` (15), `draft_last_card`, `stale_offer_state`, `batch_commit_resume`, `planning` (5, last fix in the final commit; re-run it), `secondary_draft`, `ready_secondary` (6), `history` (11), `history_session_live` (4), `never_offer_window` (2), `bluff_reaction` (11), `session_deterministic_replay`, `seat_order` (7), `random_seats` (2, 600 s), `secondary_preview_differential` (2), `undo` non-slow (2), `replay_export::seated_players_get_the_replay...`, and every module not named in this file.

Red, with the reason:
* `splice::{a_single_option_question_that_appears_is_reported, a_single_option_question_that_vanishes_is_reported}`: the native core collapses a single-way payment inside `PayStep` (`rules/production/pay.rs`, the `[only]` branch of `step` and the lone branch of `pay_in`) without any note, so `Table::auto_resolve` is never called: no toast "it was the only way left to pay" and no `Appeared`/`Vanished` alignment. Needs a core hook (a log on `Out` or the tape that the wrapper drains, like `AbilityTable::skipped`). Not done.
* `lobby_all_factions::slow_tricky_factions_play_a_longer_smart_game`: `nekro: the core stopped: aftermath: Unported(Dacxive)`: `driver/windows/invasion/fight.rs::round_unported` deliberately bails when a side owns Dacxive Animators (a Nekro copy). A known core gap for the integration lane.
* Turn redo (left alone): `batch_after_redo`, `batch_fork_differential`, `turn_redo*`, `random_bots::..redo..`, `fork_equivalence::slow_..forced_rng..`.
* Slow (dev profile, about 0.6 s per decision on this game; not a hang): `undo::slow_a_card...` (needs about 10 minutes), `replay_export::{a_lobby_game_exports.., a_game_in_progress..}` and `smart_bots::slow_*`; results of the 1800 s runs are in the final report of the round. The undo latch is exercised by that test (barriers `Revealed`, `OtherSeat`); the two fast undo tests pass.

## Item 3 and the rest, not done (exact names)

`action_cards::apply_movement_effects`, `movement::{Board, MovementRules}` (2 479 lines), `tactical::ship_routes` (the search over `MovementRules`), `transit::{CargoWindow, rifts_exited}` (2 278 lines), `reactions::window_table`, `secondary_preview::preview_secondary`. These are the string movement/transit/reaction searches, not small pure helpers: porting them is re-pointing the host's movement facts (`ti4-host/src/view/tactical.rs`, `planning/`) at the core's `query::reach`/`queries::tactical::{routes_into, typed_rift_exits}` and the cargo/rift readers of `queries::transit`, with a differential test per fact, which is a lane of its own. `rolls()` `outcome`/`reason` not added to `RollRec`. `aftermath: MissingFact` (policy, seed 7793): the `l1z1x_agent::faction_mech` suspect was not tested in a short try; still listed for the integration lane. Wrapper performance (materialise per question, `env_with_plays` per step, two core clones per step): the cause of the slow server modules above.
