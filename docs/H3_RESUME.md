# D2 lane H3 (`core-h3-leftovers`): the remaining leftovers after lane H2, resume note (2026-11-01)

Branch `core-h3-leftovers`, cut from `core-h2-leftovers` (`5d2b13ce`) and merged with `core-m6d-tactical` (`44fa75a0`, which carries M6c and M6b). Never pushed; the live branch is untouched.
Integration 11 (`core-integration-11`) is NOT finished (no `docs/INTEGRATION11.md`), so this branch is not based on it. Read `docs/H2_RESUME.md`, `docs/M6D_RESUME.md` first.

## The merge of `core-m6d-tactical` (what was reconciled)

* The three gap files (`silent/gaps.rs`, `ability_gaps.rs`, `ability_audit.rs`): took H2's side and deleted M6d's rows by id with `tools/engine_core/delete_gap_rows.py`
  (9 rows: awaken, arboreccommander, muaathero (2), nomadagentmercer, yssarilcommander (2), arborec_flagship, titans_flagship; the Ssruu copy of the Sol agent too, found by the audit test),
  the other rows M6d deleted were already gone in H2 (empyrean, ghost). The `Lane("core-m6c-tactical")` lines of the audit went (this branch has M6c). 29 rows left; `cargo test -p ti4-engine --lib ability_audit` passes.
* The neutral-side bail in `dual_window::settle_run` is absent (only `tables_of_sides`, the barrage gate and table builders mention `is_neutral`).
* `native_subject.rs`: H's generic `open_observed` arms kept (a superset), both openers' tests kept. `tail.rs`: H2's free-action tail (`open_after`) kept, M6c's `resume_strategy` seam deleted.
  `reactions_core.rs`: the PAIRS union, the lobby-factions list union (H's tables plus M6c's), the NAMED line of Mercer deleted (ported). `tactical_live.rs`: M6c's `without_seen` filter kept, H's cards kept (`planet_hub` of H renamed `one_planet_hub`).
  `live/turn.rs`: `discard_law(.., facts, ..)` of M6d, `pub(super)` of H. `combat_fight.rs` had a doubled `show_neutral` field.

## Task 1: the unknown text code (`presentation.phase`), DONE

Root cause: `combat_live::combat_texts` minted the STATE texts (presentation phases, unit names, roll groups) into the position's table (`core.texts.codes`) and the SHOWN texts into the render's table
(`Render::code`), two tables that continue the same lineage and then disagree about every code minted after the split. The product loop (`native_play` step commit: `core.texts.codes = render.codes_snapshot()`)
and the harness hand back the renderer's table: a state code (the phase `pre_roll`) then named a shown text or none (`unknown text code` in the Transient slice when the position is materialised: the
shadow, the dual and the faction sweeps), and a unit-name code could name the wrong unit.
Fix: `combat_texts` syncs the two tables first, mints both kinds through the render, and the position takes the render's table back (`Render::sync_codes`, additive; the same body as lane F2c's `8f53669d`, so the merge is
identical text); `live::live_step_inner` syncs after every `advance` call (F2c's three lines, identical) so a window that mints straight into the state (the exploration's words) is not lost either;
`native_play` commits with `sync_codes` instead of overwriting the position's table. `native_subject` (shadow): the shadow's render now continues the position's table (it was created empty before the position loaded: codes of the observed windows collided with the position's
table). Regression: `combat_live::tests::the_texts_of_a_combat_step_are_one_table_for_the_render_and_the_position` (fails on the old `combat_texts`: the render's table names the phase `pre_roll` by "minted before the step").
The Argent/Nekro rows of the faction sweep pass with it (the sweep fails only on lane G's Naalu Z'eu gap, 41004).

## Task 2: `the_turn_flow_arms_agree_with_the_legacy_game_for_every_lobby_faction`, a branch-composition artefact

Replayed with the staged mark printed (`H3_STAGED` debug print, removed): the staged event is `{"payload":{"amount":3,"player":"c","source":"trade_primary"},"type":"TRADE_GOODS_GAINED"}` at the turn prelude
of the jolnar/l1z1x/arborec/mentak table, round 1: the string's ONE legacy step holds the prelude AND the action menu (answers `strategic|pok5trade`, `done`) and so the Trade primary of the seat that chose it; the core in this tree
has no route from the menu to the strategic action (lane F2b's dispatch), so the core shadow does not stage the goods. Verified on `core-integration-11` (`127d9ec5`): the test passes there (F2b's `432b90ae` chains the prelude into the menu step
and `69da711f` stages the Trade primary's goods). So: not a defect of this tree's rules and not a shadow artefact of the oracle itself, it passes once F2b's dispatch is merged (integration 11 does). Nothing loosened; the test stays red on this branch alone.

## Merge findings that were real defects (each fixed, each guarded)

* **Double ports across lanes G and M6d**: Xuange and Acamar (Empyrean commander and agent) and Sai Seravus (Ghosts commander) were ported twice (G's `silent/{empyrean,ghost}.rs` and M6d's `ce_movement/empyrean/timing.rs`,
  `cc_ground/ghost/timing.rs`); both rules would have fired in one game. M6d's copies (and their NAMED lines, slots 50_060/50_061/50_070/50_071) are deleted, G's stay (they carry the string's registry order).
  Guards: `reactions_core::tests::{every_ported_id_maps_to_its_slot_and_back, the_native_registry_equals_the_string_one_for_the_ported_abilities}` (they failed on the union), the M6d lockstep tests that now run on G's rules
  (`sai_seravus_*`, `acamar_*`, `the_empyrean_commander_returns_its_token_*`).
* **Registry order**: So Ata's end of the look (`yssarilcommander_clear`) sits after Winnu's mech in the `ACTION_COMPLETED` bucket of the string (Yssaril is the last module): `cd_registry::register_abilities` is split into
  `register_early_abilities` / `register_last_abilities`, the table registers the last one after `silent::register_abilities`. The Muaat hero's windows (Adjudicator Ba'al, `ce_movement::muaat::timing`) are registered
  inside `silent::register_abilities` between Ghost's and Muaat's modules (they come after Voidwatch and Acamar in `MOVEMENT_FINISHED`).
* Lockstep scripts changed by the merge (not rules): `tactical_live::scanlink_*` answers Terragenesis' question (identical to integration 11's `c4503924`); `combat_live::nova_seed_*` lost its "stay" (the string now asks one question less before the hero).
  `tactical_composer::the_tail_of_a_heros_free_action_*`: the plain tail opens the Finish job since M6c.

## Task 3: Exploration Probe on the shared exploration window, DONE

* Core: `Play::maybe_explore` starts `Spawn::ExploreFrontier { who, system, named: false }` as a child (stage `Exploring`, `ErasedChild` `explore`, `drive_explore`); deleted: the card's own resolution (about 200 lines: fragments, entropic fields,
  Merchant Station reward child, Pooling stage, gamma, Lost Crew, secret objective, `held`/`gamma`/`explored` fields), `ProbeRow`, `Explored`, `Play::explored`, `Announce::explored`, `Facts::{explore_empty, action_draw_plain}`, the adapter's `close_row` (two call sites)
  and `probe_facts`' text minting. The window declines the play (`ExploreEnd::Unsupported`) as the hero's does.
* NEW `ExploreRun::named` / `Exploring::frontier(.., named)` / `Spawn::ExploreFrontier { named }`: the string's Probe draws with `explore_with(.., FRONTIER, None)`, so its log row names no system and a Mirage has nowhere to act (a frontier exploration of the hero/tail names the system).
  Guard: `exploring::an_unnamed_frontier_exploration_logs_no_system_and_a_mirage_has_nowhere_to_act` (fails on `named` ignored).
* Adapter: `action_cards_view::probe_table` (the play table plus `exploring_dual`'s words/codes/roster and the self-assembly link), `exploring_dual::FP` footprint, the hero's pre-checks (`replenish_window_unported`, `plain_draw`) as bails.
  `dual_window::drive_windowed` keeps the exploration facts when a window follows nested activity (the core table replaces the window's table: `window_driver::keep_exploration`, also in `merge_tables`).
* Tests: the 16 `exploration_probe*` engine tests pass (default areas and `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`: probe 20, exploring_dual 2, action_cards_b6d 64, action_cards_announce 16); four former bail pins are now lockstep tests that run on the core
  (`..._a_card_of_another_deck_runs_in_lockstep`, `..._command_tokens_with_a_mahact_fleet_..._run_in_lockstep`, `..._a_replenish_with_a_trade_agreement_in_play_runs_in_lockstep`, the crowded-hand Lost Crew). Core tests: `action_card_effects3`, `action_card_probe_limit`
  (table with `explore_table()` + children; the merchant snapshot got the words), `action_cards_simple`. Bail kept: a replay reserve, a non-plain draw (Yssaril), a siphoning replenish.
* Mutation (single file, committed snapshot in a scratch worktree, `D4_COVERS_FILE=/dev/null`): `effects.rs` `maybe_explore`/`drive_explore` 5/5 killed by `action_card_effects3` + `action_card_probe_limit`; `explore_windows.rs` `named` lines (mirage condition) 2/2 killed by `exploring`.
  The engine half (`keep_exploration`, `probe_table`) is killed by the 12 lockstep tests that failed without it (NoTexts), not mutated.
* The native path (the action menu playing the card as the action) is lane F2b's route; the table of `live::env` already carries the exploration facts, nothing in this lane touches it. No separate native lockstep of the Probe exists yet (F2b's dispatch is not in this tree).

## Task 5: Crash Landing, DONE (nested; a combat window needs nothing more)

* `Fx::Crash` is nested now: the hit assignment that destroys the last ship (`assign.rs::announce_loss`, `last_ship_destroyed` set before the emission) is the host; the ground forces of the space area are still on the board when the reaction window runs, and the core effect
  moves the chosen one to a planet (coexistence handled). The native tactical lockstep already played the card; what was missing was (a) the dual/registry saying the core plays it (`reactions_core` bail tests used Crash Landing as their sample unported card: now `refit`),
  and (b) the context of the card's own question: the string asks it through `ask_seeing` (the battle and the invasion the position is in), the live path rendered a non-nested play question without them. `live::render_asking` now gives every question of an action card played from a window
  (`Origin::ActionCard`) `nested_position` (the same helper as the nested rendering). This is also the case of hand-over 8a (an event-window card question inside an invasion needs the invasion context).
* Guards: `combat_live::crash_landing_saves_the_ground_force_of_a_lost_last_ship_in_lockstep` (6 seeds, the first option everywhere; failed with `question 7 context space_battle string=true native=<absent>` before the context fix), `reactions_core` (`crashlanding` followed),
  `announce::the_b6d_cards_are_not_played_from_inside_another_window`, `Fx::crash_landing_is_played_from_inside_the_window_of_the_loss_of_a_ship`. Dual: `crash` 5, `hit_assignment` 1, `reactions_core` 14 under `TI4_AREAS=all=dual`.
* Mutation: `Fx::nested` is a `const fn` (the tool lists no mutation point), the unit test pins it.

## Task 8 (hand-overs): one done

* **The combat presentation `complete` written natively where the string writes none** (PDS destroy the active seat's only ship): `chain::fighting` wrote `complete` for every `Opened::NoFight`; the string presents the finished fight only when a side holds the system or a draw was declared.
  Fixed; the filter in the Argent cannon-offense lockstep is gone. Guard: `aftermath_chain::an_undefended_system_presents_the_finished_fight_and_one_nobody_holds_presents_nothing` (fails on the old chain), mutation of `chain.rs` 170-182: 5/5 killed.

## NOT done (with reasons)

* **Task 4 Doctor Sucaban's research offer**: read, not started. The string asks BEFORE the technology (and before the affordability gate) in `paid_research`: for each source (`sucaban_sources`: the readied Jol-Nar agent's holder, then the Ssruu copy, by initiative order from the active seat)
  the holder (not the researcher) answers `yes`/decline, then the RESEARCHER removes infantry one at a time (`trade_infantry_for_research`: option per site `system:planet`, labelled `an infantry on {planet}` / `in space at {system}`, decline ends), the cost falls by one per unit, and an Ssruu copy rolls the whole
  position back when no technology lands (`rollback`). The core `Research` window (`rules/production/strategy_cards.rs`, `begin_paid`) needs two new stages (offer, trade), `Ask::Strategy` variants for both questions, the renderer with the string's prompt texts (they name the researcher / the source owner), the rollback
  (a `Box<CoreState>` like `EmpyreanHero`), `facts::research_asks_more` reduced to Specialist Compounds / Deepwrought, and lockstep against `strategy_research_tests` (which pin `(0, 1)` today). Sizeable: left.
* **Task 6** (Spinner, Duranium Armor repair queues, Devotion + retreat persistence, the Yin hero's scoring hook), **task 7** (ChaosMapping, ImperialArbiter, QuantumDatahub swap; Propagation is G4's, MapEdits M6d's): not started. Notes for the next lane: `round_end_produced` lives in `Machine` only (rebuilt per call in live mode): persist it in `CombatFields`
  (state/aftermath.rs, with the three mirrors in `dual_window.rs` 4203/4315/4883); the Spinner needs `PlaceUnits` scope `PlanetOrShipSpace` (see `docs/M5_RESUME.md` "Not done" 2).
* Task 8 rest: the reaction-card previews on the native path (`reaction_preview::preview_for` reads a `GameState`; only Harness Energy and Infiltrate are native), D's `ground.rs` cause codes for action-card/technology rows (needs a code table per `ActionCardCause` and the staged-ground lockstep),
  the invasion walk question's dice/current display details, the Jol-Nar flag of `combat/env.rs` over every commander unlock path, the cannon-offense `details.routes` with several routes (the one-ring hub of the tests gives a ship one route; a probe with a cruiser from the ring passed), the event-window question inside an invasion
  (done for action-card questions only, see task 5). Not started for budget.
* The four bail-as-failure sweeps that stop at lane G's Naalu agent Z'eu (`AbilityGap(41004)`, `naaluagent-te`): `combat_live::{an_undefended_system_with_a_space_dock..., a_space_fight_of_each_faction...}`, `tactical_live::{a_tactical_action_of_each_faction..., neuroglaive_takes_a_fleet_token...}`. Not loosened.
* F2c identity: `Render::sync_codes` here returns `bool` and syncs only tables of one lineage (a `Render` that does not continue the position's table, as in the harnesses of the older lockstep tests, is left alone: the blind overwrite of F2c's `8f53669d` made some thirty `invasion_live`/`tactical_live`/`combat_live` tests fail with `a core's code table must continue the lineage's`, found by the first whole-lib run). When F2c merges, take THIS body
  (the call sites in `live::live_step_inner` are textually identical).

## Checks (cheap), private target `/root/deploytmp/core-h3` (no incremental; deleted at the end), `CARGO_BUILD_JOBS=2`

* `cargo test -p ti4-engine --lib` (whole, default areas, run twice, the second after the last code change): 5169 passed, 4 failed, 7 ignored; the 4 are exactly the Naalu Z'eu sweeps named above (before the merge repairs and fixes: 3 failures of H2 plus the merge fallout). `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` on the filters `probe`, `exploring_dual`, `action_cards_b6d`, `action_cards_announce`, `crash`, `hit_assignment`, `reactions_core`: all pass. The whole lib under dual was not run.
* `-p ti4-engine-core --lib` 473; `-p ti4-engine-core-factions --lib` 145; `-p ti4-engine-core-bridge --lib` 102 of 103 (the open task-2 test) and `--test kernel_env` 3; core-tests binaries `tactical_composer` 18, `map_edits`, `combat_fight`, `g2_abilities`, `strike_wing`, `silent_abilities`,
  `strategic_arms`, `tactical`, `invasion_composer`, `production_use`, `leader_production`, `aftermath_chain` 18, `exploring` 29, `hero_windows`, `action_card_effects{,2,3}`, `action_card_probe_limit`, `action_cards{,_simple,_announce}`: pass.
* `check_layers.py core`: 530 files, 0 violations. `lint_core.sh`: 532 files, 0 hits.
* Not run (standing rule): golden / wide dual sweeps, benches, soak, the mutation ratchet, census, workspace clippy, `playthrough`, the whole `ti4-engine-core-tests` crate, `native_play.sh` (no zero-bail game exists while the Naalu agent is a gap).
