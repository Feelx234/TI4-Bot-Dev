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

## Phase 2 (after the first report)

Merged `core-w-l2` (Galactic Threat is lane 2's, Future Sight and Artuno lane 3's, the Thundarian lane 1's; slots renumbered) and
`core-w-l1`'s tip. Ported on top: `production_used`, `self_assembly_after_production`, Transit Diodes (W12-design-02); the Neuraloop's
roll, purge and swap, the Titan Prototype's payment, structure and trade good (W13-design-02); the commander reads of every direct
caller (W14-design-02); Production Biomes (W11-design-01). Counted gates (named in the census, each with a test): Military Support
(W14-design-02), the action leaders of the string (W10-design-03), the faction component actions (W11-open-01).

## Pieces not done, with the reason (each is pinned by a counted bail or gate, or by the string tests that exist)

* W9: Mercer and Sardakk's agent (a core emission of `TACTICAL_ACTION_ENDED`: D1b tactical aftermath), Thunder's Paradox, Infantry II's return
  and Sardakk's clean-up (a window announcing `TURN_BEGAN`: D1b turn prelude), Temporal Command Suite (the trade window as a child), Magen's
  ground-combat hit, Mahact Imperia, Ssruu copies outside combat, Ambuscade. `bail_every_family_that_stays_in_the_string_is_named`.
* W10: the action leaders of W10-design-03 (named gates).
* W11: the component actions of W11-open-01 (named gates); the listing `component_actions` and the dispatch `perform_component` are D1b's.
* W12: Chaos Mapping at the start of a turn, Minister of Industry and Freelancers (lane 1's `produce_one`, W5 item 3, on no mergeable
  branch); `technology::component_actions`/`perform_component` are composition (Sling Relay's production is the core's window; the
  dispatch is D1b's); `control_gained` is Keleres (dropped).
* W13: the composition of the exploration rewards (D1), `frln*` (production, lane 1).
* W14: Military Support's placement (a generic `PlaceUnits` window and the turn prelude: W14-design-02).
* Thunder's Edge content: undecided, nothing ported (as instructed).

## Not run (by the directive, and at the end)

No `verify_package`/gate, no golden or wide dual run, no census re-run, no bench, no soak, no clippy, no mutation ratchet. Run: the
targeted tests, `cargo test -p ti4-engine-core --lib`, `cargo test -p ti4-engine --lib` (full), `check_layers.py core`, `lint_core.sh`, and
single-file mutation checks (see the final report).

## Merge notes

`git merge-tree` against `core-w-l1` conflicts only in `docs/ENGINE_KNOWN_ISSUES.md`; against `core-w-l2` also in
`reactions_core.rs` (the `NAMED` list), `render/cards.rs` (the `Ask` match arms), `agenda_reveal_tests.rs` (appended tests). All additive:
keep both sides. Shared types I touched: `choice::cards::Ask::PredictThreat`, `choice::trade::Shape::Free`, `choice::relic::Ask::GainTechnologyFrom`,
`choice::technology` (was empty), `kernel::ability::Codes` (+ `future_sight`, `not_from_supply`, `galactic_threat`), `Eff::state_and_kernel`,
`Window::accepts_unlisted`, `DealFacts::free`, `TradePrimary::with_sources`, `HacanAgent::with_source`.
