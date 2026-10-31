# W lane 15 resume note, branch `core-w-l15` (from `core-w-l11` @ `9308abed`)

Task: the items of lane 11's NOT PORTED list whose host windows already exist (Courageous To The End nested, Scramble Frequency), the
neutral-side scenario tests, and, for the cards whose hosts are windows of the tactical action, the vote and the transaction (not ported
here, by decision), the preparation below. Pure refactor: `RULES_VERSION`, serde and fixtures are unchanged. Rows for
`docs/ENGINE_KNOWN_ISSUES.md` are in its section "W lane 15" (W15-design-01 to 04). The files of `core-d2-f-live2` (`driver/live.rs`,
`driver/step.rs`, `state/pending.rs`, `session.rs`) are not touched.

## Ported (core half + Ask/Opt/Subtype + byte-exact renderer + `dual_window!`/`dual::write` + lockstep test, each failing on the pre-W15 code)

| item | where | tests |
|---|---|---|
| Courageous To The End played from a window (`Fx::nested`): its two dice are `RollPurpose::Ability(COURAGEOUS_ROLLS)` ledger rows, kept by `announce::play_inline` for a play inside a window (`Play::ability_roll`); the opponent's casualty question has the string's display context (`dual_window::courageous_hit`: the `courageous_casualty` branch of `drive_windowed` never ran for a question asked inside an emission, because the `nested_facts` branch took it, and it dropped the context when the battle was not in `active_space_combat`; the holder is read from the handoff `last_ship_destroyed` of the position the question was asked in) | `rules/action_cards/{effects,announce}.rs`, `dual_window.rs` | `combat::tests::courageous_to_the_end_played_in_a_window_of_the_fight_runs_in_the_core_in_lockstep` (3 hand/faction variants x 10 seeds x 2 fleets, bail-as-failure on the card, replaces `bail_courageous_to_the_end_is_counted`), core `action_card_effects::courageous_to_the_end_hands_its_two_dice_to_the_host_as_the_roll_of_an_ability` |
| Scramble Frequency: registered play `scramble` in `ti4_engine_core_factions::cards` (every die of `last_reroll_player`'s staged set drawn again, forced, ledger `Reroll` rows as the string's dice history); Fire Team stays as registered (its redraw keeps no rows); one owner per card | `cards/mod.rs` | `cards::tests::{scramble_frequency_draws_every_die_of_the_roller_again_and_keeps_each_redraw_in_the_ledger, scramble_frequency_without_a_roller_or_staged_dice_draws_nothing, the_redraw_of_fire_team_keeps_no_ledger_row, one_owner_per_card_the_registered_plays_are_never_cards_the_area_plays_nested}` |
| Hosts stage their dice before the event and re-read them after it, `last_reroll_player` set: the anti-fighter barrage (`finish_side`: `mirror`, `read_back`, redraws in `drawn`/`redraws`/ledger as the commander's), the space cannon offense (`reroll_windows`: per-gunner sets in the string's order, hits of every gunner read from the faces that remain, `first_draws()` for the dual and the aftermath head), the bombardment (new window `rules::invasion::bombard_roll::BombardRolled`; `InvasionWindow::open_bombard_rerolls`, `dual_window!` `invasion::bombard_rerolls`, gated by test faces, a Heart of Ixth and the invader's Jol-Nar commander, named) | `rules/combat/{barrage,space_cannon_offense}.rs`, `rules/invasion/bombard_roll.rs`, `driver/windows/aftermath.rs`, `cannon_dual.rs`, `invasion.rs`, `dual_window.rs` | lockstep: `combat::tests::scramble_frequency_played_against_the_barrage_runs_in_the_core_in_lockstep`, `cannon_prelude_tests::{scramble_frequency_played_against_the_guns_draws_their_dice_again_in_the_core_in_lockstep, scramble_frequency_against_two_gunners_agrees_for_each_in_lockstep}`, `invasion::tests::{the_reroll_window_of_a_bombardment_runs_in_the_core_in_lockstep, a_bombardment_of_the_jolnar_commander_keeps_the_reroll_window_on_the_string_and_says_why}`; core (`w15_scramble_hosts.rs`, a stub for the card): the barrage with both sides rolling, the cannon with two gunners, the bombardment window |
| Neutral-side scenarios for every faction across from the neutral force (22 factions, single windows and composed machine, bail-as-failure), sustaining neutral fleets against Assault Cannon / Non-Euclidean Shielding / Letnev | `combat.rs` tests | `a_neutral_side_fights_in_the_core_for_every_faction_across_from_it`, `a_neutral_side_with_dreadnoughts_fights_attackers_with_cannons_shields_and_munitions_in_lockstep` |
| Found by those scenarios and fixed: every fight with a Cabal seat bailed at the end announcement of the composed machine (the Cabal's participation mark `cabal:devour:{system}` is cleared by a stateful ability the lockstep does not follow); the core announcement clears it (`end_announce::clear_participation`) | `rules/combat/end_announce.rs`, `combat.rs` (`announce_end` note check), `dual_window::END_ANNOUNCE` | `combat::tests::a_fight_of_a_cabal_seat_clears_its_participation_mark_in_the_core_announcement`, core `combat_flow::the_end_of_a_combat_clears_the_participation_mark_a_side_holds_and_only_that` |

Skipped by decision: (3) neutral PDS / space dock / mech lockstep tests (they are Thunder's Edge placements).

## Remaining, with the precise reason

* **Yin Devotion in the composed combat machine** (found by the neutral scenarios, true of every Yin fight, neutral or not): a hit the producer places
  (`Pend::producer_assigns`, built by `Machine::produced_queue` from `ProduceHits`) has no question in `Machine::pending_choice` (it returns
  `None`), so the machine bails `Bail::Unported` when Devotion produced a hit. The single windows (`ProduceHits`, `AssignHits`) carry it.
  Needs: the machine asks the producer (actor = producer) which of the victim's ships dies, with the hit context of the cause `devotion`
  (producer = the asker), a lone candidate going through `lone_casualty`, and for a neutral victim the same question (the producer chooses,
  `neutral_due` already leaves a producer-assigned hit to the producer). Pinned by `a_yin_seat_that_uses_devotion_keeps_the_composed_machine_on_the_string_and_says_why`.
* The bombardment reroll window is gated for the Jol-Nar commander and the Heart of Ixth (a question per die for the roller: the barrage
  step has the commander's walk, the bombardment window does not), the cannon offense for the same two (`cannon_dual::unported`).
* Everything of lane 11's list that is a tactical-action, vote or transaction host: below.
* Thunder's Edge, the dropped factions and features: unchanged (out of the agreed content).

## Prepared for the island lanes (not ported here, by decision)

The cards whose hosts are windows of the tactical action, the vote and the transaction wait for the lanes that port those windows
(`core-m6-tactical`, the trade and agenda lanes). Each stays a counted bail named by its card (`Unknown::*` in `driver.unknown()` or
`unported ability ... (card X)` from `Recording::unported`). What each needs, in the order of the window it is played in. "Judge" is the
core answer of `reaction_slot::can_matter` (`rules::action_cards::{reaction_can_matter, cost_is_payable}`) that today is `Unknown::*`
or absent. "Facts" are the adapter's handover (`SeatPlayMore`, `SharedPlay`: handed in already unless named).

| card | printed window (generic reaction row) | host window that must emit it | facts it needs | missing core judge |
|---|---|---|---|---|
| Upgrade | `SYSTEM_ACTIVATED` after, `AfterYouActivateSystemWithYourShips` (actor is you) | the activation step of the tactical action (`rules/tactical/activate.rs` emits the event inside the tactical window of `core-m6-tactical`; the reaction table of the adapter needs that window followed and lent the board) | the plain dreadnought the seat places (`SeatPlayMore.dreadnought`, handed in), the reinforcements left (board count) | none: `reaction_can_matter` already answers `Upgrade` (an own cruiser in the system). The only gap is the host |
| Decoy Operation | `SYSTEM_ACTIVATED` after, `AfterOtherActivatesSystemWithYourStructures` | the same activation step | the planets the holder controls in the active system, the holder's ground forces anywhere on the board (both from the board), the "up to 2" stop option | none (`_ => true`); the guard "your structures in the activated system" is the generic reaction table's |
| Harness Energy | `SYSTEM_ACTIVATED` after, `AfterYouActivateAnomaly` | the same activation step | `replenish_events` (a Cabal/Firmament/Trade Agreement table: handed in) | `reaction_can_matter(HarnessEnergy)` is `None` -> `Unknown::HarnessOrInfiltrate`. The core needs `system_is_anomaly(core, system)` (the tile's anomaly flags from the map slice, plus the gravity-rift hook `hooks_movement::extra_gravity_rifts` of the string, which the movement kernel holds as a hook) |
| Infiltrate | `PLANET_CONTROL_GAINED` when, `WhenYouGainPlanet` | the control step of the invasion (`rules/invasion/control.rs:143` emits it; the window is core but not lent the stream/map of a play, and the structures standing on the planet are the previous holder's at that moment) | the PDS and space dock the seat places (`SeatPlayMore.pds`, `.spacedock`: handed in), the frame `last_control_gained` (planet and previous holder) | `reaction_can_matter(Infiltrate)` is `None` -> `Unknown::HarnessOrInfiltrate`: from rules version `RULES_INFILTRATE_FIX` it is `infiltrate_swaps(...).is_empty()` false: another seat's PDS or space dock on the planet AND the holder still has that plastic (31.4, counted as it goes). The core needs `infiltrate_swaps(core, scope, who, planet)` over the board and the reinforcement counts |
| Bribery | `VOTES_CAST` after, `AfterSpeakerVotes` (anyone: the engine offers it after any voter, see the string `bribery` doc) | the vote window of the agenda area (the cast of one voter) | the seat's spendable goods (trade goods; the Keleres agent's commodities at a Keleres table, dropped) | `can_matter` returns `Unknown::BriberyGoods` from rules version 23 for the card: `cost_is_payable(Bribery, goods)` is `goods >= 1`, but the goods are `supply::potential_goods` (the string) and the core has no judge of the seat's potential goods through a payment window (`economy::spendable_goods`, hooks of Hacan/Nekro/Titans... pay differently). The extra votes ride on the holder's recorded vote (as Distinguished Councilor): the vote window must hold the holder's vote not yet banked |
| Lie in Wait | `TRANSACTION_RESOLVED` after, `AfterNeighborsTransact` (anyone) | the transaction window of the trade area (resolution of a deal) | `SharedPlay` + `Facts.traders: [Option<Seat>; 2]`: the neighbours who resolved a transaction this round (`transactions::neighbours_who_transacted`: the `transactions_this_round` pairs against the map adjacency of the homes), a neighbour that traded twice counts once, the hands of the two | none for the card (`_ => true`); the core needs the query `neighbours_who_transacted(core, seat)` over `TradeState` and the map |
| Crash Landing | `SHIP_DESTROYED` when, `WhenYourLastShipIsDestroyed` (the `last` fact the hit windows compute before the event) | the destroy/assign/absorb windows of the combat area (these already emit `SHIP_DESTROYED`; what is missing is a host that places the saved ground force: no combat window places units on a planet) | the planets of the active system other than Mecatol Rex (by name in the tile data), the ground forces in the space area, "coexistence" if the planet holds other players' units (Thunder's Edge coexisting planets: out of the agreed content, so the card is played only onto a planet without them) | none (`_ => true`). The gap is the placement window of the card and the handoff `last_ship_destroyed` read on a lent stream |
| Experimental Battlestation | `SHIP_MOVED` after, `AfterActivePlayerMovesIntoActiveSystem` | the movement step of the tactical action (`rules/tactical`) | the galaxy (the seat's space docks in or next to the system: adjacency from the map slice), three dice at 5 (the stream), the hits absorbed by the active seat (`Absorb`, exists) and the dice kept as `RollPurpose::Ability` rows of the ledger like Courageous To The End (W15) because the string rolls them inside the card's effect | none (`_ => true`) |

All eight need the activation, control, vote, transaction or movement windows to be followed as nested hosts, which means: the host window's
`nested_fp` carries `Slice::Pending` and the cards' footprint (`CARD_PLAY_FP`), the resolver's links (`driver::children::register`) and,
for the dice-rolling Battlestation, the dice stream (`with_seeds`). When a lane ports a host, flip `Fx::nested` for the card only if every
fact it reads is in `SeatPlayMore`/`SharedPlay`, record the dice as `RollPurpose::Ability` (see `announce::play_inline`), add the lockstep
test with bail-as-failure on `(card X)`, and add the card to the `the_b6d_cards_are_not_played_from_inside_another_window` list's complement.

## Checks (cheap only; private target `/root/deploytmp/core-w-l15`, `CARGO_BUILD_JOBS=2`, one cargo at a time)

Run (all green): `cargo test -p ti4-engine --lib` whole (4896 passed, 4 ignored; lane 11 had not run the whole lib) normal and under
`TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1` (4896 passed); `ti4-engine-core --lib` (452), `ti4-engine-core-factions --lib` (129), the touched
`ti4-engine-core-tests` files (`w15_scramble_hosts`, `combat_flow`, `action_card_effects`, `combat_barrage`, `combat_cannon`); `check_layers.py core`
(0 violations); `lint_core.sh` (0 hits).

Mutation checks (scratch worktree `core-w-l15-mut` at `ea28bc90`, target `/root/deploytmp/core-w-l15/mut-target`, runner and specs in
`/root/deploytmp/core-w-l15/mut/{runmut.py,spec_w15.py,spec_l11.py,spec_fix.py}`, run by a Haiku runner agent): 48 mutants of the new core halves
(barrage, space cannon offense, `bombard_roll`, `end_announce`, `Play::ability_roll`, `Fx::nested`, `cards::{scramble,redraw}`, the ledger row of
`play_inline`, `courageous_hit`): 47 killed, 0 survived, 1 compile error (a malformed mutant of mine; its fixed rerun is killed). Lane 11's spec
rerun on the post-lane-11 code (its survivor tests, which had not been re-measured): 29 killed, 1 survivor (`front.hits > 0` in
`Machine::neutral_due`, the equivalent mutant lane 11 already named: the flow never returns with a zero front), 1 compile error (a stale mutant
whose text no longer fits the moved `query::neutral::neutral_casualty`; its fixed rerun is killed; the Courageous-not-nested mutant against the lockstep test is killed too). Equivalent or not covered by design:
the `owner` of the ability-dice ledger row of `play_inline` is not read by the string's comparison (no counterpart), so a mutant of it has no
test that can see it; the redraw filter `purpose == Reroll` of the hosts' `read_back` is redundant by construction (only the event's cards write rows
between the mark and the read).

Regression tests that fail on the pre-W15 code (seen failing while the lane was built): the Courageous lockstep failed with the card named as a
bail (the old `Fx::nested`), then with `nested.ability_dice` `[]` against `[(None, [2, 3])]` (row written by the play window) and with the casualty context
`<absent>`; the Scramble tests fail with the card unregistered (named bail) and with a no-op card (23 differences); the Cabal composed fight showed
16 bailed windows before `clear_participation`; the bombardment reroll window test requires one compared `invasion::bombard_rerolls` window, which
did not exist; the Legionnaire bombardment lockstep test caught a wrong event text of the new window while it was written.

Not run (a full zero-bail playthrough is not possible yet): golden / wide dual sweeps, benches, soak, mutation ratchet, census, workspace clippy.
