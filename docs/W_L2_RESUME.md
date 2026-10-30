# W lane 2 resume note (W4, W6, W7), branch `core-w-l2`

Cut from `core-w` @ `4808a0ab`. Pure refactor: `RULES_VERSION`, serde output and fixtures are unchanged. Ids of the issues:
`docs/ENGINE_KNOWN_ISSUES.md` rows `W6-*`, `W7-*`, `W4-bail-01`.

## Gate deferred to the end of D1

By the user's directive no heavy work runs before the end of D1. NOT RUN: `verify_package.sh`, the gate profile, the golden dual
(`profile-20`, `golden-lent.json`, `all=dual`, wide mask), `board_equivalence`, benches, clippy, soak, wasm size, release builds, the
mutation ratchet, the G-L2 gate (strategy / agenda / trade). The post-fix census of the windows below is therefore NOT measured:
the "before" numbers in the known-issues rows are those of `docs/LENT_CENSUS.md` (commit `3de90cbe`). To run the census later:
the commands of `docs/W_RESUME.md` (golden and golden-lent, both must show 0 differences); look at `strategy_cards::imperial_secret`,
`imperial_primary`, `politics_primary`, `politics_secondary`, `technology_primary`, `technology_secondary`, `game::close_vote`,
`game::reveal_agenda`, `agenda_effects::resolve_with`.

## Done

* W6: Imperial's secret draw asks the hand limit (child `HandLimit`); Politics draws through the new `DrawCards` child (action card
  hand limit, rule 2.4); the Yssaril gate is the drawer's own; a Mentak seat researches in the core (only The Table's Grace stays).
* W7: Politics / Keleres Rider and Unconventional Measures use the same `DrawCards` child; Future Sight and Galactic Threat are core
  abilities.
* W4: nothing ported (all four cards are tail: about 2 nested runs per 20 golden games each); each has a named bail and a test, see
  `W4-bail-01`.

## Not done, with the reason

* W6 items that need W5 (Leadership / Construction secondary seat predicates): W5 is lane 1's; its `unported_window` narrowing is on
  `core-w-l1`, not re-measured here.
* Technology Rider (needs a free-only mode of the `Research` child in `rules/production/strategy_cards.rs`, a file lane 1 edits),
  Construction / Diplomacy / Warfare Riders (planet or system question, form flip, staged events): pinned bails.
* Rearmament For, Swords to Plowshares Against, Colonial Redistribution, Research Grant Reallocation, Galactic Crisis Pact, Data Skimmer
  in the action phase: no core effect window; pinned by `an_agenda_without_a_core_window_bails_with_a_reason`. Ral Nel is non-lobby.
* Artuno the Betrayer (Nomad agent, `TRADE_GOODS_GAINED` after, W9): without it a Nomad with a readied agent still bails the close
  that pays Future Sight. `NOT_FROM_THE_SUPPLY` sources need interned codes (`transaction`, `pillage`, `extreme_duress`,
  `trade_agreement`, `artuno`) and the pile mark `MarkKey::NomadArtuno`.
* Politics / Imperial: `imperial_primary` past the victory target, `research_asks_more` (Specialist Compounds, Doctor Sucaban), Yin
  Brother Omar, Inheritance Systems after the bill, Jae Mir Kan with a Nomad watching.

## Cheap checks that were run

`cargo test -p ti4-engine-core --lib` (327 ok), `cargo test -p ti4-engine --lib` on the touched modules (strategy_cards_core,
strategy_research_tests, action_cards_b6d_tests, agenda*, reactions_core registry parity, the new Galactic Threat / Future Sight
tests) and then the whole engine lib (see the report), single-file mutation checks with `tools/engine_core/mutants.sh` on committed
snapshots in the scratch worktree `core-w-l2-scratch` (`draw.rs`, `close.rs`, `effects.rs`, `strategy_primaries.rs`, `threat.rs`,
`nomad/timing.rs`). Also `tools/engine_core/check_layers.py core` (0 violations) and `lint_core.sh` (0 hits).

## Mutation results (single-file, scratch worktree, committed snapshot `c7701e05`)

`draw.rs` 18/18 killed (2 points not compilable), `close.rs` start_draw/pay 16/16, `effects.rs` `Resolve::cards` killed, `strategy_primaries.rs`
Politics / Imperial windows killed, `threat.rs` and `nomad/timing.rs` run through the engine tests (`galactic_threat`, `future_sight`).
Survivors kept on purpose: `Resolve::draw` (secret objective draws, A4 code, not touched), `Politics::ask_of` `ctx(.., false, ..)` of the
speaker / agenda questions (B8 code, not touched), `future_sight_when` `return false` for an event without an outcome (marked
`mutants:skip`: the event always names it). The two `threat.rs` survivors of that run (`payout`'s discard and agenda guards) are killed by
the unit tests added afterwards (`the_payout_needs_the_same_agenda_the_same_outcome_and_no_discard`), which were not re-run through the tool.

## Traps for the merge

* `strategy_cards_core.rs` is also edited by lanes 1 (W5: `unported_window`, Leadership / Construction) and 3 (Trade): my hunks are
  `draw_secret`, `imperial_primary`, `draw_hook` / `plain_draw`, `research_seat_unported`, `politics`, `drive_window` and the tests.
* `reactions_core::table()` now sets `facts.card_groups`; `linked_table()` is the table of every window that runs children.
* `Politics`, `ImperialPrimary` and `Close` are no longer `PartialEq`/`Eq` (they hold an `ErasedChild`).
