# core-integration-13: merge record (2026-11-01)

Branch `core-integration-13`, cut from `core-integration-12` @ `fea5991b1`. Three merges in the ordered list (G4, H4 with H3, F2d's dispatch3), cheap checks only (the user rule: heavy work waits for a
zero-bail playthrough). Not pushed. The live branch `nightly-fixes-2026-10-26` was not touched. NOT merged, by order (running, they come in integration 14): `core-g5-abilities`, `core-d2-c3-dispatch4`,
`core-h5-leftovers`.
Pure refactor: `RULES_VERSION`, serde, fixtures and fingerprints are unchanged (no file of those was edited by a resolution; the one new state field is a core-only payload field of the aftermath window,
`Aftermath::head_paused`, like H4's `CombatFields::round_end_produced`: the string keeps nothing for it).

## 1. Merges

| # | Branch | Tip merged | Merge commit | What it brings | Files in conflict |
|---|---|---|---|---|---|
| 1 | `core-g4-abilities` | `d0c218b22` | `2938dd916` | lane G4 (it contains its own redo of the G2+G3 merge, G2 slots `43_000..`, G4 slots `44_000..`, the numbers integration 12 chose): Propagation, Tungstantus, Spy Net, Stymie, Foresight, the Magen Defense Grid hit, Yin Spinner Omega, the Naalu TE mech (all nine loud rows), `silent4` questions and renderer, `may_view_hand`, `secondary_waived`, Riftmeld providers; **Z'eu comes in with it** (G3's) | 11 |
| 2 | `core-h4-leftovers` | `61c654d86` (contains `core-h3-leftovers`) | `d3e09d7fa` | H3: one code table for the render and the state (`Render::sync_codes`, checked), Exploration Probe on the shared exploration window, Crash Landing nested, duplicates of G and M6d removed; H4: Doctor Sucaban (and Ssruu's copy) in the core Research window, the round-end guard as a field of the combat window, reaction-card previews on the native path, the exploration's display facts, the Jol-Nar flag of the fight, cause texts of every staged ground force | 17 |
| 3 | `core-d2-c3-dispatch3` | `8b212698a` | `6ca6f8fc6` | F2d: Nekro Malleon, the hero routes, Ssruu copies, Sling Relay, relics (Dynamis Core, Book of Latvinia, Codex), the Muaat components, Production Biomes, the pacts and notes, the staged events of a component action announced at once | 0 (clean) |

Later commits are the fixes the checks found (section 3). The neutral-side bail of `dual_window::settle_run` was checked after EVERY merge
(`awk '/^fn settle_run/,/^}/' crates/ti4-engine/src/dual_window.rs | grep -ci neutral`): **0 each time**. `cargo check --workspace --all-targets` (without ti4-review, ti4-replayer, ti4-mlp, ti4-tensor, ti4-advisor)
was green after every merge (the first check after merge 1 failed to compile once: `TurnGap::Propagation` removed by G4, B3).

## 2. Conflicts and how they were resolved (keep both sides)

| File | Conflict | Resolution |
|---|---|---|
| **gap rows**: `silent/gaps.rs`, `ability_gaps.rs`, `ability_audit.rs` (merges 1, 2) | positional rows against G4's 13, H4's 29 | HEAD's three files (integration 12 had 9 rows), then `tools/engine_core/delete_gap_rows.py` with the nine ids G4 ported (Foresight, Propagation, Titans commander, Spy Net, Stymie, `asn_clear`, `md_hit`, `yso`, Naalu's TE mech): **0 rows left**. The ids G4 and H4 deleted beyond those were already gone. `asn_clear` is `Dropped` in the audit (the Agency Supply Network is a Keleres technology, G4's entry). The helper predicates of `gaps.rs` (`faction`, `leader`, `tech`, `note`, `downstream`) and `Status::Gap` carry `allow(dead_code, reason)` for the next lane that lists a gap; the two helpers HEAD still had for rows that are gone (`unit_in_play`, `foresight_possible`) are deleted. Merge 2 took HEAD's gap files for the same reason (H4's are the older, longer lists). The audit test is green with no unlisted gap |
| `tools/engine_core/delete_gap_rows.py` (1) | HEAD's tolerant `SUFFI_AN*` handling against G4's | HEAD's |
| `silent/mod.rs`, `choice/{mod,silent}.rs`, `render/{mod,silent}.rs` (1) | G4's `silent4` against H2's `activation`, `ExtraDie`, `StrikeWing`; G3's `T(..)` variants twice | all variants once: `Ask/Opt/Subtype::{T, U}` beside `ExtraDie`, `StrikeWing`, `Infantry`; both modules; the renderer arms once |
| `combat_live.rs` (1), `reactions_core.rs` PAIRS (1, 2) | the includes of the G3 and G4 test files; the Ssruu pairs | both includes; HEAD's pairs (the same set of entries) |
| `silent/mod.rs` (2) | H3 registers the Muaat hero's windows inside `silent::register_abilities` | HEAD's order (the hero is registered by `ce_movement::register_late_abilities`, after `register_before_muaat`); see B3 for what the clean textual merge dropped |
| `native_subject.rs` (2) | H3's shadow render continues the position's code table (built after the position loads); F2b's faction texts on the render | both: `Render::new(..).with_factions(..).with_codes(..)` after the load; the opener test once |
| `render/mod.rs` `Render::sync_codes` (2) | F2c's blind overwrite (already the checked body on dispatch3) against H3's `extends`-based body that returns `bool` | **H3's checked body** (only syncs when one table is a prefix of the other); its callers (`native_play`, `combat_live`) use the `bool`; dispatch3's `sync_codes_tests` merged cleanly on it |
| `live/mod.rs` (2) | which question position wins for display facts | HEAD's: the innermost nested position, then the question's (F2b found an emission nested in a job); H3's side is the older form |
| `tactical_live.rs` (2) | H3's copies of the Upgrade / Decoy Operation tests and `one_planet_hub` against HEAD's `planet_hub` / `centre_hub` | HEAD's tests and helpers; `one_planet_hub_for_lockstep` (used by H3's combat test) is a wrapper of `planet_hub`; HEAD's `panic!` on an unexpected native error and the exact `event_id` offset (integration 12) stay, H3's blanket `trigger.event_id` filter is not taken |
| core tests `aftermath_chain`, `strategic_arms`, `tactical_composer`, `driver_windows` (2) | both sides add a test at one place; H4 removed the `research_asks_more` tests (the function is gone) | both tests (HEAD's mirror test and H3's presentation test); the Hecatoncheires test stays, the two `research_asks_more` tests go; one copy of the free-action tail tests; `driver_windows` already had the fixture H4 fixed |
| `live/agenda.rs` (2) | H4's cause-text test module | H4's |

## 3. Bugs and breaks found, and how each is guarded

"Fails before": R = seen to fail on the code before the fix, B = by construction.

| # | Where | What | Fix | Guard |
|---|---|---|---|---|
| B1 | slots (merge 1) | **two lanes' rules in one slot, again.** H2's Strike Wing rules (`SWA2`, `AMBUSCADE`, `COMMANDER`) are `44_001..44_003`, and G4's Tungstantus, Spy Net, Stymie are `44_001..44_003` too (`AbilityTable::add`'s duplicate check is a `debug_assert`: the lockstep profile only lost three string abilities, the Argent commander, Tungstantus and Spy Net) | H2's move to `46_001..46_003` (G4 and its successor G5 keep `44_`; the next free range for a G lane is `45_`) | `reactions_core::tests::no_two_rules_of_the_core_table_share_a_slot` (R: `slot 44003 is registered twice: (Promissory, ShipMoved) and (Leader, UnitAbilityRolled)`), `ability_audit` (R: the three ids), the two Spy Net lockstep tests (R: `every scripted answer was taken`) |
| B2 | registry order (merge 1) | Stymie (Arborec, `SHIP_MOVED`) was registered with the rest of `silent` (after the Empyrean and Ghost modules); the string's Arborec module comes first | `silent::g4::register_first` moved into `register_before_muaat` | `reactions_core::tests::{the_native_registry_equals_the_string_one_for_the_ported_abilities, the_registry_of_the_g3_rules_equals_the_string_one}` (R: "the order of the registry differs at 76 / 75"); `g4_abilities`' table calls it too |
| B3 | **a registration dropped by a clean textual merge** (merge 2) | H3 moved `muaat::timing::register_abilities` (the Muaat hero's windows, the war sun record) out of `ce_movement::register_late_abilities` into `silent::register_abilities`; resolving `silent/mod.rs` with HEAD's side (where `silent` no longer has it) left it registered nowhere: the string abilities `muaathero:MOVEMENT_FINISHED` and `muaathero_moved:SHIP_MOVED` had no core rule | restored in `ce_movement::register_late_abilities` (the string's order, after `register_before_muaat`) | `ability_audit::every_ability_the_string_registers_is_ported_or_accounted_for` (R: the two ids), `reactions_core::every_ported_id_maps_to_its_slot_and_back` (R: "a named slot is in the table"), `combat_live::nova_seed_replaces_the_tile_in_lockstep` (R: `Illegal` at the hero's reaction) |
| B4 | compile (merge 1) | `strategic_live_tests` excused `TurnGap::Propagation`, the variant G4 removed | the arm is deleted | the compile |
| B5 | Carth's branch question (dispatch3 on the tree with G/H4's reactions) | the native question of Carth of Golden Sands carried no `details.commodities` (the string's `.detailed("commodities", ..)`); dispatch3's lockstep did not compare the question because the Cabal agent's reaction after the replenish was not native yet (its note: "the native step does not open it yet") | `live/details.rs` fills it from `commodity_standing` | `action_dispatch_live_tests::carth_replenishing_a_player_at_a_cabal_table_announces_it_before_the_ability_resolves_like_the_string` is a full lockstep now (answers `component|leader|hacanagent`, `b`, `decline`; `assert_same`). R: with the reaction native the old script ended `Exhausted`, then `assert_same` failed on `details` |
| B6 | the finished fight (merge 2, **found by the whole-game shadow**, `--factions all --players 3,4,5,6 --seeds 1 --shadow`: 35 of the first 47 differences) | H3 stopped writing the presentation phase `complete` for a system nobody holds ("the string presents nothing for a fight that did not happen"), measured on one scenario (the Argent commander's cannon offense destroys the only ship). The string's `AftermathWindow::new` settles the combat window, and so presents the finished fight, **unless the head's space cannon offense paused for a scoring window** (`pending_event_scoring`; a temporary print over that lockstep: 72 of 96 constructions paused with no combatant left, the 24 that did not had one). A tactical action that moved no ship has no pause, and the string presents `complete` | new core-only payload field `Aftermath::head_paused` (set when the scoring pause is taken before the fight opens); the chain writes the phase unless `head_paused` and nobody holds the system and no draw was declared | `aftermath_chain::an_empty_system_presents_the_finished_fight_unless_the_head_paused_and_nobody_holds_the_system` (four cases: 0 / 1 ship x paused / not; R: on H3's code the (0 ships, not paused) case writes nothing); H3's lockstep (`the_additional_die_of_the_argent_commander_runs_in_the_space_cannon_offense_in_lockstep`) stays green; the sweep: 35 -> 0 `combat_presentation` differences |
| B7 | shadow harness (`native_play::shadow_play`) | the quick game now plays on past step 54 (Doctor Sucaban is native) and meets the harness limit integration 12 recorded in its section 5: one native step asks a seat's last question of an action and the first follower's question (a glued step), the string ends its step after the first and asks the follower in the next | the questions of the tail steps (a step cut in two keeps its events with the later half) open the next group, which the next legacy step must open with the same questions (a question the string never asks is a difference there); the position of a group with a tail is compared with the next group (the native position is ahead) | `native_play::tests::the_shadow_run_agrees_with_the_legacy_game_up_to_the_gap` (R: first difference at step 85 on the merge: `spend a strategy token to produce at home` native, `<absent>` legacy); `the_round_the_asks_of_a_job_and_the_shadow_count_are_the_positions` |
| B8 | trip-wire `native_subject::tests::the_turn_flow_arms_agree_with_the_legacy_game_for_every_lobby_faction` | "a known silent difference is gone": `KNOWN_SILENT` held `StrategyAbility` (Thunder's Edge Warfare's free action, Doctor Sucaban), both native now (H2, H4) | `KNOWN_SILENT` is empty | the test itself (fired as written, like integration 12's exploration pin) |
| B9 | clippy (`clippy_core.sh`) | `strategy_cards_core::plain_draw_within_limit` unused since H3 deleted Probe's own resolution; an unneeded `mut` and a very complex type in `silent4_live_tests` | function deleted, `mut` removed, `type Case` | `clippy_core.sh` |

Not a bug, a trip-wire that fired as written: the quick-game pins of `native_play` (section 4).

## 4. Edits of assertions, fixtures and harnesses (reasons)

| Where | Edit | Reason |
|---|---|---|
| `native_play` quick game (`a_native_game_plays_the_first_turns_..`, `the_budgets_end_a_game_..`, `the_round_the_asks_of_a_job_and_the_shadow_count_..`) | first gap `TurnGap(StrategyAbility)` -> `TurnGap(PeaceAccords)`; 54 native steps -> 90; shadow `(36, 54)` -> `(57, 90)` | the game plays 36 steps further: the Technology primary (Doctor Sucaban, H4) and the free tactical action of Thunder's Edge Warfare (H2) are native, the next stop is a Xxcha seat's annexation. Measured twice (`--quick --twice` gives the same fingerprint) |
| `native_subject` `KNOWN_SILENT` | the `StrategyAbility` line deleted (B8) | the test demands it |
| `aftermath_chain` presentation test | H3's two-case test replaced by B6's four cases | H3's "no ships presents nothing" was a statement about a game where the head paused; the string code (`AftermathWindow::new`) and the sweep show it presents when the head did not |
| `action_dispatch_live_tests` Carth test | from "native events only" to a full lockstep (B5) | tightened: the Cabal agent's reaction is native, the question is compared |
| `strategic_arms` | the two `research_asks_more` tests deleted | the function is gone (H4: `Carried.research` is true, Sucaban is a window); replaced by H4's `research_sucaban` and the lockstep tests |
| `ability_audit.rs` | `Status::Gap` and the `Lane` variant under `allow(dead_code)`; `asn_clear` `Dropped` | no row is left |
| `tactical_live` diff filter | H3's blanket `trigger.event_id` filter NOT taken | as in integration 12: the exact offset (1 or 2) is kept |
| `g4_abilities.rs` `table()` | also calls `register_before_muaat` | Stymie moved there (B2) |

No assertion was loosened to make a test pass.

## 5. Expected greens, and what is left

* The four Z'eu sweeps (`combat_live::{an_undefended_system_with_a_space_dock_goes_on_to_the_production_step_in_lockstep, a_space_fight_of_each_faction_runs_native_without_a_bail_and_as_the_string_does}`,
  `tactical_live::{neuroglaive_takes_a_fleet_token_from_the_activator_in_lockstep, a_tactical_action_of_each_faction_runs_native_without_a_bail_and_as_the_string_does}`) and the 3 M6d-owned tests of G4's note: **green** (the whole lib has no failure).
* **No loud gap row is left** (`silent::gaps::GAPS` is empty; no game of the matrix stops at `AbilityGap`).
* Doctor Sucaban and Thunder's Edge Warfare are native: `StrategyAbility` is no longer the first gap of any game (integration 12: 16 games).

## 6. The native playthrough (`tools/engine_core/native_play.sh`, gate profile, `te` card set)

`--quick` (sol, 4 players, seed 1): **90 native steps, then the first gap** `ask.action_menu [bail: TurnGap(PeaceAccords)]` (a Xxcha seat uses Diplomacy / its ability: the annexation window). Integration 12: 54 steps, stop at
the Technology primary. Fingerprint `29576de02c9e6ff1` (the same on the second run, `--twice`). It plays the draft (8 steps), the announcement, the phase flip, 17 turn preludes, 16 action menus, 13 follower steps of strategic actions (among them the Thunder's Edge Warfare
card: the free tactical action, then the follower window), 25 tactical steps and 9 aftermath steps.

`--quick --shadow`: 90 native steps, **57 legacy steps compared with NO difference**, then the same gap (integration 12: 54 / 36).

The gap matrix over `--factions all --players 3,4,5,6 --seeds 1` (96 games, all stop in round 1; **5,823 native steps** in all; integration 12: 3,729). First gap per game, steps then cause:

| faction | 3 players | 4 players | 5 players | 6 players |
|---|---|---|---|---|
| sol | 13 PA | 90 PA | 128 PA | 124 TOK |
| hacan | 125 PA | 25 PA | 16 PA | 147 TOK |
| letnev | 81 TOK | 115 TOK | 109 TOK | 122 TOK |
| xxcha | 14 PA | 130 TOK | 60 PA | 9 PA |
| jolnar | 13 PA | 17 PA | 99 PA | 100 TOK |
| l1z1x | 13 PA | 17 PA | 12 PA | 100 TOK |
| arborec | 13 PA | 13 PA | 51 PA | 127 TOK |
| mentak | 13 PA | 17 PA | 12 PA | 133 TOK |
| muaat | 30 PA | 11 L5 | 128 PA | 44 L5 |
| naalu | 13 PA | 17 PA | 63 PA | 127 TOK |
| nekro | 31 PA | 69 PA | 16 PA | 164 TOK |
| saar | 13 PA | 17 PA | 12 PA | 133 TOK |
| sardakk | 13 PA | 17 PA | 12 PA | 125 TOK |
| winnu | 84 TOK | 118 TOK | 123 TOK | 152 TOK |
| yin | 13 PA | 17 PA | 12 PA | 133 TOK |
| yssaril | 13 PA | 17 PA | 85 PA | 121 TOK |
| ghost | 13 PA | 17 PA | 12 PA | 140 TOK |
| argent | 27 PA | 45 PA | 123 TOK | 149 TOK |
| cabal | 13 PA | 17 PA | 12 PA | 131 TOK |
| empyrean | 13 PA | 17 PA | 12 PA | 138 TOK |
| mahact | 47 PA | 17 PA | 99 PA | 129 TOK |
| naaz | 13 PA | 17 PA | 63 PA | 133 TOK |
| nomad | 13 PA | 13 PA | 95 PA | 129 TOK |
| titans | 13 PA | 17 PA | 12 PA | 133 TOK |

| code | first gap | games | owner |
|---|---|---|---|
| PA | `ask.action_menu`: `TurnGap::PeaceAccords` (61), `secondary`: `PeaceAccords` (3) | 64 | lane E (the annexation window; `docs/G_SILENT_RESUME.md` says what it needs) |
| TOK | `tokens`: "close lent: the status steps 81.6-81.8 after the last token (KS5, lane C)" | 30 | lane C (status) |
| L5 | `Menu(Unrouted(5, 67108914))`: a leader's component action (the two Muaat tables: Umbat, the Muaat agent, a use of PRODUCTION by an ability) | 2 | F2d (`docs/KM4_RESUME.md` section 3) |

(Integration 12: PA 46, SA 16, L5 20, TOK 9, F11 3, T6 2, AG 1.) Not a single game stops at a loud ability gap, a strategy-card ability, a faction component or Sling Relay any more; 30 games now play the whole action phase of
round 1 and stop in the status phase (the `tokens` close).

Native steps by arm (all 96 games): draft 600, starting technology 12, announce 96, phase 126, turn prelude 973, action menu 923, closing menu 55, tactical 1,415, aftermath 464, secondary 730, tokens 302, status 30,
scoring 31, event scoring 30, trade 36. By island: M1 trade 36, M5 aftermath 464, M6 tactical 1,415, B turn flow 1,862 (63 gaps), C status 393 (30 gaps), E strategic action 730 (3 gaps), F menu and leaders 923;
M2, M3, M4, D: not reached.

**The same sweep with `--shadow`** (not required, run for the harness): 35 of the 96 games report a first difference (before B6 and B7: 47), the details by class:

| class | games | owner |
|---|---|---|
| a strategic action's close: `STRATEGIC_ACTION_COMPLETE` before `STRATEGY_TOKEN_SPENT` (the string spends the token first); also at `events [1]` once | 27 | lane E (KM4 section 3 knew it: "the order of a strategic action's close against a spent token") |
| movement options on a hyperlane map: native `done_moving`, legacy `move|NN|k` (the first option differs) | 3 | A2 / M6 movement facts (KM4 section 2 knew the class) |
| `PRODUCTION_RESOLVED` / `PRODUCTION_USED` native, `UNIT_ABILITY_ROLLED` / `INVASION_BEGAN` legacy (the order of a production against the unit-ability roll / the invasion) | 3 | M5 / M2 (not investigated) |
| Mitosis asked natively at the start of the status phase, not by the legacy step | 1 | the Arborec module / lane C (the step glue of the phase; not investigated) |
| `faction_marks.private:a:winnu:tactical` native absent, legacy `1` | 1 | Winnu (G2): the mark of a tactical action (not investigated) |
| `TURN_PASSED` native, `ACTION_CARD_DISCARDED` legacy (a Yssaril secondary) | 1 | lane E / Yssaril (not investigated) |

## 7. Checks (cheap), `CARGO_BUILD_JOBS=2`, private target `/root/deploytmp/int13` (profile `lockstep`, `CARGO_PROFILE_DEV_DEBUG=0`, no incremental; deleted at the end)

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets` (without ti4-review, replayer, mlp, tensor, advisor) after every merge and at the end | ok |
| `cargo test -p ti4-engine --lib` plain (final tip) | **5235 passed**, 0 failed, 7 ignored (integration 12: 5199; the first whole run after merge 3 had 4 failures: B3, B5) |
| same with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (WHOLE lib) | 5235 passed, 0 failed, 7 ignored |
| `ti4-engine-core --lib`, `-bridge --lib`, `-factions --lib`, `ti4-rule-bot-typed --lib` | 475, 107, 149, 1 passed (the first bridge run had 5 failures: B7, B8 and the three quick-game pins) |
| bridge `--test kernel_env --test play_facts --test menu_sources` | 3 + 3 + 2 passed |
| `check_layers.py core`, `lint_core.sh` | 568 files, 0 violations; 570 files, 0 hits |
| `tools/engine_core/clippy_core.sh` | ok (core crates 0 warnings; engine 41, baseline 41). The first run found three new engine warnings (B9) |
| the D4 crate (`ti4-engine-core-tests`) with the debug flags of `tools/engine_core/d4/run_core_tests.sh` (profile `lockstep`, debug assertions and overflow checks on for the three core crates), built once, in my tree (same private target), run twice (before and after the B6 change) | 146 test binaries: 1837 passed, 0 failed both times |
| `native_play.sh --quick` / `--quick --shadow` / `--factions all --players 3,4,5,6 --seeds 1` | section 6 (final binary) |
| the usage: five-hour window 4 % at the start, 9 % at the end | -- |

Where the runs were done: all in this worktree with the private target (no scratch worktree), one cargo at a time, as background scripts of this session (not Haiku runners). The target dir and the logs under
`/root/deploytmp/int13*` are deleted at the end.

Not run (user rule): `end_of_d1.sh` and its steps (golden / wide dual, `board_equivalence`, `core_sync_gate`, `alpha_roundtrip`, benches, soak, mutation ratchet, census, workspace clippy beyond
`clippy_core.sh`), `decision_replay`, mutation batches (the new lines of this integration -- `head_paused`, the shadow's tail split, Carth's detail -- were not mutation-checked), `TI4_CORE_STRICT=1`, Playwright.

## 8. Remaining gates, consolidated

Dropped by decision (not open): as in `docs/INTEGRATION11.md` section 8. Items 1 to 11 of `docs/INTEGRATION9.md` section 6 stand unless named below.

1. **Integration 14**: `core-g5-abilities`, `core-d2-c3-dispatch4`, `core-h5-leftovers` (all running). Watch the slot ranges (G5 builds on G4's `44_`; Strike Wing is `46_`), the registration order of `ce_movement` (B3), and `Render::sync_codes` (now H3's body, `bool`).
2. **The next first gaps, by games stopped** (section 6): Peace Accords 64, the status `tokens` close 30, Umbat 2.
3. **Shadow differences** (section 6): the close order of a strategic action (27 games), movement options on hyperlane maps (3), production order (3), Mitosis in the phase glue, the Winnu tactical mark, the Yssaril secondary.
4. Open from integration 12: Scheming's two machines (`DrawCards` and `HookedDraw` + `SchemingDiscard`), the dispatch of the menu (Enigmatic Device, Maw of Worlds, Stellar Converter, Titan Prototype, Silver Flame, Wormhole Generator, Cabal's Vortex, Nekro C4R, Titans' Terraform, the leader list of KM4 section 3),
   the tactical remainder (`CannonHits`, ship routes beyond five), turn flow (`ChaosMapping`, `ImperialArbiter`, `QuantumDatahub`), production / invasion / aftermath leftovers, action cards played inside another window.
5. G4's note: the research window does not ask Riftmeld's waiver and payment questions (Cabal) and Yin's Brother Omar with it (a counted bail of the live path); the Yin hero's scoring hook; Task 7 of H3 (ChaosMapping, Imperial Arbiter, Quantum Datahub swap).
6. Heavy gates still to run once a zero-bail playthrough is possible: golden and wide dual sweeps, `board_equivalence`, `core_sync_gate --strict`, `alpha_roundtrip --strict`, the benches, `census`, `soak`, the mutation ratchet and the
   mutation batches of every resume note since integration 9 (this integration adds: `head_paused`, the shadow's tail split, `delete_gap_rows.py` for zero rows), workspace clippy, `TI4_CORE_STRICT=1`.

## 9. Tip

`git log -1 core-integration-13` (the last commit is this document's). The private target dir was deleted.
