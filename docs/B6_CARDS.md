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

## B6d: the remaining 33 cards (2026-10-29, branch core-b6-action-cards after the merge of core-b4-trade)

Infrastructure (all in the core, no panics): `kernel::stage::Child<W>` (a window run inside another window's step: `step` returns
`Ran::Asks / Pass / Done(window)`, `ask_of`, `answer`; tests `child_tests`), `rules::production::pay::PayBill` (the whole
question-driven payment loop over `PayStep`, the string `production::pay`; tests `bill_tests`), `Runner::with_sx` (an `Sx` over
an inline runner, so a play with a child window is stepped the same inside an effect), `Play` takes the kernel
(`Play::begin(sx)` / `choose(sx, pick)`), `Research::researchable_for` / `research_chosen` (a card researches like
`technology::research`), the neutral owner in the owner table (`Ctx::with_extra`, kept by `refresh`), the map lent to the play
window (`Sources.galaxy` + `Slice::Map`, `Fx::needs_map`), `Facts` in the play `Env` (neutral seat, unit forms of a destroyer,
dreadnought, mech, PDS and space dock, rules version, tech and unit id order, roster, Reveal Prototype's candidate set, the
traders of Lie in Wait, the Fracture facts, the replenish-events flag).

PORTED (core effect in `rules::action_cards::effects::Play`, lockstep test in `ti4-engine/src/action_cards_b6d_tests.rs` through
the announce window under three deciders, mutation-checked by Haiku runners in scratch worktrees): Pirate Contract x4, Pirate
Fleet, Mercenary Contract, Ghost Ship, Signal Jamming, Upgrade, Refit Troops, Decoy Operation, Ghost Squad, Harness Energy,
Brilliance, Impersonation, Plagiarize, Focused Research, Divert Funding, Reveal Prototype, Lie in Wait, Bribery, Infiltrate,
Crash Landing, Exploration Probe (26 of the 33 cards).

Conditions inside ported cards that stay string (each a `None` of `Play::of`, `unsupported`, or a bail of the adapter, a test each):
a readied Keleres agent that could be offered before a payment (dropped by decision: Keleres); a Deepwrought, Keleres or Mentak
seat for a card that gains a technology (dropped by decision / the Corsair's form); a Deepwrought holder of Crash Landing (dropped
by decision); Harness Energy while a replenishment causes events the core does not hold (a Cabal, a siphoning Firmament plot,
a Trade Agreement in play); Brilliance when the Fracture enters play (the ingress questions; the roll is the Fracture stream's and
is compared); a Naaz mech for Refit Troops; Infiltrate below rules version 1; Ghost Ship and Signal Jamming played without a map;
a deck reserve for Impersonation (dropped by decision). From inside another window (a reaction slot) the cards of this package are
the string's (`Fx::nested`, the census names them).

Exploration Probe (`Fx::Probe`): the token pick is the frontier tokens in a system with a ship of the holder or next to one (the
adapter lends the galaxy when the string has one; without it only the systems with a ship, a test each), then the string
`exploration_probe` -> `explore_with(.., FRONTIER, None)` as the window: the token is spent first (a deck that has run out loses it
and logs no row, whatever the string's comment said; a test pins it), the top card is drawn, its log row opened by the core (the
words of the row, `exploration::row_words`, are written by the adapter from the holdings the core reads before and after) and the
card resolved. Which resolutions of the frontier deck ask or touch what (checked against `resolve_instant`, planet and system None):
a fragment (no question); the three entropic fields and the Keleres Ship (command tokens: one pool question each from rules
version 12, the strategy pool before, then trade goods; no `TRADE_GOODS_GAINED`, the string explores with no timing handle); Dead
World (a relic, the Shard scores); Derelict Vessel (a secret, the `HandLimit` child window); Enigmatic Device (kept in the play
area); Lost Crew (two action cards, plain draw only); Merchant Station (the `Reward` child window of `explore_windows`: replenish or
convert, the conversion offered to a seat without commodities below version 19); Gamma Relay (the gamma token in the ACTIVE system
of the state, nothing without one); Ion Storm and Mirage (nothing: they need "this system", which `explore_with` does not pass — the
string's own shortcoming, pinned by `exploration_probe_resolves_every_card_of_the_frontier_deck_in_the_core_window`; fixing it
needs a rules version). The windows of `explore_windows` that the probe never reaches (ChooseDeck, DistantSuns, IonStorm,
DominusOrb, CrownExplore, PayMechOrInfantry) all need a planet. Conditions that mark the run `unsupported` (the adapter declines
the whole play; a core unit test `bail_exploration_probe_*` and an adapter test each): a card of another deck in the frontier deck
(an attachment, a planet's reward); a Lost Crew whose draw is not plain (a full hand, Yssaril, a draw bonus); command tokens while
a Mahact fleet holds the holder's tokens; a Merchant Station's replenish while a replenishment causes events (Cabal, a siphoning
plot, a Trade Agreement). A replay reserve declines the play (`Play::of`).

DOCUMENTED BAILS left (`BAILS` in `rules/action_cards/effects.rs`; the reason of each is the constant):
Strategize x4 and Overrule (eight card kinds, each with a window and guards of its own to run as child windows), Fire Team
(ported on `core-b12` as a registered play, bb26913b; not merged here) and Scramble Frequency (a bail of `core-b12` too).
Courageous To The End and Experimental Battlestation left the list in branch `b6d-hits` (section below).
STRATEGIZE x4 AND OVERRULE (branch b6d-strategize, after the list above; core half `rules/action_cards/perform.rs`, lockstep tests in
`ti4-engine/src/action_cards_b6d_tests.rs` and `action_cards::tests::strategize_and_overrule_agree_with_the_core_play_window_in_lockstep`).
`Fx::Perform { primary }`: the pick question byte for byte (`What::Perform`, `Opt::Perform { card, holder }`, kind `strategy card`, the
`holders` detail completed by the adapter from the core's own options), then the chosen card's ability as a child window of the strategy
areas (`Child<Performed>`, an enum window over `Research`, `Leadership`, `Construction`, `Politics`, `WarfarePrimary`, `Diplomacy`,
`TradePrimary` and `HandLimit`). The guards that read the string position are the adapter's `perform::Carried` facts (`carried_of`);
the display facts of the child questions are `strategy_cards_core::Performing`.

* Carried secondaries (Strategize): Trade (the replenishment), Imperial (the secret objective draw and the hand limit), Politics,
  Technology (four resources), Leadership, Construction (both printings and the Thunder's Edge one) and Diplomacy (both printings).
* Carried primaries (Overrule): Trade (the three trade goods are staged as the string stages them), Politics, Technology (a free research
  and one for six), Leadership, Construction (the plain printings), Warfare (the plain printing) and Diplomacy (both printings; the
  announcements of the placed tokens are staged on the string state by the adapter, `stage_placed_tokens`).
* Conditions that stay string (the window marks the run `unsupported` or `Play::of` answers `None`; the adapter declines the whole
  run; a core test in `effects::tests` and an adapter test each): the Warfare secondary (`home_production`: a production window), the
  Imperial primary, the Thunder's Edge Construction and Warfare primaries (a free tactical action, the choice of a production), a replenishment that causes events (Trade,
  both abilities), a draw that is not plain (Politics), a table or rules version the research window does not carry or a research only a
  faction waiver opens (Technology), payments the production rules do not follow (Leadership), a faction effect on the placement of a
  structure (Construction), a command token in a Mahact pool (Leadership, Warfare), two seats of one faction (Overrule names the options
  of the Trade primary and the Politics speaker by faction: the whole card), and a replay's deck reserve (dropped by decision).

DOCUMENTED BAILS left (`BAILS` in `rules/action_cards/effects.rs`; the reason of each is the constant):
Exploration Probe (the exploration resolution is not one core window; ported later, see above), Fire Team (ported on `core-b12` as a
registered play, bb26913b; not merged here) and Scramble Frequency (a bail of `core-b12` too). Courageous To The End and Experimental
Battlestation left the list in branch `b6d-hits` (section below).

Layers (branch `b6d-layers`): `tools/engine_core/check_layers.py core` reports 0 violations again. The card windows of B6d ran other
areas' windows as children (`PayBill`, `HandLimit`, `Reward`, the strategy windows) and called their helpers; areas may not import
each other, so they now talk through the kernel, as ability effects and `Next::Open` do:
* `kernel::children` (L3): `Spawn` (a request for a child window, the arguments of its constructor as plain data), `ErasedWindow`
  and `Erased<W>` (the object-safe face of a `Window`, with a function that reads what the finished window did as a `Report`),
  `ErasedChild` (`Child<W>` over an erased window; `Child<W>` stays for same-area children) and `AreaLinks` (three function
  pointers on `AbilityTable::links`: start a child, `researchable`, `research`). `StructureFacts` moved here (re-exported by
  `production::strategy_cards`).
* `driver::children::register` (L5, which may import every area) fills the links; it is the only place that names the windows.
  The adapter installs it: `action_cards_view::play_table` (the plain table of `play_window` and `announce_window`) and
  `reactions_core::table()` (the table a window that follows nested activity runs on).
* The exploration writes a card plays in its own window (`draw`, `gain_fragment`, `take_frontier`, `open_record`, `play_area_card`,
  `place_wormhole_token`, `gain_commodities`, `spend_commodities`) are L3 primitives in `kernel::fx_explore`; `rules::explore`
  re-exports them.
* A play whose card needs a link (`Fx::links`: a bill, the hand limit, a research, the Merchant Station, Strategize/Overrule) on a
  table without all of them declines at its first step, before anything changed: `unsupported`, so `play_window` answers `None`
  (a counted bail) and the inline announcement marks the run unknown; never a panic. Those cards are exactly the ones that are not
  `Fx::nested` (a test checks it), so the reactions table never needs them for a nested play.

## B6d hits: Courageous To The End and Experimental Battlestation (branch b6d-hits, cut from core-b6-action-cards at 891aed47)

Both cards run a hit flow of the combat area, so they are `Fx::Courageous` and `Fx::Battlestation` of the play window with the hits
as **child windows reached through the kernel links** (`kernel::children::Spawn::{Destroy, Absorb}`, filled in `driver::children`;
`check_layers.py core` stays at 0 violations: `rules/action_cards` imports nothing of `rules/combat`). The core half is
`rules/combat/effect_hits.rs`; the play window starts the child, steps it, forwards its question and its answers (`Stage::Hitting`,
`Play::flow`). Both cards are `Fx::rolls` (the dice are the Dice stream: faces and stream positions are compared like Spy and Plague)
and need the table's links (`Fx::links`), so a table without them declines at the first step, never a panic.

* **Courageous To The End** (`Fx::Courageous`): the handoff `last_ship_destroyed` must be the holder's own ship; "your opponent" is
  the first other seat with a ship in that system *now* (seating order, as the string `combatants`), none: no dice. A ship type
  without a combat value rolls nothing. Two dice; each result at or above the printed combat value of the destroyed ship is one
  casualty. One child per casualty, `Spawn::Destroy { who: opponent, system, card, subtype, cause }` -> `combat::effect_hits::Destroy`:
  the opponent's ships (`ships_of`: a lone ship is taken unasked, as the string `choose_casualty_as`), one option per (type, damage)
  (`casualty_opts`), the question `Ask::Combat(AssignHit)` with `Origin::ActionCard(card)`, the subtype
  `courageous_to_the_end_assign_casualty`, the system as target and nothing owed; the answer removes the ship (`assign::remove_ship`:
  the ship-lost observers) and **stages** its destruction in `combat.pending_destructions` (cause `action_card:courageous`,
  `during_space_combat`). A die that finds no ship ends the card (`Report::Destroyed(false)`; the string `break`s). The `HitDetail`
  of the question (producer = the holder, `destroy: true`, cause `CourageousToTheEnd`) is the adapter's display fact
  (`action_cards_view::hit_details`), as for every assignment question.
* **Experimental Battlestation** (`Fx::Battlestation`, `What::Dock`, kind `space dock`, options the systems): the systems with a space
  dock of the holder that are the active system or next to it (the map's adjacency when the adapter lent the galaxy,
  `Fx::uses_map`/`Facts::map`; without a map only the active system, as the string), sorted by id; none: nothing rolled, one: no
  question. Three dice, hits on 5; the active seat (`meta.active`, not the holder) takes the hits: `Spawn::Absorb { who, producer: holder,
  system, hits, texts }` -> `Absorb`: the cancellations a card granted are spent (`kernel::hits::spend`), SUSTAIN DAMAGE is offered
  (`SustainOffer`, not in a combat, nothing announced), the rest assigned (`AssignHits`, cause `effect_hit`). Nobody active: the dice
  are rolled and the hits have no target.
* `Absorb` is the child hit flow of ENGINE_REACTIONS 9.2 (a sustain offer and an assignment run inside another window's step); the
  combat agent of `core-b12` owns Reflective Shielding and may reuse it (`Spawn::Absorb` with the producer as target); it is not
  ported here.
* **The announcement.** The destructions Courageous stages are announced after the card's discard (`ACTION_CARD_DISCARDED`) by
  `AbilityTable::after_play`, the hook the reactions table already carries for Direct Hit. The adapter's `play_table()` now carries it
  too (`ti4_engine_core_factions::cards::register_after_play`), so `Announce` no longer ends the run `Unknown::PendingDestructions`.
  Found by the lockstep test: the events the hook emits (`SHIP_DESTROYED`) were lost by the window that ran it (the helper services
  of the faction crate keep their own list); they travel in `Tape::emitted` and `Sx::after_play`/`run_registered_play` append them
  to the window's events.
* **Texts.** The interned texts the child windows and events carry are `Facts` of the play (`courageous_subtype`, `courageous_cause`,
  `hits: HitTexts`), computed by `action_cards_view::hit_facts`. A staged destruction that stays in the state (the plain `play`
  window, no announcement) carries the text code of the state's table; one that is announced carries the render code of the event.

Conditions that stay string (each a `None` of `Play::of`, `unsupported`, or a counted bail of the adapter; a test each):
* Courageous To The End at a table with a Naaz or Nekro seat (the units that fight from a planet are the combat area's; the same
  condition as Intercept, B6-bail-12): `bail_courageous_to_the_end_declines_at_a_table_with_a_naaz_or_nekro_seat`,
  `bail_courageous_to_the_end_with_a_neutral_opponent_or_a_naaz_seat_is_counted`.
* Courageous To The End when the only ship left to fight is a neutral owner's (the string names it the opponent after the seated
  ones and nobody chooses its loss; declined before any die): `bail_courageous_to_the_end_declines_when_only_a_neutral_ship_is_left`.
* Experimental Battlestation in a game below `RULES_FACTION_EFFECTS` and against a seat that pays on a sustain without an unlocked
  commander (the sustain offer is the combat window's from that rules version, the commander's payment only when unlocked, as
  `dual_window::sustain_offer`): `bail_experimental_battlestation_where_the_sustain_offer_is_not_the_core_windows`.
* **From inside another window (a reaction slot, a card declared there) both cards are still the string's**: they are not
  `Fx::nested` (`the_b6d_cards_are_not_played_from_inside_another_window`; the census names them as before, and core-b12's
  `bail_courageous_to_the_end_is_counted` stays true). Their reaction windows (SHIP_DESTROYED after, the movement of the active
  player) are the combat area's and run on the reactions table, which carries the links and the `after_play` hook but not the texts
  (`courageous_subtype`, `hits`) nor the combat kernel the play window lends; making that path work needs the texts in
  `PlayFacts` and the stub-resolver semantic (nothing announced) of the dock's hits inside an ability, which `Inline` does not give.
* The dice of a game whose streams the core cannot reproduce (test faces, a pending restore): `lend_rolls`, as Spy.

Tests: core `rules::combat::effect_hits::tests` (7), `rules::action_cards::effects::tests` (the `courageous_*`, `bail_courageous_*`,
`experimental_battlestation_*` group, 9), `ti4-engine-core-factions::cards::tests::the_events_of_the_after_play_hook_are_the_events_of_the_window_that_plays_the_card`,
adapter `action_cards_b6d_tests` (lockstep through the announce window under three deciders and twelve dice seeds, `(n, 0)` windows:
`courageous_to_the_end_makes_the_opponent_destroy_ships_in_the_core_window`,
`experimental_battlestation_rolls_and_the_active_seat_absorbs_the_hits_in_the_core_window`,
`experimental_battlestation_offers_the_letnev_commander_after_a_sustain_in_the_core_window`, the play-window test
`courageous_to_the_end_and_experimental_battlestation_agree_with_the_core_play_window_in_lockstep`,
`courageous_to_the_end_is_announced_on_the_plain_table_of_the_play_window` (no nested activity: the table is `play_table()`, whose
`after_play` is the only announcement), `experimental_battlestation_follows_non_euclidean_shielding_in_the_core_window` (the combat
kernel the play window lends), `action_cards_view::hit_facts_tests`, and the bail tests above). Mutation checks (Haiku runner, scratch
worktree, 48 mutations of the core halves, the adapter and the render; the first batch left six survivors, five of which got
the tests above and were killed on the re-run): all killed except one equivalent mutant,
`.filter(|v| *v > 0)` of Courageous's combat value (no unit of the tables prints a combat value of 0; the guard is parity with
`combat::hits_on`).

