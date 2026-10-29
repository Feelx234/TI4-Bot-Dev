# B6 closing package: the fixed card list (2026-10-29)

Derived from `ENGINE_KNOWN_ISSUES.md` B6-bail-01..14, `ENGINE_REACTIONS.md` sections 7 and 9 (reaction-slot plays done in stage 3:
Sabotage, Intercept, Parley, Direct Hit, Skilled Retreat as `CardPlay`s) and the code (`Fx::of`, `Simple::of`, the registered
`CardPlay`s). Of the 142 action cards, 97 have a core effect (44 `Simple` stamps, 44 `Fx` windows incl. Intercept, Rescue, Fighter
Conscription, Manipulate Investments, 9 registered plays incl. Direct Hit x4, Skilled Retreat x4, Parley, Sabotage x4 counted in Fx).
The 45 cards below (by printed name, 55 copies counted as names) had no core effect at the start of this package.

Status column: PORT (core effect, lockstep test, mutation checked), BAIL (documented, with a test). Updated as the package
proceeds; the final column is the state at the closing commit.

| card | window | what it needs | plan |
|---|---|---|---|
| Repeal Law | action | pick a law in play, B9 `laws` leave | Fx pick |
| Spy | action | pick a seat, a random card (dice stream) | Fx + dice |
| Plague | action | pick a rival planet, one die per infantry, staged ground destruction | Fx + dice + staged mark |
| Unstable Planet | action | hazardous planet, 3 infantry destroyed (staged mark) | Fx + staged mark |
| Seize Artifact | action | neighbours with relic fragments (map fact) | Fx + neighbour fact |
| Lie in Wait | after neighbours transact | neighbours who traded (map + transactions fact) | Fx + fact |
| Exchange Program | action | another seat answers yes/no, fleet tokens | Fx (second asked seat) |
| Reparations | after other gains your planet | two questions, control-gained record | Fx |
| Public Disgrace | when other chooses a strategy card | the strategy card pick again | Fx |
| Summit | start of strategy phase | gain 2 command tokens (pool pick) | Fx |
| Ghost Ship | action | wormhole systems free of ships (map fact) | Fx + fact |
| Lucky Shot / Scuttle | action | per-unit option details | see plan |
| Archaeological Expedition / Exploration Probe | action | exploration decks (rng, frontier) | see plan |
| Divert Funding / Brilliance | action | technology / planet | see plan |
| Bribery / Focused Research / Impersonation / Plagiarize / Reveal Prototype / Mercenary Contract / Pirate Fleet | various | payment windows, research | registered play + child window |
| Strategize x4 / Overrule | action | perform another card's ability | registered play + child window |
| Pirate Contract x4 / Refit Troops / Upgrade / Decoy Operation / Ghost Squad / Experimental Battlestation | various | faction hooks, neutral units, unit-form flip | see plan |
| Fire Team / Scramble Frequency / Courageous To The End / Crash Landing | in a combat window | rerolls, dice, casualty questions | see plan |
| Harness Energy / Infiltrate / Signal Jamming | various | tile catalogue, swap rules, blockade map | see plan |

## Closing state (the commit that is gated)

PORTED in this package (core effect in `rules::action_cards::effects::Play`, lockstep test in
`ti4-engine/src/action_cards_closing_tests.rs` through the announce window with three deciders (last option, first option,
last option that does not stop), mutation-checked; Spy/Plague/Repeal/Unstable also in
`action_cards::tests::repeal_law_spy_plague_and_unstable_planet_agree_with_the_core_window_in_lockstep` through the play window):

| card | core half |
|---|---|
| Repeal Law | law pick (kind `repeal`, `laws` display facts taken from the recorded question), `fx::repeal_law` (shared with the agenda area) |
| Spy | seat pick, one die (`RngDomain::Dice`, 10 sides) picks the card `(face - 1) % held`; faces and stream positions compared |
| Plague | planet pick, one die per infantry, a six destroys one; ground-force deaths staged as `private:#staged:ground:` rows (`kernel::fx_ground`) |
| Unstable Planet | hazardous planet pick, exhaust, up to three infantry destroyed and staged |
| Seize Artifact | neighbour (seating order) with fragments, then the fragment kind |
| Reparations | exhaust (asked of the new controller) then ready (asked of the holder); skipped steps as the string |
| Public Disgrace | the picker's strategy card goes back to the mat, the picker chooses again ("if able" restores) |
| Archaeological Expedition | planet deck pick (printed traits, then attachment traits), top three cards, fragments kept |
| Lucky Shot | per-ship options (one per distinguishable kill), display facts from the recorded question |
| Scuttle | one or two own non-fighter ships (second question offers "stop after one"), cost in halves as trade goods |
| Summit | two pool questions of the strategy area (`gain_pool_choice`, previews and pool details completed in the adapter) |
| Exchange Program | the asked seat answers, one generic infantry each on the planet, or both pay a fleet token |

Also new in the machinery: `Runner::with_seeds` (a rule that rolls while it writes), the component action's window plays a
registered `CardPlay` like an inline play (`Sx` implements `has_registered_play` / `run_registered_play` / `after_play`;
unit test `a_registered_effect_runs_in_the_component_actions_window`), the dice of a card that rolls are lent to the core
window (`Sources::rng`, the streams' positions compared like the agenda effects), the play footprint carries the ledger.

DOCUMENTED BAILS (the string effect runs alone; `Fx::of` is `None`; the list and the reason of each is the constant `BAILS`
in `rules/action_cards/effects.rs`, and `every_action_card_is_ported_or_a_documented_bail` fails when a card is in neither
that list nor the ported set): Bribery, Brilliance, Courageous To The End, Crash Landing, Decoy Operation, Divert Funding,
Experimental Battlestation, Focused Research, Fire Team, Ghost Ship, Ghost Squad, Harness Energy, Impersonation, Infiltrate,
Signal Jamming, Lie in Wait, Mercenary Contract, Overrule, Pirate Contract x4, Pirate Fleet, Plagiarize, Exploration Probe,
Refit Troops, Reveal Prototype, Scramble Frequency, Strategize x4, Upgrade.
What they wait for (not a card list problem but a missing child-window helper and facts): a `Child<W>` runner for the
question-driven payment and research windows (Focused Research, Divert Funding, Impersonation, Plagiarize, Reveal Prototype,
Mercenary Contract, Pirate Fleet), the neutral seat (Pirate Contract x4, Mercenary Contract, Pirate Fleet), the map facts
(Ghost Ship, Signal Jamming, Exploration Probe, Lie in Wait), the faction unit-form flip (Refit Troops, Upgrade, Decoy
Operation, Ghost Squad, Crash Landing, Experimental Battlestation), reroll questions with the dice stream inside the
announcement (Fire Team, Scramble Frequency, Courageous To The End), the goods window of the Keleres agent (Bribery, Focused
Research, Mercenary Contract), the performed ability of another card (Strategize, Overrule), the capture glue (Infiltrate,
Harness Energy). Bails with an adapter-level behaviour keep their tests from earlier slices (`bail_a_card_with_an_unported_effect_*`).

Conditions inside the ported cards that stay string (each a `None` of `Play::of` or an `unsupported` window, one test each):
Summit with a Mahact token held; Archaeological Expedition and Veto with a deck reserve; Plague / Unstable Planet when a
galvanized ground force falls (the staged row has no `|g` tail); Intercept at a table with a Naaz or Nekro seat (earlier).

## Dropped by decision (2026-10-29, user)

Last Bastion / Crimson Rebellion, the structured-diplomacy option and the turn-redo deck reserve are disabled at the product
edges (the core will panic if they are reached). No effort went into action-card pieces whose only purpose is one of these:
the deck-reserve bails of Veto and Archaeological Expedition (`Play::of` is `None` while `cards().reserve` is set; tests
`veto_declines_while_a_replay_reserve_is_installed`, `bail_archaeological_expedition_declines_while_a_replay_reserve_is_installed`),
the Last Bastion bar of Skilled Retreat, and the Crimson card counts carried in `Placed.on_card` stay as the existing documented
bails: dropped by decision.

## Gate (code commit 7483c5b9; this file is the only later change)

`tools/engine_core/verify_package.sh action_cards` (`--profile gate`, `SKIP_AB=1 SKIP_WASM=1`, `SHARDS=4`, private target dir,
`CARGO_BUILD_JOBS=3`, under the heavy lock): PACKAGE OK. layers 0 violations (371 files), lint_core 0 hits (373 files), clippy 0
in core/bridge/dual/core_sync/render and 0 errors, immutable surfaces unchanged (against `engine-core-state-base`), 41 test
suites ok, `gen-cards --check`, xtask and wasm tests ok, dual suite (6 suites, `action_cards=dual`, fail fast) ok, golden dual
`action_cards=dual` (20 games): 59462 calls, 63366 comparisons, 0 differences, hashes equal (`action_cards::announce` 395 compared,
152 bailed); `board_equivalence templates` (100 positions, 344338 answers) and `golden profile-20 20 10` (2999 positions,
24783332 answers): 0 mismatches; `bench` + `bench-v1..v5` with the area on Core 200 identical / 0 different each, `bench` on String
200/0. Also on the same binary: golden dual `driver=dual` 4564 calls, 26593 comparisons, 0 differences; the wide mask
(`driver,invasion,production,leaders,exploration,agenda,action_cards` = dual): 11754515 calls, 11884141 comparisons, 0 differences,
hashes equal. Not run: the CPU A/B, the wasm size gate, release builds, `state_size`.
