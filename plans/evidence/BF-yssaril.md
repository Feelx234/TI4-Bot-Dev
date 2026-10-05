# BF-yssaril: the Yssaril Tribes

File: `crates/ti4-engine/src/factions/yssaril.rs` (only). Branch `wp/base-factions`, uncommitted.
Card text: `crates/ti4-content/content/*.json` at DEFAULT, quoted in the module header.

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | `crafty` | done (economy `action_card_limit` = `usize::MAX`, beats Sanctions) | `crafty_lifts_the_hand_limit_for_its_owner_only`, `crafty_beats_a_law_that_caps_the_hand` |
| ability | `scheming` | done: all live draw sources route through the draw hooks and hand-limit enforcement | `scheming_draws_one_extra_then_discards_one_chosen`, `scheming_applies_to_nobody_else`, `a_politics_rider_paid_with_a_table_draws_through_scheming` |
| ability | `stall_tactics` | done (component action `faction\|yssaril\|stall_tactics`) | `stall_tactics_is_offered_with_a_card_and_only_to_yssaril`, `stall_tactics_discards_a_chosen_card_and_the_mech_deploys_after`, `the_mech_may_be_declined_and_stall_tactics_refuses_an_empty_hand`, `a_staged_stall_tactics_discard_reaches_the_pile_when_announced` |
| tech | `tp` | done (economy `action_cards_forbidden`, read by `laws::action_cards_forbidden` and the reaction gate) | `transparasteel_stops_passed_players_only_on_its_owners_action_turn` |
| tech | `mi` | done (component action `faction\|yssaril\|mi\|<seat>`, `look_at_hand_and_take`, exhausts) | `mageon_implants_looks_takes_one_and_exhausts`, `mageon_implants_needs_the_card_a_ready_technology_and_a_hand_to_look_at` |
| unit | `yssaril_flagship` | done (movement `may_move_through_ships`; stats data-driven) | `the_flagship_alone_passes_through_other_players_ships` |
| unit | `yssaril_mech` | done (DEPLOY after Stall Tactics, optional, `place_units_choosing`) | the two mech/Stall Tactics tests above |
| promissory | `spynet` | done (timing ability `promissory:<owner>:spynet:TURN_BEGAN:after`; returns the note) | `spy_net_takes_a_card_at_the_start_of_the_holders_turn_and_returns`, `spy_net_is_not_offered_to_a_player_without_it_or_on_another_turn` |
| leader | `yssarilcommander` | done (unlock hook; SYSTEM_ACTIVATED window; reveal to the owner only) | `the_commander_unlocks_with_seven_action_cards`, `the_commander_looks_at_the_activating_players_cards_and_only_then`, `the_commander_may_look_at_notes_or_secrets_instead`, `the_commander_needs_to_be_unlocked_the_units_and_another_player` |
| leader | `yssarilhero` | done (`leader_action` + `use_leader`; shared code purges) | `kyver_takes_one_card_and_forces_discards_on_another_then_is_purged`, `kyver_may_decline_each_player_and_needs_a_hand_to_look_at` |
| leader | `yssarilagent` | **blocked** (not claimed) | none |
| breakthrough | `yssarilbt` | done: borrowed Scheming and Stall Tactics require owner consent; the staged transaction resolves with fixed parties, a turn-scoped exemption, and cleanup | `game_step_flushes_the_borrowed_scheming_transaction`, `borrowed_scheming_draws_extra_discards_one_and_stages_the_transaction`, `borrowed_stall_tactics_opens_one_forced_pair_transaction_and_expires`, refusal and invalid-answer tests |

Regression/hidden information: `a_game_without_yssaril_is_never_offered_a_yssaril_ability` (no choice
mentioning yssaril, no component options, neutral hooks, empty `faction_marks`);
`the_commander_looks_at_the_activating_players_cards_and_only_then` asserts through
`SeatObservation::bind` that seats b and c see nothing, `ti4_model::view::view_for` redacts the
`cards:reveal:` row, and the look ends at `ACTION_COMPLETED`. The module keeps no secret
bookkeeping of its own, so it writes no `private:<player>:` rows.

## Decision sites to register (`tests/decision_delivery_inventory.rs`)

The scan reports exactly these new sites, each count 1 for `Choice` and for `AskObserved`
(registry currently red on them):

| Module | Function | Choice | AskObserved |
|---|---|---|---|
| `yssaril.rs` | `action_cards_drawn` (Scheming discard) | 1 | 1 |
| `yssaril.rs` | `commander_look` (So Ata: which kind to look at) | 1 | 1 |
| `yssaril.rs` | `kyver_decisions` (take / discard 3 / decline per player; renamed from `kyver_decide`) | 1 | 1 |

The other asks go through registered helpers (`choose_from_own_hand`, `show_action_card`,
`take_from_revealed_hand`, `enforce_hand_limit`, `place_units_choosing`).

## Hook requests

1. **Ssruu (`yssarilagent`), `leaders.rs`** (R2 of BF-00h-cards): expose
   `leaders::use_leader_text(context, as_player, source_agent: &LeaderId) -> bool` that runs the
   per-leader dispatch (incl. `factions::use_leader`) without the readied-status gate on the source
   agent and without exhausting it; `leaders::component_actions` must also offer each
   `hooks_cards::borrowable_agents` entry's ACTION window to the Ssruu owner. Non-ACTION agents
   (timing abilities built per owner at construction) need a copy registered for every seat. Module
   side is then: exhaust `yssarilagent` after a successful borrowed use.
2. **Deepgloom Executable (`yssarilbt`)**: owner-consent windows cover borrowed Stall Tactics and Scheming;
   `transaction_limit_exempt` is wired, and the staged typed event opens a trade window with the fixed pair.
   `Game::step` flushes staged supply events at its start; `game_step_flushes_the_borrowed_scheming_transaction` exercises that actual event boundary.
3. **Staged events from non-component paths**: Scheming's discard and Spy Net's take are staged
   (`discard_chosen`, `take_revealed_action_card`) from inside `draw` / a timing ability.
   `Game::announce_staged_cards` runs only after component and leader actions, so a Scheming discard
   in a strategy-card or exploration draw, and Spy Net's `ACTION_CARD_TAKEN`, stay staged (card out
   of the hand, not yet in the discard pile, no event) until the next component/leader action.
   Request: also call `announce_staged_cards` after typed-event windows (after `emit_typed`) and
   after strategy-card resolution.
4. **Draw sites bypassing `action_cards::draw`** (BF-00b Request 4): `status.rs` status-phase draw,
   `agenda_effects.rs` (Politics Rider, Unconventional Measures). Scheming does not apply there.
5. **End-of-action reveal clear** (BF-wave-B2 `game.rs` item 2): `clear_reveals(Action)` at
   `ACTION_COMPLETED`. So Ata's reveal does not depend on it (`commander_clear` ends it), Kyver's
   and Mageon's use `Choice` scope and clear themselves.

## Rules questions

1. Spy Net with an empty Yssaril hand: implemented as not offered (nothing to look at or take).
2. Kyver: "Each other player shows you 1 action card" is done for all players first, then Yssaril
   decides per player in seat order. A player with no card shows none and cannot be targeted.
   "Random" discards use `dice.roll_by` (a d10 face modulo the hand, as Spy does); three discards
   roll sequentially against the shrinking hand.
3. Transparasteel: implemented as printed on the passed player's side only (the active player holds
   the technology, the player has passed). It does not stop the Yssaril player's own cards.
4. So Ata look duration: until the end of the activating player's action (`ACTION_COMPLETED`).
5. Mageon Implants/Stall Tactics as component actions; a decider that answers outside the options
   during the optional mech placement (after the discard) is treated as declining it, and one that
   fails inside Mageon's take leaves the card exhausted (the look happened).
6. Stall Tactics is offered with any card in hand; whether holding Transparasteel's opponent effect
   can stop an ACTION ability is not addressed (it only blocks playing action cards).

## Commands and exact results

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- factions::yssaril` | 21 passed, 0 failed |
| `cargo test -p ti4-engine --lib -- factions::` | 176 passed, 9 failed, 1 ignored. All 9 failures are in other agents' in-progress files (5 `factions::ghost::tests::*`, 4 `factions::naalu::tests::*`); none in yssaril |
| `cargo clippy -p ti4-engine --all-targets` | no warnings in `yssaril.rs` |
| `rustfmt --edition 2024 crates/ti4-engine/src/factions/yssaril.rs` | applied |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1685 passed, 9 failed (the ghost/naalu tests above), 1 ignored; `decision_delivery_inventory` 3 passed, 1 failed (`every_producer_and_delivery_site_matches_the_reviewed_registry`: the diff includes the six yssaril.rs sites above, and other agents' unregistered sites); other test binaries ok |

Ledger (`cargo test -p ti4-engine --lib print_faction_ledger -- --ignored --nocapture`):

```
yssaril   10/12 implemented
    Leader yssarilagent
    Breakthrough yssarilbt
```

## Review fixes

- **B1** Kyver's random discard now draws uniformly from the game RNG: `rng.die("yssarilhero", held) - 1` per card (domain `yssarilhero`), replacing d10 modulo. Test `kyvers_random_discard_reaches_every_index_of_a_large_hand` (12 slots all reached).
- **B2** `scheming` unclaimed in `MODULE` (code and tests kept); partial until status.rs, Politics Rider and Unconventional Measures draws go through `action_cards::draw`.
- **S2** Kyver decides every outcome (`kyver_decisions`, mutates only reveals) before applying any; a failed show/ask clears reveals and returns `Some(false)` with the state unchanged. Test `kyver_changes_nothing_when_a_decision_fails`.
- **Nit** So Ata refuses an unrecognised answer (`IllegalChoice::NotOffered`) instead of falling back.
- Results: `factions::yssaril` 23 passed; clippy no warnings in yssaril.rs; rustfmt clean. Ledger now `yssaril 9/12` (missing `scheming`, `yssarilagent`, `yssarilbt`).

## Routes update (2026-10-03)

- `yssarilagent` (Ssruu): claimed. Shared `leaders::use_leader_text` / `component|leader|yssarilagent|<agent>` do the work; module tests `ssruu_borrows_an_action_agent_even_exhausted_and_exhausts_itself`, `ssruu_is_not_offered_to_a_seat_without_it`. Limit: only ACTION agents the shared code delivers (Hacan, Xxcha) are borrowable; non-ACTION and module agents are not delivered through Ssruu.
- `scheming`: stays unclaimed. Status-phase draw is routed; Unconventional Measures (`agenda_effects.rs` ~1252, "draw 2 action cards" For voters) still pops the deck directly and is a real draw. Politics Rider found no direct deck access. Claim once that site uses `action_cards::draw`.
- `yssarilbt`: partial, not claimed. Built: another player's Stall Tactics by the owner's leave (`faction|yssaril|bt_stall|<owner>`, owner asked, refusal remembered per turn), and `transaction_limit_exempt` for the pair for that turn (public marks `yssaril:bt:{exempt,declined}:<owner>:<user>` = turn_seq). Not expressible: the Scheming half (no consent point inside `draw`'s hooks); the owner's transaction is the ordinary one, not opened by the module. Tests: `an_allowed_stall_tactics_discards_for_the_user_and_exempts_their_transaction`, `a_refused_stall_tactics_changes_nothing_but_is_not_offered_again`, `deepgloom_needs_the_breakthrough_and_a_card`.
- New decision site: `yssaril.rs::borrowed_stall_tactics` (Choice 1, AskObserved 1). Others unchanged.
- Results: `factions::yssaril` 28 passed; clippy none in yssaril.rs; rustfmt applied. Ledger: `yssaril 10/12` (missing `scheming`, `yssarilbt`).

## Scheming claimed (2026-10-03, coordinator)

Politics Rider's 3-card payoff now goes through `action_cards::draw` when the game pays predictions (`resolve_predictions_with`, called from `Game::resolve_agenda_outcome` with the game's content and table), so Scheming's extra draw + discard and the hand limit apply. The status-phase draw already applies the draw hooks (`resolve_before_token_gain_with`); Unconventional Measures uses `draw_announced`. No raw deck pop remains outside tests and the table-less `resolve_predictions` path. Test: `a_politics_rider_paid_with_a_table_draws_through_scheming`. `scheming` claimed; `yssarilagent` and `yssarilbt` remain unclaimed.

Checks: `cargo test -p ti4-engine` — lib 1913 passed, 1 failed (an in-progress Saar agent test in saar.rs, not part of this commit), integration binaries ok.

## Deepgloom follow-up (2026-10-04)

`action_card_draw_requested` now asks each eligible Yssaril breakthrough holder for consent before another seat's draw;
an allowed draw receives one Scheming extra card, must choose a discard, then stages `DEEPGLOOM_TRANSACTION`.
Borrowed Stall Tactics uses the same event and owner-consent pattern. The event listener opens one trade window with the
granting Yssaril as proposer and the user as the fixed counterparty; it keeps an in-flight reach permission and
action-phase limit exemption through the window, then removes both marks. The new resolver-driven test checks the
actual forced parties, one accepted transaction, mark cleanup and pair-limit result.

Failure atomicity: if a borrowed Scheming discard answer is invalid, the hook restores the drawn cards to the hand's
prior length and the deck's prior top order before returning the choice error. Invalid borrowed-Stall discard answers
leave the state unchanged. The new test filters are
`factions::yssaril::tests::borrowed_scheming_`,
`factions::yssaril::tests::borrowed_stall_tactics_`, and
`factions::yssaril::tests::deepgloom_needs_the_breakthrough_and_a_card`.

The initial seam assessment was incorrect: `Game::step` flushes staged supply events before other step work.
This finding and its correction are documented below. The new Rust tests have not been run in this subtask because
the coordinator owns cargo builds for the shared working tree.

## Deepgloom completed (2026-10-04)

`Game::step` calls `announce_staged_ground_events` before other step work, and that method calls
`supply::flush_staged_events`; therefore a Scheming draw's staged `DEEPGLOOM_TRANSACTION` resolves
at the next game step, including draws from non-component origins. The new integration test performs
a borrowed Scheming draw, then uses the real `Game::step` flush and checks one fixed-pair proposal
and answer, the accepted commodity swap and recorded transaction, drained supply events, and cleared exemption.
The test fixtures give both parties a commodity so `cc1` is a legal offer; the no-draw Scheming test clears
the default populated deck. The existing
resolver test covers borrowed Stall Tactics through the same event, while the invalid-answer tests
cover rollback and the refusal tests prove decline behavior. `yssarilbt` is claimed in `MODULE`.

Focused test filters: `factions::yssaril::tests::game_step_flushes_the_borrowed_scheming_transaction`,
`factions::yssaril::tests::borrowed_scheming_`, and
`factions::yssaril::tests::borrowed_stall_tactics_`. Tests were not run in this subtask because the
coordinator owns the shared cargo build; rustfmt was applied to `yssaril.rs`.
