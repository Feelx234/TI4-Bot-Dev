# BF-yssaril: the Yssaril Tribes

File: `crates/ti4-engine/src/factions/yssaril.rs` (only). Branch `wp/base-factions`, uncommitted.
Card text: `crates/ti4-content/content/*.json` at DEFAULT, quoted in the module header.

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | `crafty` | done (economy `action_card_limit` = `usize::MAX`, beats Sanctions) | `crafty_lifts_the_hand_limit_for_its_owner_only`, `crafty_beats_a_law_that_caps_the_hand` |
| ability | `scheming` | **partial, not claimed** (code kept; status-phase and agenda draws bypass `draw`). Done for `action_cards::draw` callers (bonus hook + `action_cards_drawn` choose-and-discard); status/agenda draws bypass `draw` (see Hook requests) | `scheming_draws_one_extra_then_discards_one_chosen`, `scheming_applies_to_nobody_else` |
| ability | `stall_tactics` | done (component action `faction\|yssaril\|stall_tactics`) | `stall_tactics_is_offered_with_a_card_and_only_to_yssaril`, `stall_tactics_discards_a_chosen_card_and_the_mech_deploys_after`, `the_mech_may_be_declined_and_stall_tactics_refuses_an_empty_hand`, `a_staged_stall_tactics_discard_reaches_the_pile_when_announced` |
| tech | `tp` | done (economy `action_cards_forbidden`, read by `laws::action_cards_forbidden` and the reaction gate) | `transparasteel_stops_passed_players_only_on_its_owners_action_turn` |
| tech | `mi` | done (component action `faction\|yssaril\|mi\|<seat>`, `look_at_hand_and_take`, exhausts) | `mageon_implants_looks_takes_one_and_exhausts`, `mageon_implants_needs_the_card_a_ready_technology_and_a_hand_to_look_at` |
| unit | `yssaril_flagship` | done (movement `may_move_through_ships`; stats data-driven) | `the_flagship_alone_passes_through_other_players_ships` |
| unit | `yssaril_mech` | done (DEPLOY after Stall Tactics, optional, `place_units_choosing`) | the two mech/Stall Tactics tests above |
| promissory | `spynet` | done (timing ability `promissory:<owner>:spynet:TURN_BEGAN:after`; returns the note) | `spy_net_takes_a_card_at_the_start_of_the_holders_turn_and_returns`, `spy_net_is_not_offered_to_a_player_without_it_or_on_another_turn` |
| leader | `yssarilcommander` | done (unlock hook; SYSTEM_ACTIVATED window; reveal to the owner only) | `the_commander_unlocks_with_seven_action_cards`, `the_commander_looks_at_the_activating_players_cards_and_only_then`, `the_commander_may_look_at_notes_or_secrets_instead`, `the_commander_needs_to_be_unlocked_the_units_and_another_player` |
| leader | `yssarilhero` | done (`leader_action` + `use_leader`; shared code purges) | `kyver_takes_one_card_and_forces_discards_on_another_then_is_purged`, `kyver_may_decline_each_player_and_needs_a_hand_to_look_at` |
| leader | `yssarilagent` | **blocked** (not claimed) | none |
| breakthrough | `yssarilbt` | **blocked** (not claimed) | none |

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
2. **Deepgloom Executable (`yssarilbt`)**: (a) `transactions.rs` must call
   `hooks_cards::transaction_exempt_from_limit` (still not wired); (b) the "allow another player to
   use STALL TACTICS or SCHEMING" window has no engine shape: another seat would need a component
   action for Stall Tactics that Yssaril consents to, and Scheming for another seat's draw needs a
   consent prompt inside `action_cards::draw`'s hooks (which receive no breakthrough/consent
   context). Requested: a decision on how a non-Yssaril seat uses these (see Rules question 3), then
   a `component_actions` window keyed on another seat plus the transaction opening API.
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
