# Lent bodies, lane 3 (branch `core-w-l3`, from `core-w` @ 4808a0ab)

Packages of the lane: W9 ability-table tail, W10 leaders, W12 technology, W13 exploration/relics, W14 promissory, W15 free-form
deals, W11 faction component actions. Lines per piece: `docs/ENGINE_KNOWN_ISSUES.md` (W9-design-01..03, W10-fix-01, W10-design-02,
W11-open-01, W12-design-01, W13-design-01, W14-design-01, W15-design-01).

## Gate deferred to the end of D1

By the user's directive no heavy work ran: no `verify_package`/gate, no golden or wide dual run, no census re-run (the numbers of
`docs/LENT_CENSUS.md` are those of `core-w`), no bench, no soak, no mutation ratchet, no clippy, no board_equivalence. Gate deferred to
end of D1. What ran: `cargo test -p ti4-engine-core -p ti4-engine-core-factions`, `cargo test -p ti4-engine --lib` (full, by Haiku runners
in a scratch worktree at intermediate commits: 4605, 4617 passed, 0 failed), targeted tests while working, `check_layers.py core` (0
violations), `lint_core.sh` (0 hits), single-file mutation checks (`/root/deploytmp/core-w-l3/mut1..5.sh` over a scratch worktree).

## What changed in the census terms (estimated from the census tables of `LENT_CENSUS.md`, not re-measured)

| window (golden, calls bailed before) | change |
|---|---|
| `strategy_cards::trade_primary` 58 of 100, `trade_secondary` 70 of 116, `leaders::hacanagent` 8 of 11 | the lent Trade Agreement and the Cabal are the core's; a siphon plot and two seats of one faction remain |
| `game::close_vote` 12 (Future Sight), `game::reveal_agenda` 10 (Galactic Threat) | both abilities are in the core table |
| `relics::ask_technology` 7 of 9 | a source that is no relic is a core window |
| `transactions::answer` (free-form deals, never in the golden games) | followed by the core, random census 415 steps 0 bailed |
| Nomad games: nested `unported ability` at every trade-good gain (Artuno), Winnu at Mecatol, Magen holders at every activation | followed |

## Pieces not done, with the reason (each is pinned by a counted bail or by the string tests that exist)

* W9: Mercer, Thundarian (lane 1), Temporal Command Suite, Thunder's Paradox, Magen's ground-combat hit, Infantry II's TURN_BEGAN return,
  Mahact Imperia, Ssruu copies outside combat, Ambuscade, Sardakk's TURN_BEGAN clean-up. `bail_every_family_that_stays_in_the_string_is_named`.
* W10: every other leader arm asks over the map, a production or a nested card; `check_unlocks`/`award_tail`; Artuno's payout.
* W11: nothing started; see W11-open-01.
* W12: `production_used` (opening of the production window, lane 1), `self_assembly_after_production`, `component_actions`/
  `perform_component` (lane 1 W5), `control_gained` (lane 1 W8), Transit Diodes and Chaos Mapping at the start of a turn (kept on the string
  with a counted reason, test `a_turn_start_with_nothing_to_offer_asks_nothing_in_lockstep_and_transit_diodes_stay_the_strings`).
* W13: the effects behind the reward answers are already `dual_write`s in `exploration.rs` (commodities, tokens, placement); what is left is
  the composition (D1) and `frln*` (production, lane 1); relic bodies: Neuraloop's pool check, Titan Prototype's payment and structure.
* W14: Military Support's placement window, the commander reads (`has_commander_ability`, `commander_ability_from`).
* Thunder's Edge content: undecided, nothing ported (as instructed).

## Merge notes

`git merge-tree` against `core-w-l1` conflicts only in `docs/ENGINE_KNOWN_ISSUES.md`; against `core-w-l2` also in
`reactions_core.rs` (the `NAMED` list), `render/cards.rs` (the `Ask` match arms), `agenda_reveal_tests.rs` (appended tests). All additive:
keep both sides. Shared types I touched: `choice::cards::Ask::PredictThreat`, `choice::trade::Shape::Free`, `choice::relic::Ask::GainTechnologyFrom`,
`choice::technology` (was empty), `kernel::ability::Codes` (+ `future_sight`, `not_from_supply`, `galactic_threat`), `Eff::state_and_kernel`,
`Window::accepts_unlisted`, `DealFacts::free`, `TradePrimary::with_sources`, `HacanAgent::with_source`.
