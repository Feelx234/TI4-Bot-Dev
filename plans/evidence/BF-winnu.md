# BF-winnu

Source: `crates/ti4-content/content/*.json` at DEFAULT. Code: `crates/ti4-engine/src/factions/winnu.rs`.

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | `blood_ties` | done (`custodians_free`) | `blood_ties_makes_the_custodians_free_for_winnu_only` |
| ability | `reclamation` | done | `reclamation_places_a_pds_and_a_space_dock_after_taking_mecatol`, `reclamation_needs_control_gained_this_action_and_may_be_declined`, `reclamation_is_atomic_when_the_box_lacks_a_space_dock` |
| tech | `lgf` | done (link + ACTION) | `gate_folding_links_mecatol_to_wormhole_systems_only_while_not_holding_it`, `gate_folding_needs_the_technology`, `gate_folding_action_places_an_infantry_and_exhausts` |
| tech | `htp` | done for tactical-action production (see Rules questions) | `trade_policy_swaps_one_planet_for_one_use_and_exhausts`, `trade_policy_is_not_offered_exhausted_or_for_another_players_production` |
| unit | `winnu_flagship` | done (`unit_dice`, gated on a Winnu seat) | `the_flagship_rolls_a_die_per_opposing_non_fighter_ship` |
| unit | `winnu_mech` | done | `the_reclaimer_places_one_structure_on_the_planet_it_stands_on`, `the_reclaimer_needs_the_mech_on_the_planet` |
| promissory | `acq` | done (note id `acq:winnu`) | `acquiescence_is_offered_to_its_holder_and_returns_when_used` (hook level; the strategy window path is covered by BF-00d tests) |
| leader | `winnuagent` | done | `the_agent_cuts_any_players_production_cost_by_two`, `the_agent_may_be_declined_and_needs_to_be_ready` |
| leader | `winnucommander` | done | `the_commander_unlocks_by_holding_mecatol_or_fighting_there`, `the_commander_adds_two_in_mecatol_home_and_legendary_systems` |
| leader | `winnuhero` | done, thinly tested | `the_hero_resolves_a_primary_and_lets_chosen_players_follow`, `the_hero_is_not_offered_without_cards_and_other_leaders_are_not_claimed` |
| breakthrough | `winnubt` | combat half done; move half done in module, needs game.rs wiring | `the_breakthrough_adds_one_per_support_in_the_opponents_play_area`, `the_breakthrough_gives_one_ship_plus_one_move_after_a_legendary_activation` |

Neutrality: `a_game_without_winnu_is_offered_nothing`. Every window checks the seat's faction or card.

## Hook requests

1. `game.rs begin_one_move`: pass `Some(index)` through `effective_move_value_for_ship` (already requested in BF-00e). `winnubt`'s +1 stays in a private `faction_marks` row `private:<p>:winnu:imperator` = `origin|index|ships_in_origin`. It is void once the ship count at the origin changes (a move out shifts indices), so if the player moves another ship first the bonus is lost. A "ship moved" payload with system and index, or a consume hook, would fix this.
2. `htp` and `acq`: `planet_spend_value` has no `sources`; the module reads DEFAULT. Pass `sources` if non-default scopes matter.
3. `component_actions` has no `sources`; `lgf` ACTION uses DEFAULT for the box check (placement itself uses the real sources).

## Rules questions

* Reclamation / Reclaimer: "tactical action during which you gained control" is approximated as `PLANET_CONTROL_GAINED` for the active seat, kept until the next `TURN_BEGAN`. Control gained without that event (action cards) is not seen.
* `htp`: only uses of PRODUCTION that emit `PRODUCTION_USED` (tactical action). Warfare secondary via `resolve_timed` does not emit it (BF-00d request 2). `htp_planets` offers only planets whose resources differ from influence.
* `lgf` link: Mecatol links to every alpha/beta system while the Winnu player has an active system and does not control Mecatol.
* Hero: cards offered are those held by any seat, except `te6warfare` (it returns a free tactical action that cannot run inside a leader effect). Followers are asked one by one (yes/no), in clockwise order, then each pays the usual token cost through the foreign window. An illegal answer inside the primary aborts after partial resolution (no rollback available).
* Commander unlock by combat: SPACE/GROUND_COMBAT_STARTED payload `system` equals 18 and the seat is attacker or defender.
* Imperator: opponent = first other owner of ships (space) or ground forces on the planet; counts `support_holders` entries held by that opponent.

## Decision sites to register

`winnu.rs`: `reclaimer` (1, `ask_seeing`), `htp` (1), `imperator` (1), `use_leader` (2: card, follower yes/no; plus the follower choice `pending` is asked through `ask_seeing` in the same loop = 3 asks). Reclamation, agent and the commander unlock ask only the ability window's own question.

## Commands and results

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- factions::winnu` | 23 passed, 0 failed |
| `cargo test -p ti4-engine --lib -- factions::` | 104 passed, 1 failed: `hooks_movement::tests::empty_tables_are_neutral` (asserts no module registers a movement hook; Winnu now registers `linked_systems`, `move_bonus`. That assertion needs updating by the coordinator) |
| `cargo clippy -p ti4-engine --all-targets` | no warnings in winnu.rs |
| `rustfmt --edition 2024 winnu.rs` | clean |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1611 passed, 3 failed: the `hooks_movement` one above, `production::tests::the_mc_technology_doubles_trade_good_payment_value` (not mine), `strategy::bf00d_tests::a_waiver_lets_a_tokenless_follower...` (coordinator's in-flight waiver change); `decision_delivery_inventory` 3 passed, 1 failed (the sites above) |

Ledger: `winnu 11/11 implemented`.

## Review fixes

| Finding | Fix | Test |
|---|---|---|
| B2 | `hero_cards` now includes `unclaimed_strategy_cards`. `te6warfare` stays excluded (its primary is a free tactical action that cannot run inside a leader effect), so `winnuhero` is no longer claimed in `MODULE` (ledger 10/11). | `the_hero_offers_unclaimed_cards_too` |
| S1 | Primary resolves first on a state snapshot; anything but `Ability::Resolved` (Err, `Unresolved`, `FreeTactical`) restores the snapshot and returns `Some(false)`. Followers are asked after the primary. Each follower's token plus secondary restore together on error. No new decision sites (same asks, reordered). | `a_failed_primary_leaves_the_game_untouched`, `the_hero_resolves_a_primary_and_lets_chosen_players_follow` (uses real card ids `pok1leadership`) |
| S2 | `room_for` (max 1 space dock, 2 PDS per planet) gates Reclamation and Reclaimer, re-checked at effect time. | `a_structure_is_not_placed_where_the_planet_has_no_room` |
| S5 | New `tactical_mark` on `SYSTEM_ACTIVATED`; control gained is recorded only after it; cleared on `TURN_BEGAN`. | `control_gained_in_a_strategic_action_does_not_count` |
| S6 | Imperator mark is `origin|type|damaged|count`; `move_bonus` goes to the first matching ship; `SHIP_MOVED` ability consumes it when a matching ship leaves. Hook request 1 above is superseded. | `the_imperator_bonus_survives_another_ship_leaving_and_is_spent_by_its_own` |

Checks: `factions::winnu` 28 passed, 0 failed; no clippy warnings in winnu.rs; rustfmt clean.
