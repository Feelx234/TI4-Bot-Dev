# W lane 13 resume note, branch `core-w-l13` (from `core-w-l9` c639bdfc, merged with `core-w-l10` 959b95d1)

Task: the lent bodies still left after lane 9 (TE strategy primaries in the heroes, the counted bails of lane 9, nested reaction windows in the heroes,
bail-as-failure tests). Pure refactor: `RULES_VERSION`, serde output and fixtures are unchanged. No heavy work (no golden/wide dual, bench, soak,
ratchet, workspace clippy, census). `core-w-l11` / `core-w-l12` had no commits beyond l10's tip when this lane started (nothing to merge from them).

## Merge note
`core-w-l10` was merged first. Conflicts: `dual_window.rs` (l9 extracted `build_settle_env` out of `settle_run`; l10's edits to the body were already in
the extracted function; the header was taken from l9, `waylay_check(gs, script)` kept), X-89 in the committed fight (`FightFacts::x89`; the Yin gate
for X-89 is gone), `tests/combat_fight.rs` (the new `Env` fields).

## Ported (core half + Ask/Opt/Subtype + byte-exact renderer + lockstep test, each failing on the pre-W13 code)

| piece | core | adapter | test |
|---|---|---|---|
| TE Warfare primary in the Winnu hero (the redistribution, the system of the free tactical action; the system reaches the game as the dual value `(card, resolved, free system)`) | `Spawn::TeWarfare`, `FreeSystems`, `Report::Free`, `Finished::free`, `WinnuHero::free` | `hero_dual::free_systems`, `factions::winnu::use_leader_strategy_primary` (value type) | `winnu::tests::the_te_warfare_primary_runs_in_lockstep_with_the_core_and_carries_the_free_action`, core-tests `hero_windows` |
| TE Construction primary (the choice, the use of PRODUCTION as a child `HomeProduction` of the production area, then the second structure), in the Winnu hero and played by its holder | `Construction::new_te_producing`, `CStage::Producing`, `Spawn::ConstructionTe`, `TeUses`, `Report::Produced`, `WinnuHero::produced` | `hero_dual::te_uses`, `hero_dual::te_construction` (`strategy_cards_core::construction_te` calls it) | `strategy_te_tests::the_te_construction_production_runs_as_a_child_of_the_card_in_lockstep`, `winnu::tests::the_te_construction_primary_...` |
| Mahact fleet pool gates (Leadership / Warfare secondaries and primaries in the heroes, the Empyrean command token): the core reads the pool from the marks (`fx::tokens_held_by_mahact`, W10) | `fx_perform::begin`, `EmpyreanHero::gain_tokens` | | core-tests `hero_windows` (two tests rewritten) |
| Imperial primary: a point past the victory target stops at it (`fx::adjust_victory_points`) | `ImperialPrimary::finish` | `imperial_facts` (the gate is gone) | `strategy_te_tests`, `strategy_cards_core::tests`, core `imperial_primarys_point_for_mecatol_stops_at_the_victory_target` |
| Home production: the Prophecy of Ixth is discarded by the window (a swapped planet value cannot linger into a use outside the tactical action: declared in its production, ended with it) | `HomeProduction::finish_use` | `leaders_core::home_declined` | core-tests `production_home` |
| Yin hero gates: X-89 (`FightFacts::x89`), Dacxive Animators (`FightFacts::dacxive`), the Letnev Dunlain Reaper (`FightFacts::pay_env`, `fx_pay::can_afford`: what the seat can spend is read in each round), hits added to a roll (the kernel hook, read in `GroundRoundHits`), the Minister of Exploration and the Crown of Emphidia on the control gain (`YinFacts::minister`, `fx::steal_throne_card`), the Thundarian | `rules::invasion::{fight,round,begin,yin_hero}`, `kernel::fx` | `dual_window::{yin_hero, yin_unported, round_hits_unported}` (the Agenda slice is written) | core-tests `invasion_yin`, `invasion_windows`; `yin::tests::the_effects_of_the_core_fight_run_in_lockstep_and_no_window_bails` |
| Nested reaction windows in the heroes: the hero windows follow the nested activity of the string run (`outside_following` + `nested_check` + `nested_fp`; `drive_windowed` replays it on `reactions_core::table()`, which now carries the Self-Assembly link) for Naaz, Winnu, Empyrean, Mahact, Yin, the TE Construction | `dual_window::outside_following`, `hero_dual`, `reactions_core::table` | | the existing lockstep tests (Thundarian, ...) |
| Indoctrination (Yin) as a rule of the core table: question, `pay_in` influence, replace the opponent's first infantry, the Omar mark | `cd_registry::yin::timing::INDOCTRINATION`, `Ask::Indoctrination`, `Opt::Indoctrinate`, `Subtype::Indoctrination`, `Codes::indoctrination` | `render/reaction.rs`, `reactions_core::NAMED`, `dual_window::with_trigger_of` | `yin::tests::the_hero_runs_in_lockstep_with_the_core` (the case with the means to pay) |
| Bail-as-failure helpers | | `dual::tests::{native, no_bails_in, bailed_windows}`, `core_sync::unsupported::enter_disabled_policy` | used by the hero lockstep tests (naaz, winnu x3, arborec x2, argent, cabal, empyrean x3, saar, yssaril, yin) |

## Left (precise reasons)
* **Warfare secondary after a Leadership / Technology / Construction secondary (Naaz hero)**: the facts of the production (`limit`, `buildable`, `fleet_limit`,
  `discount`) are read by the string (`ProductionWindow::new`, `buildable_for`, `fleet::limit`) on the position the card starts from; after a secondary that
  changes them they are stale. The core has the capacity query (`rules::production::capacity`) but not `buildable_for` / the fleet limit / the cost reduction
  hooks, and a mid-run decode of the core state into a string position (to call `home_facts` there) needs the `Ctx` that `with_core` holds. Counted bail
  (`NaazHero::unsupported`).
* **Two seats of one faction (Empyrean replenish, Politics speaker)**: the Trade Agreement note and the option ids are keyed by faction in the core and in the
  string ("which the string cannot tell apart either"); not reachable in a drafted lobby.
* **Yssaril draw (Politics, Lost Crew, Strategize)**: Scheming (one card more, then the discard question, the staged `ACTION_CARD_DISCARDED`) and Crafty (no
  limit) are in `action_cards::draw` only; `DrawCards` has no Scheming mode (needs an `OwnHand` reason, the staged discard, and the Deepgloom Executable,
  a Thunder's Edge breakthrough, stays out). Counted bail via `strategy_cards_core::draw_hook`.
* **A scoring window after a won combat (Yin / Mahact heroes)**: who may score what, in the table order, is the string `ScoringWindow`; the core has the
  offers (`query::objectives`) but the window order is not ported (`ENGINE_KNOWN_ISSUES`, A4).
* **Yin: a coexisting planet** (Thunder's Edge Coexistence; the chain of combats, the bombardment targets), **a neutral side** (lane 11), Proxima Targeting
  VI and Alarum (dropped factions), L1Z1X Fealty Uplink (Thunder's Edge breakthrough).
* **Mahact**: dice the core cannot reproduce (test faces, a stream restore); a Letnev / Mentak round opening and the other `SettleArgs` gates the
  machine reports (`Machine::bailed`).
* **Nested reaction windows**: Greyfire Mutagen, the Ssruu copies of Indoctrination, the Spinner, Van Hauge and the Ascendant (Yin) are not rules of the core table
  (`Recording::unported` names them: a counted bail). Action cards played inside a window are lane 11's.
* The "more than sixteen scoreable objectives" gate of the Imperial primary stays (`ImperialFacts` holds sixteen; a game cannot reveal more than ten at a
  time that a seat can score).

## Checks (cheap only; private target dir `/root/deploytmp/core-w-l13`, `CARGO_BUILD_JOBS=2`)
* `cargo test -p ti4-engine --lib`: 4889 passed, plain and with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1`, the one failing test (`action_cards_b6d_tests::bail_leadership_...`, the Mahact pool now carried) then fixed and re-run with `factions::yin`; `cargo test -p ti4-engine-core -p ti4-engine-core-factions -p ti4-engine-core-tests`: all green after the effects.rs unit test followed the lifted gate.
* `tools/engine_core/check_layers.py core`: 0 violations; `tools/engine_core/lint_core.sh`: 0 hits.
* Mutation (single-file, scratch worktree `core-w-l13-mut`, runner `/root/deploytmp/core-w-l13-mut/runmut.py`, specs `spec1.py`, `spec2.py`, Haiku runner): 31 mutants of the new core halves; 25 killed at once, 6 survived, 3 closed by new tests (`unsupported |=` of the child use, the winner of Dacxive, the Prophecy threshold), 1 killed by the lib unit test outside the batch's test target (Imperial clamp). Open survivors: Dacxive's `remaining >= 1` (needs an empty box), `x89` in the committed fight's roll tail facts.
Not run: golden / wide dual, benches, soak, ratchet, workspace clippy, census.
