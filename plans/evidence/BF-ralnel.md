# BF-ralnel (The Ral Nel Consortium, alias `ralnel`, Thunder's Edge)

Code: `factions/ralnel.rs` (module, Survival Instinct, Miniaturization, Nano-Link Permit, Linkship space-cannon loan, Last Dispatch, Alarum, Director Nel, shared Watchful Ojz claim), `factions/ralnel_cards.rs` (Nanomachines, Kan Kip Rel, Data Skimmer, Alarum's move, readiness). Ledger: `ralnel 13/13 implemented`; every other line unchanged.

## Items

| Kind | Id | Status | Tests (real route named) |
|---|---|---|---|
| ability | survivalinstinct | done | `survival_instinct_moves_a_ship_from_a_neighbour_into_the_activated_system`, `survival_instinct_brings_the_ground_forces_a_ship_carries_from_its_system` (real `SYSTEM_ACTIVATED` window, `transit::CargoWindow`), `survival_instinct_moves_at_most_two_ships_and_may_be_declined`, `survival_instinct_does_not_take_ships_from_a_system_with_your_command_token`, `survival_instinct_needs_one_of_your_ships_in_the_activated_system` |
| ability | miniaturization | done | `a_structure_in_the_space_area_cannot_fire_but_one_on_a_planet_still_does` (real `combat::space_cannon_offense`), `a_structure_in_space_is_cargo_for_any_ship_and_another_players_is_not` (real `transit::loadable`), `at_the_end_of_a_tactical_action_space_structures_settle_on_your_planet` (`TACTICAL_ACTION_ENDED` window) |
| technology | nanomachines | done | `nanomachines_places_a_pds_on_a_planet_you_control_and_exhausts`, `nanomachines_repairs_all_of_your_damaged_units`, `nanomachines_discards_one_card_and_draws_one`, `nanomachines_is_offered_only_with_the_technology_and_is_readied_by_the_status_phase` (real `perform_component`, `STATUS_PHASE_ENDED` window) |
| technology | linkship2 | done (stats data-driven; the loan is in `linkship_guns`) | `linkship_ii_borrows_the_structure_once_for_each_linkship` (real `space_cannon_offense`) |
| unit | ralnel_destroyer (Linkship I) | done | `linkship_i_borrows_each_structure_once` (real `space_cannon_offense`) |
| unit | ralnel_destroyer2 (Linkship II) | done | `linkship_ii_borrows_the_structure_once_for_each_linkship` |
| unit | ralnel_flagship (Last Dispatch) | done | `last_dispatch_destroys_a_ship_without_sustain_when_the_flagship_retreats` (`FLAGSHIP_RETREATED` window), `last_dispatch_counts_only_a_flagship_that_arrived_in_the_retreat` (`flagship_among`). |
| unit | ralnel_mech (Alarum) | done | `alarum_moves_a_ground_force_from_an_adjacent_planet_to_its_planet` (`GROUND_COMBAT_ROUND_ENDED` window), `alarum_needs_the_mech_on_the_planet_where_the_round_ended` |
| promissory | nanolink (Nano-Link Permit) | done | `the_nano_link_permit_moves_the_holders_structures_and_goes_back`, `declining_the_nano_link_permit_keeps_the_card_and_the_structures` |
| leader | ralnelagent (Kan Kip Rel) | done | `kan_kip_rel_draws_two_and_gives_one_to_the_player_chosen` (real `use_leader`) |
| leader | ralnelcommander (Watchful Ojz) | done (pre-existing borrowed-commander effect, claimed here) | `watchful_ojz_retreats_up_to_two_ships_when_the_commander_is_unlocked` (`RETREAT_DECLARED` window) |
| leader | ralnelhero (Director Nel) | done | `director_nel_takes_back_the_pass_for_two_command_tokens_and_a_card`, `director_nel_declined_keeps_the_pass_and_is_asked_only_after_the_last_pass` (`PLAYER_PASSED` window) |
| breakthrough | ralnelbt (Data Skimmer) | done | `data_skimmer_takes_another_players_discard_and_gives_one_card_when_the_holder_passes`, `data_skimmer_does_not_take_the_holders_own_discard` |
| neutrality | no Ral Nel | done | `a_game_without_the_ral_nel_is_untouched_by_every_window` (every window fired with no answers; state equal) |

## Model decisions

* **Windows.** Each optional ability asks its own "may" first (the window id, e.g. `ability:ralnel:survivalinstinct:SYSTEM_ACTIVATED:after`), then its own choices. Director Nel's window "may" is the only question (no second accept).
* **Survival Instinct.** Sources are the neighbours (`PlayerAdjacency`) without a Ral Nel command token; ships with `is_ship`. Up to two moves, each asked (decline ends). Each moved ship asks its own transport (`CargoWindow`, from the system it leaves); passengers move with it (`ralnel::carry_passengers`, the same shape as Ahk Ravin's carry).
* **Miniaturization.** A Ral Nel structure in a space area is silenced for its own SPACE CANNON (`combat.rs` gun filter, `silenced_in_space`) and is cargo for any ship at no capacity (`transit.rs::loadable_by` + the `free_cargo` hook). At `TACTICAL_ACTION_ENDED`, per system with a space structure and a controlled planet, one question picks the planet.
* **Linkships.** `linkship_guns` (called from `space_cannon_offense` with the same `may_fire_here` gate as every gun): Linkship I takes one structure each, distinct (best hit first); Linkship II each fires the best structure's gun again. The shot is the structure's own unit record, so it rolls like any gun.
* **Nano-Link Permit.** Any holder (condition: note holder is the activating player) moves their structures from adjacent systems without their command token, in space or on planets, onto one planet they control in the active system (one question). The card returns to the Ral Nel player only when it was used.
* **Last Dispatch.** `combat.rs::retreat` emits `FLAGSHIP_RETREATED` (only when a Ral Nel flagship, or a Nekro flagship lent its text, arrived). The window destroys one ship (any owner's) in the active system whose type has no SUSTAIN DAMAGE; `combat::destroy_units`.
* **Alarum.** `invasion.rs` emits `GROUND_COMBAT_ROUND_ENDED` after the round's hits, only when a Ral Nel mech stands on that planet (so other games' event logs are unchanged). Up to two ground forces move from planets of the planet's own system or an adjacent one (the ruling) to this planet.
* **Director Nel.** `PLAYER_PASSED` window, only when the hero is unlocked and every seat has passed. Un-pass, 2 command tokens (`strategy_cards::gain_tokens`), 1 action card, purge.
* **Nanomachines.** One exhaustion (`ralnel:nano:exhausted:<player>`) covers PDS / repair / discard-and-draw; readied at `STATUS_PHASE_ENDED`. The PDS is placed through `action_cards::place_units_counted` (reinforcements apply); discard asks when more than one card is held.
* **Kan Kip Rel.** `use_leader`: draw 2; if other seats exist, the card to give (asked when two) and the recipient (asked when several). Exhaustion is the generic caller's.
* **Data Skimmer.** While the holder has not passed in the action phase, another player's discard goes on the card (`ralnel_cards::skimmer_takes`), wired into `reactions.rs::announce_discard` (played/announced cards) and Nanomachines' discard. At the holder's `PLAYER_PASSED`, one card is taken (asked when several) and the rest discarded.

## Shared rights (predicates)

| Card | Reached through |
|---|---|
| Watchful Ojz | `promissory::has_commander_ability` (the existing borrowed-commander effect, seat-wide) |
| Last Dispatch text | `factions::flagship_has_text`; `ralnel_flagship` added to `nekro::LENDABLE_FLAGSHIPS` |
| Nanomachines | `technology` ownership (`seat.technologies`) |
| Linkships' loan, Miniaturization | unit-type predicates (`is_structure`), no faction-name checks in shared code |

## Open questions and limits (most literal reading used)

1. **Linkship vs Miniaturization (operator ruling, 2026-10-07).** A Linkship may use the SPACE CANNON of a structure in its space area, while the structure itself stays silenced by Miniaturization. This is the implemented reading; no longer an open question.
2. **Nano-Link Permit "your".** Read as the activating holder's structures (the note is traded). Return only when used (the "may" gates the return).
3. **Last Dispatch "destroy 1 ship".** Read as any player's ship in the active system without SUSTAIN DAMAGE (the unit type's sustain flag, not its damage state).
4. **Alarum adjacency.** The async ruling (includes the planet's own system) is applied.
5. **Director Nel.** The un-pass uses the seat's `passed` flag after `PLAYER_PASSED`; `advance_turn` keeps the turn with the un-passed seat (driven test `director_nel_un_pass_is_honoured_by_the_real_turn_route`).
6. **Miniaturization planet question** is asked even when only one planet is controlled (the "may" is the window's).
7. **Kan Kip Rel recipient hand limit** is not enforced on receipt (the hand-limit discard path runs only at the normal draw).


## Decision registry

`ralnel.rs::ask` (every window question, Choice-producer and observed) and `ralnel.rs::passengers` (Survival Instinct's transport, the shared cargo hold). Registered in `tests/decision_delivery_inventory.rs`. `ralnel_cards.rs` calls `ask` only.

## Files

Created: `crates/ti4-engine/src/factions/ralnel.rs`, `crates/ti4-engine/src/factions/ralnel_cards.rs`, `plans/evidence/BF-ralnel.md`.

Modified (engine, shared): `crates/ti4-engine/src/factions/mod.rs` (modules `ralnel`, `ralnel_cards`; `MODULES` 25 with `&ralnel::MODULE`), `crates/ti4-engine/src/factions/nekro.rs` (`ralnel_flagship` in `LENDABLE_FLAGSHIPS`), `crates/ti4-engine/src/combat.rs` (space-cannon silence and loan in `space_cannon_offense`; `FLAGSHIP_RETREATED` emit in `retreat`), `crates/ti4-engine/src/transit.rs` (`loadable_by` structure cargo), `crates/ti4-engine/src/invasion.rs` (`GROUND_COMBAT_ROUND_ENDED` emit, only with a Ral Nel mech on the planet), `crates/ti4-engine/src/reactions.rs` (`announce_discard` redirect to Data Skimmer), `crates/ti4-engine/tests/decision_delivery_inventory.rs` (`ralnel.rs` producer and observed entries; `passengers`), `crates/ti4-sim/examples/base_faction_soak.rs` (`ralnel` in the soak list, 25 entries).

Not touched: any policy, training, UI or server crate. `game.rs` carries another session's hunks; this package changes exactly one line there (Extreme Duress, see review round) and nothing else.

## Review round (coordinator findings, 2026-10-07)

| # | Finding | Fix | Driven test |
|---|---|---|---|
| 1 | A zero-capacity ship could never carry a structure | `transit.rs::CargoWindow::for_ship`: with capacity <= 0, candidates that consume capacity are dropped; the hold closes only when no candidate rides free. A destroyer's hold offers only Ral Nel structures; a carrier's slots are unchanged by them | `a_destroyer_carries_a_structure_free_but_still_not_infantry_in_a_real_move`, `structures_do_not_use_a_carriers_capacity_in_a_real_move`, `a_zero_capacity_hold_with_nothing_free_stays_closed` (real tactical action through `Game::step`) |
| 2 | Space-area structure kept PRODUCTION (and the adjacent-reaching SPACE CANNON) | `production.rs::producers` filters `ralnel::silenced_in_space`; `combat.rs::reaching_guns_by` takes a `silenced_in_space` predicate for the neighbour's space area. Every other unit ability read (PLANETARY SHIELD, invasion SPACE CANNON DEFENSE, FIGHTER support) reads planet units only, so a space-area structure never reaches them | `a_structure_in_the_space_area_has_no_production_but_one_on_a_planet_does` (`production::capacity`), existing `a_structure_in_the_space_area_cannot_fire_but_one_on_a_planet_still_does` |
| 3 | Data Skimmer redirect covered only some discard sites | New `action_cards::discarded(state, discarder, card, to_pile)` is the one landing place: Data Skimmer first, else pile (`to_pile`) or nowhere (callers that never kept a pile, behaviour unchanged). Routed: `reactions::announce_discard` (played cards and every staged `discard_chosen`: Stall Tactics, Scheming, Malleon, Kyver), Nanomachines, Extreme Duress (`game.rs`, one line), hand-limit (`enforce_hand_limit`), Sanctions, Expedition (`thunders_edge.rs`), `agenda_effects::discard_hand` and Unconventional Measures. Agenda-phase sites are inert (the redirect is action phase only). The Form a Spy Network cost (`secrets.rs`) is a score cost, not a discard, and is left alone | `data_skimmer_takes_a_hand_limit_discard`, `data_skimmer_takes_a_played_card_instead_of_the_pile`, `data_skimmer_leaves_a_played_card_on_the_pile_once_the_holder_has_passed` |
| 4 | Nano-Link Permit return | Reading recorded: the window is offered only when there is a structure to move and a controlled planet to take it to, and the holder's "may" is the decision; accepting always moves and then returns the card. Declining keeps both. A card with nothing to move is never offered, so it never returns for nothing | existing nanolink tests |
| 5 | Director Nel and Last Dispatch only window-tested | Driven tests below | `director_nel_un_pass_is_honoured_by_the_real_turn_route` (`pass` through `Game::step`, `advance_turn` keeps the turn with the un-passed seat, phase stays Action), `last_dispatch_destroys_a_ship_when_the_flagship_retreats_in_a_real_space_combat` (`CombatWindow` drive, the flagship retreats, the window fires from `combat::retreat`, the sustainless cruiser dies and the dreadnought is not offered) |
| 6 | Linkship + Miniaturization | Operator ruling 2026-10-07: Linkships may use the SPACE CANNON of a structure in their space area; the structure itself stays silenced. Implemented as before | existing `linkship_*` tests |

Observation (not changed): hand-limit, Sanctions, Expedition and agenda discards never reached the discard pile before this round and still do not; only the Data Skimmer interception was added to them.

Extra files changed this round: `crates/ti4-engine/src/action_cards.rs` (`discarded`, hand-limit route), `crates/ti4-engine/src/agenda_effects.rs`, `crates/ti4-engine/src/thunders_edge.rs`, `crates/ti4-engine/src/production.rs`, `crates/ti4-engine/src/game.rs` (one line, Extreme Duress), plus `combat.rs`, `transit.rs` (converted CRLF to LF to match HEAD), `reactions.rs`, `factions/ralnel.rs`, `factions/ralnel_cards.rs`.

## Results

* `cargo test -p ti4-engine --lib factions::ralnel -j1`: 27 passed, 0 failed (`out/ralnel_test5.log`).
* `cargo test -p ti4-engine --test decision_delivery_inventory -j1`: 4 passed (`out/ralnel_inventory4.log`).
* `cargo test -p ti4-engine -j1 -- --test-threads=12`: lib 2694 passed / 1 ignored; 1, 4, 5 passed; 0 failed (`out/ralnel_engine_full.log`).
* `cargo test -p ti4-policy -p ti4-sim -j1 -- --test-threads=12`: 273 and 51 passed (1 ignored), 0 failed (`out/ralnel_policy_sim.log`).
* Ledger (`print_faction_ledger --ignored`): ralnel 13/13; the other 29 lines identical to the baseline (`out/ralnel_ledger_final.log`, baseline `out/ralnel_ledger_before.log`).
* Soak `cargo run --release -p ti4-sim --example base_faction_soak -q -j1 -- ralnel 0 25 10`: 25 games, 0 failures, replay every 10 (`out/ralnel_soak.log`).
* Workers: `-j1` for every build; `--test-threads=12` for suites (AGENTS.md bound).

### Results after the review round

* `cargo test -p ti4-engine --lib factions::ralnel -j1`: 36 passed (`out/ralnel_fix_test2.log`).
* `cargo test -p ti4-engine -j1 -- --test-threads=12`: lib 2703 passed / 1 ignored; 1, 4, 5 passed; 0 failed; exit 0 (`out/ralnel_fix_engine_full2.log`). Includes the coordinator crimson BREACH_SUPPLY change; no crimson failure.
* `cargo test -p ti4-policy -p ti4-sim -j1 -- --test-threads=12`: 273 and 51 passed (1 ignored), 0 failed (`out/ralnel_fix_policy_sim.log`).
* Ledger: ralnel 13/13; implemented lines identical to `out/ralnel_ledger_final.log` (`out/ralnel_fix_ledger.log`).
* Soak `ralnel 0 25 10` (release): 25 games, 0 failures (`out/ralnel_fix_soak.log`).
* Workers: `-j1` builds, `--test-threads=12`.
