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
See the final report of the lane and `docs/ENGINE_KNOWN_ISSUES.md` rows W14-design-01..06.

## Checks (cheap only; private target dir `/root/deploytmp/core-w-l14`, `CARGO_BUILD_JOBS=2`)
See the final report. Not run: golden / wide dual, benches, soak, ratchet, workspace clippy, census.
