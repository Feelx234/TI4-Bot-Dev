# BF-mahact (part A)

Part B (flagship, Crimson Legionnaire I/II, Starlancer, hero) is `factions/mahact_units.rs`.

## Items

| Kind | Id | Status | Tests (`factions::mahact::tests`) |
|---|---|---|---|
| ability | edict | done | `edict_takes_the_opponents_token_once_after_a_space_win`, `edict_works_after_a_ground_combat_win_and_needs_a_token_to_take`, `two_eligible_opponents_are_the_mahact_players_choice`, `a_foreign_token_counts_in_the_fleet_limit_and_leaves_its_owners_reinforcements` |
| ability | hubris | done | `hubris_purges_the_alliance_and_refuses_anyone_elses` |
| ability | imperia | done | `imperia_lends_unlocked_commanders_of_token_owners_through_their_real_routes` (Xxcha via the vote window planet votes, Hacan via trade-goods votes), `imperia_needs_the_commander_unlocked_and_the_token_held` |
| technology | gr | done | `recombination_binds_a_voter_who_chooses_to_vote`, `recombination_lets_a_voter_pay_a_fleet_token_instead`, `recombination_forces_the_only_possible_way_and_can_be_declined` |
| technology | cl2 | claimed with part B's `mahact_infantry2` claim (B's tests) | n/a here |
| promissory | scepter | done | `the_scepter_places_the_pool_owners_tokens_and_returns`, `the_scepter_returns_even_with_nowhere_to_go_and_never_fires_unheld` |
| breakthrough | mahactbt | done | `the_vaults_purge_a_technology_for_a_relic_and_ready_in_the_status_phase` |
| leader | mahactagent | implemented, claim by coordinator | `the_agent_pays_a_secondary_with_the_active_players_board_token` |
| leader | mahactcommander (unlock) | implemented, claim by coordinator | `the_commander_unlocks_with_tokens_of_two_other_factions` |
| neutrality | no Mahact | done | `games_without_the_mahact_are_untouched_by_every_window` |

## Decisions

* Foreign tokens in the fleet pool are command tokens in that pool: counted in `fleet::limit` before the Fleet Regulations cap; they leave the owner's reinforcements while held (`mahact::reinforcements`); not part of the Mahact `fleet_tokens`, hence not redistributable. Writers: `add_token`, `remove_token` (fleet-pool mark `mahact:fleet:<mahact>` stays the shared API).
* Edict windows: `SPACE_COMBAT_WON` and `GROUND_COMBAT_ENDED` (with `winner`), Relation::When, mandatory; asks only if two opponents qualify.
* Hubris: `promissory::deal` skips `an` for Mahact; `promissory::may_receive` is checked in transactions (offer generation and `why_illegal`, new `OfferError::NoteRefused`).
* Scepter: resolves at `STRATEGY_PHASE_BEGAN` (no "may"); "command sheet" = fleet pool tokens; each other player (not the holder) with a token places one from reinforcements unless already in that system; card returns even with no legal system.
* Agent: offered as `StrategyHooks::secondary_waivers`, one waiver per system holding a token of the active (primary) player; generated up front.
* Commander unlock: two tokens of two distinct other factions.
* GR: new `VoteWindow` stages `Recombine` (holder picks outcome) and `Tribute` (voter picks cast-a-vote or return a fleet token); only the possible way is forced; the voter bound to vote may back only that outcome and must exhaust a first planet.
* Vaults: one option per technology; purge via new `technology::purge` (reverts unit upgrades on the board); readies at `STATUS_PHASE_ENDED`.

## Hook / change requests (files not mine)

1. `ti4-model/src/state.rs` `tokens_in_reinforcements`: subtract tokens of that player held in Mahact fleet pools (`mahact::held_by_mahact`), so `gain_token` and every cap agree with `mahact::reinforcements`. Until then a Mahact-held token can be re-gained by its owner.
2. `factions/mentak.rs` `notes_in_hand` (commander forces a note): filter with `promissory::may_receive(state, receiver, note)` (Mentak commander through Imperia, Hubris).
3. `diplomacy/builder.rs` and `diplomacy/candidates.rs` offer notes via `available_notes` without a receiver: filter with `promissory::may_receive`.
4. `choice.rs` observation `fleet_tokens` (line ~802) does not include foreign tokens; policy features may want `mahact::foreign_tokens`.
5. Claim `mahactagent`, `mahactcommander` in `mahact_units::LEADERS`.

## Decision sites to register

* `mahact.rs`: `ask_among` (3 callers: Edict opponent, Scepter system).
* `vote.rs`: `VoteWindow::pending_choice` arms `Stage::Recombine`, `Stage::Tribute`.
* Agent: option generation in `mahact::secondary_waivers` (strategy follower choice, existing site).
* Vaults: `mahact::component_actions` options.

## Commands

See final report for exact results.

## Integration (coordinator pass, single agent)

| Item | Result |
|---|---|
| Starlancer turn end | `game.rs` after `offer_nullification_field`: `offer_starlancer`, emits `TURN_ENDED_BY_STARLANCER:<holder>`; Game test `a_starlancer_ends_the_activators_turn_in_the_driven_game` |
| Hero hooks | `mahact.rs` Hooks: `leader_action` and `use_leader_timed` from `mahact_units` (part A has no leader_action; agent/commander are not action leaders). No `#[ignore]` existed. Game test `the_mahact_hero_is_offered_and_resolves_in_the_driven_game` (component action, fleet moves, card purged) |
| take_token | now `mahact::remove_token` (token returns to reinforcements, then `gain_tokens` takes one: no duplicate) |
| Reinforcements | Model fix: `ti4_model::GameState::tokens_held_by_mahact` (reads the `mahact:fleet:*` marks) subtracted inside `tokens_in_reinforcements`; so `gain_token`, tokens.rs, naaz.rs and every reader agree. `mahact::reinforcements` delegates (no double subtraction). Test `an_owner_cannot_regain_a_mahact_held_token` |
| Hubris | `promissory::may_receive` filter in `mentak.rs` `notes_in_hand` (now takes the receiver), `diplomacy/builder.rs` `item_options`, `diplomacy/candidates.rs` own-notes template. Test `the_deal_builder_never_offers_an_alliance_to_a_mahact_seat` |
| Observation | `choice.rs` `PublicSeat.fleet_tokens` adds `mahact::foreign_tokens` (0 without a Mahact); ti4-policy tests pass |
| Claims | UNITS += `mahact_mech`; LEADERS = `mahacthero`, `mahactagent`, `mahactcommander` |
| Registry | `decision_delivery_inventory.rs`: all new sites ObservedHere (Choice 1 + AskObserved 1 each: `mahact.rs ask_among`, `legionnaire`, `legionnaire_returns`, `offer_starlancer`, `use_leader_timed`); `vote.rs pending_choice` 4 -> 6 |

Results: `cargo test -p ti4-engine -j1`: lib 2473 passed / 1 ignored, content_ids_resolve 1, decision_delivery_inventory 4, 5 more passed, 0 failed. `-p ti4-policy -p ti4-sim`: 273 and 51 passed (1 ignored), 0 failed. `-p ti4-model`: 83 passed. Ledger: mahact 14/14 implemented. Soak `mahact 0 25 10`: 25 games, 0 failures.

Notes: `ti4-review/src/lib.rs` (~1627) lists `TURN_ENDED_BY_*` events for display; it does not know `TURN_ENDED_BY_STARLANCER` (UI crate, not touched). Edited files rustfmt-checked only where clean at base; game.rs not formatted.
