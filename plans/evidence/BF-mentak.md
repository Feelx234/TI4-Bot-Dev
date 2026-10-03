# BF-mentak: Mentak Coalition

File: `crates/ti4-engine/src/factions/mentak.rs`. Only other edit: the `mc` hard-code in
`production::trade_good_worth` (base is now 1; the hook is the single source).

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| tech | `mc` | done | `mirror_computing_doubles_trade_goods_through_both_payment_paths` |
| unit | `mentak_flagship` | done | `fourth_moon_stops_other_players_ships_sustaining` |
| unit | `mentak_mech` | done | `moll_terminus_stops_other_ground_forces_on_its_planet_sustaining` |
| unit | `mentak_cruiser3` | done, claimed (commit 083ff72b route): reachable through production and research | `the_corsair_passes_blockades_only_towards_other_players_non_fighter_ships` (through `MovementRules::path_from_ship`), `the_tables_grace_makes_the_corsair_the_cruiser_ii` |
| ability | `ambush` | done | `ambush_rolls_each_chosen_ship_against_its_own_combat_value`, `ambush_declined_unavailable_or_not_mentak_changes_nothing` |
| ability | `pillage` | **done, claimed (2026-10-03)**: double trigger fixed, all rules gain sites announce (see Final pass) | `pillage_takes_a_trade_good_from_a_neighbour_with_three_or_more`, `..._may_take_a_commodity_instead_...`, `pillage_also_follows_a_resolved_transaction`, `pillage_needs_three_goods_a_neighbour_and_no_promise_of_protection` |
| tech | `so` | done | `salvage_gains_a_trade_good_for_winning_or_losing_and_may_rebuild_when_winning`, `salvage_does_nothing_without_the_technology_or_after_a_draw` |
| promissory | `pop` | done (see Rules questions on the ACTION step) | `pillage_needs_...` (bar), `promise_of_protection_returns_when_its_holder_activates_a_mentak_system` |
| leader | `mentakagent` | **done, claimed (2026-10-03)** | `suffi_an_draws_an_action_card_each_after_pillage` |
| leader | `mentakcommander` | done | `the_commander_unlocks_with_four_cruisers_on_the_board`, `the_commander_takes_a_note_the_opponent_chooses_from_their_hand`, `the_commander_is_not_offered_locked_or_when_the_opponent_has_no_note_in_hand` |
| leader | `mentakhero` | done | `ipswitch_replaces_each_other_players_destroyed_ship_for_the_combat`, `ipswitch_is_not_offered_locked_declined_or_to_a_non_participant` |
| breakthrough | `mentakbt` | **done, claimed (2026-10-03)**: `unit_form_override` plus the `BREAKTHROUGH_GAINED` flip | `the_tables_grace_makes_the_corsair_the_cruiser_ii`, `the_tables_grace_needs_the_breakthrough_the_upgrade_and_a_mentak_seat`, `gaining_the_tables_grace_flips_the_cruiser_iis_already_on_the_board`, `the_tables_grace_does_nothing_without_cruiser_ii` |
| regression | no Mentak seat | done | `a_game_without_a_mentak_seat_is_offered_no_mentak_ability` (every event the module listens to, plus the `mc`/sustain hooks, on a Sol/Hacan game: nothing asked, state unchanged) |

Design notes:

* Pillage listens to `TRADE_GOODS_GAINED` and `TRANSACTION_RESOLVED`. The latter carries no parties,
  so the parties are the last entry of `GameState::transactions_this_round` (pushed when a deal
  resolves, `transactions.rs`). Neighbour test is `transactions::are_neighbours`, which needs a map:
  with no galaxy nobody is a neighbour. The taken commodity becomes a trade good (21.5). The take does
  not itself emit `TRADE_GOODS_GAINED` (no Pillage chain).
* Suffi An has no event to hang on ("after Pillage is used"), so it is asked inside the Pillage effect
  after the take; failure after the take restores the state.
* Salvage: the trade good is one mandatory ability, the ship another optional one (so the card's
  "also" is respected). Producing = pay the unit cost in resources (fighters round up to 1) and place
  the ship in the system; the system's PRODUCTION value is not read. Offered only for ship base types in
  `SPACE_COMBAT_ENDED.destroyed` that `production::buildable_for` allows, the box holds, and the owner
  can afford.
* Ipswitch: purge at `SPACE_COMBAT_STARTED` writes `faction_marks["mentak:hero:<system>"]`; each
  `SHIP_DESTROYED` of another player's ship in that system places one ship of that base type
  (`action_cards::place_units_counted`, supply capped); `SPACE_COMBAT_ENDED` clears the mark.
* S'ula Mentarion: the opponent chooses the note (their own question); several opponents: the
  commander's owner chooses whom. No event is emitted, so the note taken is not announced.
  Notes in hand = held and not faceup.

## Review fixes

* B1: `pillage` and `mentakagent` unclaimed (see Items). S4: `mentak_cruiser3` unclaimed.
* S3: S'ula Mentarion can take the opponent's own Support for the Throne (`promissory::available_support`
  counted as in hand) and transfers it with `promissory::receive` (scores the point), others with
  `promissory::take`. Test `the_commander_can_take_support_for_the_throne_and_scores_it`.
* After the fixes: `factions::` 195 passed, 1 failed (`arborec::...mitosis_places_an_infantry_on_the_planet_you_choose`,
  another agent's file); mentak tests all pass; clippy has no mentak.rs warning.

## Routes update (commits 083ff72b, c5632448)

* The Table's Grace: `unit_form_override` returns `mentak_cruiser3` for base `cruiser` when the seat is
  Mentak, holds `mentakbt` and the chosen form is `cruiser2`. Still not covered, so `mentakbt` is not
  claimed: (1) gaining the breakthrough does not run `technology::apply_unit_upgrades`, so Cruiser IIs
  already on the board stay Cruiser IIs until the next research (needs a call where
  `seat.breakthrough` is set: `breakthroughs.rs` ~277, `thunders_edge.rs` ~315, `legendary.rs` ~234,
  `action_cards.rs` ~4414); (2) `action_cards::placed_unit_id` (~4684, used by `place_units_counted`
  and every card placement) ignores the override. My Salvage and Ipswitch placements call
  `apply_unit_upgrades` afterwards to compensate; other cards that place a cruiser would place a
  Cruiser II.
* Pillage / Suffi An stay partial. `TRADE_GOODS_GAINED` now fires from the Trade primary/secondary,
  relic gains and exploration; it does not fire at the `trade_goods +=` sites BF-F3.md lists:
  action_cards.rs (7), agenda_effects.rs (4), combat.rs, diplomacy/transfers.rs, draft.rs,
  faction_abilities.rs, faction_techs.rs, game.rs:1933/4549/4576 (including income), laws.rs,
  legendary.rs, promissory.rs:511, transactions.rs:476 (covered by the transaction half instead).
  Staged events also depend on `flush_staged_events` being called after the acting window.
* `TRANSACTION_RESOLVED` now supplies `proposer`/`partner`; Pillage reads them (test updated).
* No new decision sites (still only `ask_among`).
* Commands: `cargo test -p ti4-engine --lib -- factions::mentak` 23 passed; clippy: no `mentak.rs`
  warning; rustfmt applied.

## Final pass (2026-10-03)

* **Double trigger.** A resolved transaction stages `TRADE_GOODS_GAINED` (source `transaction`) per
  gaining party in `transactions::resolve`, then `game.rs` emits `TRANSACTION_RESOLVED` at once; the
  staged gains are flushed at the start of the next `Game::step`. Both Pillage abilities qualified.
  Fix (mentak.rs only): on `TRANSACTION_RESOLVED`, a party with a pending staged `transaction` gain
  is left to that event (`transaction_gain_pending`); a party with no gain (note-only deal) is still
  offered from `TRANSACTION_RESOLVED`. Diplomacy promise settlement (`diplomacy/promises.rs` ->
  `transactions::resolve`) emits no `TRANSACTION_RESOLVED`, so the gain event covers it. Reads the
  private `supply` staging rows by prefix (`the_staged_event_prefix_matches_supply` guards drift).
  Tests: `a_transaction_that_moves_goods_offers_pillage_once`,
  `the_flush_after_a_transaction_resolves_pillage_exactly_once` (real `flush_staged_events` through an
  armed resolver, one scripted answer: a second offer would fail it).
* **Flush.** Staged events are flushed by `announce_staged_ground_events`, the first thing in
  `Game::step` and after faction windows (`announce_staged_destructions`). Pillage therefore resolves
  before the next decision is applied. The gain is visible in state before then.
* **Gain-site audit** (`trade_goods +=`/`=` in `crates/ti4-engine/src`, non-test): every site either
  calls `gain_trade_goods_{announced,via,staged}` or `note_trade_goods_gained` (action_cards x7,
  agenda_effects x4, combat, draft, exploration, faction_abilities, faction_techs, game.rs extreme
  duress, laws, legendary, promissory, relics, strategy_cards Trade, technology, transactions, Saar,
  Muaat, Mentak). Not announced: `game.rs` ~4798 and ~4825, the experiment-only
  `STATUS_INCOME_TRADE_GOODS` / `round_income_trade_goods` (zero by default; not printed rules);
  `diplomacy/transfers.rs:75` only builds `Terms` (settled via `transactions::resolve`). `agenda_effects.rs:1187`
  sets goods to 0 (a loss).
* **Pillage's take** stages `TRADE_GOODS_GAINED` for the Mentak player (source now `pillage`, was
  `mentakagent`); the owner is never a target, so no chain.
* **Table's Grace.** Both earlier gaps are closed: `action_cards::placed_unit_id` now asks
  `unit_form_override`; gaining the breakthrough is `BREAKTHROUGH_GAINED` (announced by
  `Game::announce_gains` at the next step), on which a Mentak ability runs `apply_unit_upgrades`.
* No new decision sites. Commands: `cargo test -p ti4-engine --lib -- factions::mentak` 28 passed;
  `-- factions::` 333 passed, 0 failed, 1 ignored; clippy no `mentak.rs` warning; ledger
  `mentak    12/12 implemented`. First builds hit rustc "out of memory" (concurrent builds); reran.

## Hook requests

1. **`mentakbt` The Table's Grace and the Corsair's acquisition.** `mentak_cruiser3` has no
   `requiredTechnology`, so `ti4_content::units::unlocked_upgrade` never returns it; every placement
   site (`production.rs` ~1659 `buildable_for`, `technology.rs` ~1054, `action_cards.rs` ~4664
   `placed_unit_id`, and `fleet`/supply readers) would need to ask a hook: `unit_form_override(state,
   content, sources, player, base_type, chosen_id) -> Option<UnitTypeId>` in `hooks_strategy.rs`
   (any module's `Some` wins), called after the upgrade lookup. Mentak returns `mentak_cruiser3` for
   base `cruiser` when the seat holds `mentakbt` and owns `cruiser2`. Also decide how a breakthrough is
   "held" (`Player::breakthrough` exists, `breakthroughs::holds`). Until then the Corsair never appears
   in a real game and `mentakbt` stays unclaimed.
2. **Trade-goods-gained coverage** (coordinator already knows): 31 gain sites still `+=` directly; only
   `supply::gain_trade_goods_*` callers emit `TRADE_GOODS_GAINED`. Pillage after "gains trade goods"
   fires only at converted sites. Most valuable to convert: Trade primary, status-phase income,
   Hacan/relic/agenda gains.
3. **`TRANSACTION_RESOLVED` payload**: add `proposer` and `partner` (strings). Pillage works from
   `transactions_this_round.last()` today, which is right only when the push precedes the emit (true in
   `game.rs` step_trade, the push is in `transactions::resolve`), and would read a stale pair for any
   emit without a push.
4. **`payment::tests::plans_ignore_the_hard_coded_mc_technology_as_before`** (`payment.rs` ~L428)
   now fails by design: `plans` honours `mc` through the hook. The test must be rewritten to assert the
   Mentak behaviour (two goods now pay 4 influence; `plans(... 3 ...)` returns a plan with 2 goods).
   Not mine to edit.

## Decision sites to register (`tests/decision_delivery_inventory.rs`)

`every_producer_and_delivery_site_matches_the_reviewed_registry` is red until registered.
In `factions/mentak.rs`:

| Function | Count | Note |
|---|---|---|
| `ask_among` | 1 `Choice::new` / 1 `ask_seeing` | every question goes through it; `ask_one` (adds a decline) calls it. Callers: `produced_hits` (Ambush), `pillage_effect`, `suffi_an`, `salvage_effect`, `commander` (opponent, note) |

## Rules questions

* **PoP "ACTION: Place this card faceup".** `promissory::take` puts a `playArea` note faceup the
  moment it changes hands (69.3 handling), so the note is in effect on receipt and the placing action
  is not a separate step. Implemented the effects (Pillage bar, return on activation) on the faceup
  state.
* **Pillage and the taken commodity**: treated as a trade good (21.5 applies to a commodity changing
  hands); not verified against a Mentak-specific ruling.
* **Pillage after a transaction with the Mentak player as a party**: the other party is a neighbour
  who resolved a transaction, so it is offered; one Pillage per event even if both parties qualify
  (printed text is "1 of your neighbors").
* **Salvage "produce"**: cost paid, PRODUCTION value and blockade (68.10) ignored; a draw is neither
  winning nor losing (no trade good).
* **Commander "your opponent"**: with several opponents (3+ player combat) the winner picks one.
* **Ambush value**: printed combat value, no combat modifiers (not a combat roll).
* **Hero timing**: ships are placed as each destroyed ship is announced, so they join the ongoing
  combat; "that you are participating in" read as attacker or defender.

## Commands and results

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- factions::mentak` | 20 passed, 0 failed |
| `cargo test -p ti4-engine --lib -- factions:: production::` | 184 passed, 0 failed, 1 ignored |
| `cargo clippy -p ti4-engine --all-targets` | no warning in `mentak.rs`; 15 lib / 18 lib-test warnings elsewhere (other files) |
| `rustfmt --edition 2024 crates/ti4-engine/src/factions/mentak.rs` | clean |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib: 1633 passed, 1 failed (`payment::tests::plans_ignore_the_hard_coded_mc_technology_as_before`, hook request 4); `decision_delivery_inventory`: 1 failed (`every_producer_and_delivery_site_matches_the_reviewed_registry`, the `ask_among` site above); other targets ok |

Not run: `ti4-sim`, training (forbidden).

## Ledger line

`mentak    12/12 implemented`
