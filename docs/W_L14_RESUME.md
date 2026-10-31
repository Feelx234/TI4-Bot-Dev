# W lane 14 resume note, branch `core-w-l14` (from `core-w-l13` 9b48bcbf)

Task: the lent bodies lane 13 left that nobody else owns (lane 11 `core-w-l11`: neutral sides and action cards played inside other windows; lane 12 `core-w-l12`: Nomad
agent exhaustion, Ssruu, Inferno, Hecatoncheires, legacy versions, the invasion tail; `core-m3-invasion`: the native invasion island; `core-d2-f-live2`: `driver/live.rs`,
`driver/step.rs`, `state/pending.rs`, `session.rs`: none of these touched). Pure refactor: `RULES_VERSION`, serde output and fixtures are unchanged. No heavy work (no golden / wide
dual, bench, soak, ratchet, workspace clippy, census): a full zero-bail playthrough is not possible yet.

## Ported (core half + Ask/Opt/Subtype + byte-exact renderer + lockstep test, each failing on the pre-W14 code)

| piece | core | adapter | tests |
|---|---|---|---|
| Naaz Warfare secondary after a Leadership / Technology / Construction secondary: the facts of the home production are read from the core when the use starts | `production::home::{home_facts, fleet_limit, buildable_for}`, `HomeProduction::at`, `Spawn::HomeAt`, `fx_perform::Home::At`; `NaazHero::changes_production` deleted | `hero_dual::home_of` (the home system and whether the table is one the production windows follow) | `production_home` (4 new), `hero_windows::the_home_production_reads_the_position_when_it_starts_not_where_the_card_started`, `naaz::tests::a_warfare_secondary_beside_a_secondary_that_changes_the_production_runs_in_lockstep_with_the_core` (four secondaries x both orders, no window bailed) |
| The announcement of the home production (`UNITS_PRODUCED`) goes where the core finished the use (the "staged events would interleave" bail is gone) | `NaazHero::home_staged` | `hero_dual::naaz_hero` applies the staged events in two halves | `hero_windows::the_announcement_of_the_home_production_goes_after_the_events_staged_before_it` |
| Yssaril draw hook (Politics, Lost Crew, Strategize, agenda close): Scheming (a card more, then the discard `OwnHand { Scheming }`, staged) and Crafty (no limit) | `DrawCards` follows `economy.action_card_draw_bonus` / `action_card_limit`; `Reason::Scheming`, `Subtype::SchemingDiscard` | `render/cards.rs`, `strategy_cards_core::draw_hook` (only the Deepgloom Executable, blank laws and Ral Nel remain), footprints write the Marks slice, the Empyrean hero's kernel is the full table | core `draw_hooks` (5), `strategy_cards_core::politics_follows_scheming_and_crafty_in_the_core_window` |
| Greyfire Mutagen as a rule of the core table | `yin::timing::GREYFIRE` | `reactions_core::NAMED` | core `yin_greyfire` (3) |
| Supercharge carried by a Valefar Assimilator (the two gates are gone) | `naaz::offer::offer_supercharge` through `fx_strategy::text_carrier` / `exhaust_technology_text` | `dual_window::{round_opening_why, begin_ground_unported}` | `combat::tests::a_supercharge_carried_by_a_valefar_assimilator_is_offered_and_used_in_the_core_in_lockstep` |
| Tuple-style hero tests tightened to `bailed == 0` + `native()` | | `naaz::tests::the_hero_runs_in_lockstep_with_the_core`, `winnu::tests::...`, `mahact_units::tests::...` | |
| Irreproducible dice in the Mahact hero documented and tested (XF-7: counted bail outside the product, assertion inside) | | | `mahact_units::tests::a_hero_fought_with_test_faces_*` |

## Left (precise reasons)
* **A scoring window after a won ground combat in the Yin hero** (`dual_window::yin_hero` still declines a recording with a "score an objective" question; `CommittedFight::scoring` lists the occurrences). The string opens it inside
  `InvasionWindow::fight_committed_planet` after each ground round, over `state.initiative_order()`, with the secrets whose phase is Action and whose feat was recorded for the occurrence
  (`secrets::scoreable_event`: only the twelve feat secrets, all Action-timed, plus a leaked public one) and asks through `ask_seeing`. A core port needs an `EventScoring` window (a `ScoringBook` walk over
  core-computed `Offers`, a `ScoreOne` child per player, the plain `OBJECTIVE_SCORED:{alias}` record and the `GAME_FINISHED` stop), a `FStage::Scoring` in `CommittedFight` that runs it for each occurrence in `scoring`, a link in
  `driver::children`, and the adapter must compare the plain records the string pushes (`ctx.emit("OBJECTIVE_SCORED:..")`) which `Run::with_events_vec` does not carry. The live driver's own `EventScoring` arm (`core-m3-invasion`,
  `driver/session.rs`) runs a scoring pause after a window, which would order a hero's scoring after the whole hero. **The Mahact hero has no scoring window to port**: the string consumes the pause of its combat
  (`window.take_scoring_occurrence()` in `resolve_resolving`) and the core fight matches (lane 9).
* **Van Hauge, the Spinner, the Yin Ascendant** (reasons in `docs/ENGINE_KNOWN_ISSUES.md` W14-design-03): Van Hauge needs the combat machine to drain staged destructions at the string's four points (a retreat, produced hits, after
  `SPACE_COMBAT_ROUND_ENDED`, the end announcement) instead of bailing at a transition; the Spinner a placement question over planets and ship spaces (`PlaceUnits` has planets only); the Ascendant is a Thunder's Edge breakthrough.
  There is no Ssruu copy of Indoctrination (Ssruu copies the agent only; the Milor copies are the pair rules).
* **Remaining gates of the combat machine** (`SettleArgs::gate_why`, `Machine::bailed`) after Supercharge: a neutral side (lane 11), diplomacy, goods only a Keleres agent could make spendable, a Last Bastion retreat (all dropped
  by decision), the Heart of Ixth (Thunder's Edge), and irreproducible dice (XF-7: a product assertion; the counted bail is reached only by dev tools and unit tests with test faces).
* **Two seats of one faction**: the lobby cannot seat them (`FactionAssignmentError::DuplicateChoice`); left.
* **The Deepgloom Executable** (a Thunder's Edge breakthrough that lets another seat borrow Scheming), blank laws (Keleres' Law's Order) and a Ral Nel seat keep a draw with the string (`draw_hook`).
* **The inline Lost Crew of an action card's effect** (`rules/action_cards/effects.rs`) still needs a hook-free draw within the limit (`plain_draw_within_limit`): lane 11's (action cards inside other windows). The agenda *effect* window
  (`dual_window::agenda_effect`, plain kernel) keeps a faction-hooked draw with the string (`draw_hooks_on_plain_kernel`); the vote close follows the hooks.

## Checks (cheap only; private target dir `/root/deploytmp/core-w-l14`, `CARGO_BUILD_JOBS=2`)
* `cargo test -p ti4-engine --lib`: 4894 passed, 0 failed, plain and with `TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (final code); `cargo test -p ti4-engine-core-tests` (all binaries), `-p ti4-engine-core -p ti4-engine-core-factions`: green.
* `tools/engine_core/check_layers.py core`: 442 files, 0 violations; `tools/engine_core/lint_core.sh`: 0 hits.
* Regression tests that fail on the pre-W14 code: the Naaz pair test (counted bail before), `politics_follows_scheming_and_crafty_in_the_core_window` and the four former bail tests of the Yssaril draw, `a_supercharge_carried_by_a_valefar_assimilator_*`, the core `yin_greyfire`, `draw_hooks`, `production_home` additions.
* Mutation (single-file, scratch worktree, Haiku runner, 34 mutants of home.rs, hero_windows.rs, fx_perform.rs, draw.rs, yin/timing.rs): 30 killed, 3 survived, 1 invalid (a pattern that did not match). Survivors are equivalent: the `limit > 0` guard of `HomeProduction::start` (differs only by the `ProductionWindowClosed` observer of an Argent/Keleres seat), `requested > 0 &&` of the Scheming test (`drawn > 0` follows from it), and the answer check of the Scheming discard (the driver refuses an unoffered pick first). Not mutated: `offer_supercharge` (killed by the engine lockstep test only).
Not run: golden / wide dual, benches, soak, ratchet, workspace clippy, census.
