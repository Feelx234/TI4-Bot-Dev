# BF-crimson (The Crimson Rebellion, alias `crimson`)

Code: `factions/crimson.rs` (breach model, Sorrow, Incursion, Sundered, Quietus, Exile, Revenant DEPLOY, Resonance Generator movement bonus), `factions/crimson_cards.rs` (Subatomic Splicer, Sever, Ahk Ravin, Homesick Phantom, Resonance Generator ACTION). Ledger: crimson 14/14 implemented; every other faction unchanged.

## Items

| Kind | Id | Status | Tests (all drive the real route named) |
|---|---|---|---|
| ability | sorrow | done | `the_crimson_seat_starts_in_tile_118_and_the_sorrow_holds_an_inactive_breach`, `the_sorrow_sits_in_the_home_position_and_the_home_system_is_off_the_map`, `boards_without_a_crimson_seat_are_unchanged` (real `seating::deploy`, `build_board`) |
| ability | incursion | done | `incursion_lets_the_crimson_flip_the_breach_in_a_system_they_activate`, `incursion_is_only_for_the_crimsons_own_activation_of_a_system_with_a_breach`, `systems_with_active_breaches_are_adjacent_for_every_player` (`MovementRules`, `PlayerAdjacency`), `at_the_end_of_the_status_phase_a_player_with_ships_may_remove_an_active_breach` (real `SYSTEM_ACTIVATED` / `STATUS_PHASE_ENDED` windows) |
| ability | sundered | done | `sundered_closes_every_wormhole_but_epsilon_to_its_owner` (real `tactical::movable_into`: never offered), `sundered_keeps_the_epsilon_wormhole_open_both_ways`, `a_retreat_is_a_move_so_sundered_closes_the_wormhole_to_it_as_well` (`combat::eligible_retreats`), `units_that_move_into_the_home_system_are_destroyed_in_a_real_tactical_action` (driven `Game`: activate 118, move, cargo), `units_placed_into_the_home_system_are_destroyed_whatever_the_route` (driven `Game::step`) |
| technology | subatomic | done | `subatomic_splicer_produces_a_ship_of_the_destroyed_type_at_the_home_dock`, `..._is_a_may_and_needs_the_card_a_ship_the_dock_and_the_means`, `a_nekro_assimilating_subatomic_splicer_uses_its_text` (real `SHIP_DESTROYED` window + `production::produce_unit_by_ability`) |
| technology | exile2 | done | `the_exile_destroyers_and_the_exile_ii_upgrade_are_the_corpus_stats` (real `technology::apply_unit_upgrades`) |
| unit | crimson_destroyer (Exile I) | done | `exile_i_places_an_inactive_breach_where_a_combat_ended_in_or_next_to_its_system` (real `SPACE_COMBAT_ENDED` / `GROUND_COMBAT_ENDED` windows over a map) |
| unit | crimson_destroyer2 (Exile II) | done | `exile_ii_reaches_two_systems_and_may_place_an_active_breach` |
| unit | crimson_flagship (Quietus) | done | `the_quietus_strips_sustain_damage_from_other_players_units_in_active_breaches`, `the_quietus_stops_production_space_cannon_barrage_bombardment_and_planetary_shield` (real `placements`, `space_cannon_offense`, `roll_barrage_side`, `bombardable`, `bombardment`), `a_nekro_flagship_carrying_the_quietus_text_strips_abilities_too` |
| unit | crimson_mech (Revenant) | done | `the_revenant_deploys_and_commits_with_no_units_in_an_active_breach_system` (real `invasion::resolve`), `the_revenant_is_not_deployed_without_an_active_breach_a_mech_or_the_rebellion`, `a_tactical_action_opens_the_commit_step_for_a_deploy_with_no_ship_in_the_system` (driven `Game`) |
| promissory | sever | done | `sever_is_played_for_an_action_and_silences_a_systems_wormholes_for_all_movement_until_the_status_phase_ends`, `sever_may_be_played_in_a_system_with_no_wormhole_and_needs_a_system_with_units` (real component action, `MovementRules`) |
| leader | crimsonagent (Ahk Ravin) | done | `ahk_ravin_swaps_two_ships_and_what_they_carry_ignoring_anomalies_and_blockades` (real `leaders::use_leader` / `component_actions`), `..._lets_the_chosen_player_decline_...`, `..._asks_the_user_which_player_when_several_could_swap`, `ssruu_copies_ahk_ravins_action` |
| leader | crimsoncommander (Ahk Siever) | done | `placing_a_breach_where_another_players_unit_stands_unlocks_ahk_siever`, `ahk_siever_triggers_off_the_combat_that_unlocked_it_and_pays_a_commodity`, `ahk_siever_reaches_an_alliance_holder_through_the_shared_commander_predicate` |
| leader | crimsonhero (Homesick Phantom) | done | `the_hero_takes_produced_ships_onto_its_card_and_they_stay_out_of_reinforcements` (real `production::resolve`), `the_hero_places_every_ship_on_its_card_into_the_active_system_and_is_purged` (real `SPACE_COMBAT_STARTED` window) |
| breakthrough | crimsonbt (Resonance Generator) | done | `the_resonance_generator_adds_one_to_ships_starting_at_home_or_in_an_active_breach` (real `tactical::effective_move_value`), `the_resonance_generator_flips_a_breach_exhausts_and_readies_in_the_status_phase`, `the_resonance_generator_places_an_active_breach_in_a_non_home_system_with_the_owners_units` (real component action) |
| neutrality | no Crimson | done | `a_game_without_the_crimson_is_untouched_by_every_window`, `a_game_without_the_crimson_offers_none_of_these_cards` |

The faction promissory note alias is `sever` (`promissory_notes.json`, faction `crimson`; the sheet lists it as `sever`). The card text names the owner "the Rebellion player" (the sheet's `shortName`); the note id is `sever:crimson`.

## Content findings

* **The sheet's home system is the Sorrow.** `factions.json` names tile 94 as `homeSystem`; the Sorrow ability puts tile 94 where the home would go and the real home system, tile 118 (Ahk Creuxx, planet `ahkcreuxx`, 4/2), in the play area. This is the Creuss gate/home arrangement exactly, and `seating.rs` mirrors it: `SORROW`/`CRIMSON_HOME`, `deploy` seats the Rebellion in 118, and `place_crimson_home` (called from `place_wormhole_nexus`, which every map family already calls) puts 118 beside the board when 94 is on it. 94 and 118 are the only two tiles that print an epsilon wormhole, so the home system is reached only through the Sorrow, which is why Sundered leaves epsilon open.
* Breaches already had one field, `GameState::breach_tokens` (systems holding a breach, drawn by the reviewer). Nothing used it for gameplay. Active/inactive is the mark `crimson:breach:active:<system>`.
* The commander's payment already existed (`borrowed_commanders::crimson_commander`, registered for every seat and gated on `promissory::has_commander_ability`). It is reused, not duplicated; this package adds the unlock and claims the leader.
* The Revenant's DEPLOY has no engine mechanism (`DEPLOY` is faction-specific text everywhere). It is a new landing option in the commit step.

## Model decisions

* **Breaches**: `crimson::{has_breach,is_active,active_breaches,flip_breach,remove_breach,place_breach}`. One breach per system. `BREACH_SUPPLY` = 6 tokens (the corpus prints no count; see open questions). With none in reinforcements the placer pulls an inactive breach off the board (asked when several; the printed note). Placing one in a system holding another player's unit unlocks Ahk Siever at once (so it is live in that same combat).
* **Incursion**: flip is an optional `SYSTEM_ACTIVATED` (when) window for the activator. Adjacency is the `linked_systems` movement hook, returning every pair of active-breach systems for **every** player; it reaches `MovementRules` and `PlayerAdjacency` (neighbours, transactions, space cannon range). Status removal: an optional `STATUS_PHASE_ENDED` window registered for every seat (repeatable), offered to a player with ships in an active-breach system; which breach is asked when several.
* **Sundered, wormholes**: new `MovementHooks::usable_wormhole_kinds` (the kinds a mover may use; Sundered returns epsilon only). `MovementRules` builds a copy of the map with the other kinds removed (`Galaxy::retain_wormhole_kinds`, printed and token alike), so a ship is never *offered* the move and a route never uses the wormhole. `PlayerAdjacency` is left alone (the note: the owner still fires SPACE CANNON and is a neighbour through them). Retreats are moves: `combat::eligible_retreats` reads `hooks_movement::galaxy_for_mover`.
* **Sundered, home system**: `crimson::enforce_sundered`, a reconcile run once per `Game::step` (beside station control and the Keleres reconcile) and when a movement step finishes (`MOVEMENT_FINISHED`, before space cannon, combat or invasion). Every unit that is not the owner's, on the space area or any planet of the seat's home system, is destroyed. Ships go through `combat::destroy_units` (staged `SHIP_DESTROYED`); ground forces through the ground staging (`GROUND_FORCE_DESTROYED`, cause `sundered`). A reconcile instead of a hook per route because units reach a system through tactical movement, relocation, production, action cards, exploration, leaders and coexistence, each writing the board itself.
* **Quietus**: `crimson::abilities_lost(state, owner, system)`: the system holds an active breach and another player's Quietus (or a Nekro flagship carrying its text, `flagship_has_text`, `crimson_flagship` added to `nekro::LENDABLE_FLAGSHIPS`) stands in a system holding one. Read at the seam of each unit ability: SUSTAIN DAMAGE (space and ground, through the `may_sustain` hooks), PRODUCTION (`production::placements`), SPACE CANNON (offense for guns in the system and adjacent-reaching guns, defense), ANTI-FIGHTER BARRAGE (`roll_barrage_side`), BOMBARDMENT (`roll_bombard_plan`), PLANETARY SHIELD (`bombardable`). The seven abilities are the list in `rules.json` (`entropicScar`).
* **Exile I/II**: an optional `SPACE_COMBAT_ENDED` / `GROUND_COMBAT_ENDED` window per Crimson seat. In range = the Exile's own system or adjacent (II: within two) by the owner's `PlayerAdjacency`. A single question when II allows a choice (active / inactive / decline); Exile I alone has only the window's accept/decline. The two destroyers' stats and the Exile II upgrade (`exile2`, replaces the Destroyer II) are the corpus's, delivered by the unit-upgrade route.
* **Revenant**: `deploy_commit|<planet>` option in the commit step (`invasion.rs::landing_options`), offered when the invader is the Rebellion, the system holds an active breach, a mech remains in reinforcements and the Revenant has not already been deployed in this activation (`crimson:deployed:` mark). The mech lands from reinforcements as a committed ground force (toll, `UNITS_COMMITTED`, diplomacy hostility all go through the new shared `announce_landing`). The tactical action opens the commit step for it even when no ship holds the space (`game.rs`, `nobody_there && can_deploy`), but not while any player's ship does.
* **Resonance Generator**: `move_bonus` hook (home system or active-breach origin, only for the holder). ACTION: component action, exhaust mark `crimson:bt:exhausted:<player>`, ready at `STATUS_PHASE_ENDED`; options are `flip|<system>` for any breach and `place|<system>` (active) for each non-home system holding the holder's units where a breach can be placed. "Non-home" = holds no faction's home planet and is no seat's home system.
* **Subatomic Splicer**: the shape of Nekro Null Reference: an optional `SHIP_DESTROYED` window, paid as ordinary production in the seat's home system, offered only when that production can happen (`can_produce_unit_by_ability`). Rights through `technology::has_technology_text` (Nekro X/Y).
* **Sever**: `sever` is ACTION-placed (`promissory::is_action_placed`), played from the holder's hand through `faction|crimson|sever`; the holder picks a system holding their units (asked when several). Mark `crimson:sever:<note>` = `<holder>|<system>`; the `severed_systems` hook makes `MovementRules` (and retreats) drop that system's wormholes for every mover; at `STATUS_PHASE_ENDED` the mark goes and the note returns (`promissory::give_back`).
* **Ahk Ravin**: `leader_action` / `use_leader` hooks (the generic caller exhausts), so Ssruu reaches it. The agent's owner picks the player (asked when several qualify: ships in two or more systems); that player decides: swap or decline, the first ship, the second (a ship in a different system), then each ship's transport (the shared `transit::CargoWindow`, from the system the ship is leaving, 95.5 applies). Applied atomically; not a move, so no activation, route, anomaly, blockade or command-token rule applies. The agent is used (exhausted) even if the chosen player declines.
* **Homesick Phantom**: the production placement spot `crimsoncard@space` (offered by `crimson_cards::offer_card_spot` in `ProductionWindow::spots`, taken by `ProductionWindow::place`) puts a produced ship on the card (mark `crimson:card:<player>`, unit type ids). `supply::held` counts them, so they stay out of reinforcements. Launch: optional `SPACE_COMBAT_STARTED` (after) window for a combat the owner is in (attacker or defender); the hero is purged and every ship on the card joins the active system. Heroes unlock generically at three scored objectives.

## Rights (shared predicates)

| Card | Reached through |
|---|---|
| Ahk Siever | `promissory::has_commander_ability` (Alliance, Yin, Mahact Imperia, Nekro, direct grants); test with `grant_commander_ability` |
| Subatomic Splicer | `technology::has_technology_text` (Nekro Valefar X/Y); test with an assimilated token |
| Quietus | `factions::flagship_has_text` / `has_flagship_text_in`; `nekro::LENDABLE_FLAGSHIPS` |
| Ahk Ravin | the generic leader dispatch (Ssruu `use_leader_text`) |
| Resonance Generator | `breakthroughs::holds` |

## Open questions and limits

1. **Breach count.** The corpus prints none; `BREACH_SUPPLY` is 6. Only the pull-from-board rule depends on it.
2. **One breach per system** is the engine's reading (the model is a set of systems; "that breach" in the text).
3. **Quietus and DEPLOY / faction text.** DEPLOY, and the text abilities printed on faction units, are not suppressed: there is no shared DEPLOY seam (each faction's mech has its own site) and the rules (entropic scar 2.1) leave text abilities alone. The seven keyworded abilities above are suppressed everywhere the engine reads them. Attachment-granted SPACE CANNON is not a unit's and is untouched.
4. **Sundered and relocation.** Tactical moves and retreats close non-epsilon wormholes. Out-of-turn relocations (`transit::relocate_ships`) check adjacency with `PlayerAdjacency`, which keeps the wormholes by design; no Rebellion effect relocates a Rebellion ship through a wormhole, but another player's relocation card moving a Rebellion ship would not be refused.
5. **Active-breach adjacency** is wired through the player-aware adjacency (`MovementRules`, `PlayerAdjacency`). Code that reads `Galaxy::adjacent` directly (retreat destinations, adjacent-reaching SPACE CANNON) does not see it.
6. **Home system destruction** is a reconcile, so a unit that reaches 118 in the middle of a step is destroyed at the next reconcile or movement-finished window, not at the instant it is written. No decision is offered in between.
7. **Exile "may"** is one question per combat and seat (Exile I/II in range), not one per destroyer: a system holds one breach, so a second use could never resolve.
8. **Revenant** reads "commit 1 mech" as "place one from reinforcements and commit it", once per activation; a Revenant already on a planet commits through the ordinary option.
9. **Homesick Phantom** places a whole purchase (same unit type, same spot) onto the card, because the production window asks one placement per purchase. Placing ships on the card ignores fleet supply and capacity in the producing system (Dane); placing them into the combat is subject to the normal end-of-turn fleet enforcement.
10. **Ahk Ravin** swaps ships between two different systems only; a transported unit is set down in the destination's space area (ground forces do not land).
11. **Sever** is "a system that contains your units": ships, ground forces or structures of the holder.
12. **Ahk Siever's** payment is the pre-existing mandatory implementation (a choice only when both a gain and a convert are possible).

## Shared files touched

`ti4-content/src/galaxy.rs` (+`retain_wormhole_kinds`, `suppress_wormholes_at`), `factions/mod.rs` (module registered, `MODULES` 24), `factions/hooks_movement.rs` (+`usable_wormhole_kinds`, `severed_systems`, `wormhole_limits`, `galaxy_for_mover`), `movement.rs` (`MovementRules` applies the limits), `seating.rs` (`SORROW`, `CRIMSON_HOME`, `deploy`, `place_crimson_home`), `combat.rs` (`eligible_retreats`, ANTI-FIGHTER BARRAGE, SPACE CANNON), `invasion.rs` (commit-step deploy option and `announce_landing` extracted from the commit branch, BOMBARDMENT, PLANETARY SHIELD, SPACE CANNON defense, `landable_planets` crate-visible), `production.rs` (PRODUCTION gate, hero-card spot), `supply.rs` (`held` counts the hero card), `promissory.rs` (`sever` is ACTION-placed), `factions/nekro.rs` (`crimson_flagship` lendable), `tests/decision_delivery_inventory.rs` (new sites), `ti4-sim/examples/base_faction_soak.rs` (crimson). `game.rs`: two separate additions only (the `enforce_sundered` call beside the Keleres reconcile in `step`, and the `nobody_there && can_deploy` clause after the Coalescence clause in the aftermath's `holds`); the other session's round-income hunks are untouched. No policy, training, UI or server crate.

## Decision registry

`crimson.rs::ask` and `ask_about` (every timing-window / component-action question); `crimson_cards.rs::fill_hold` (Ahk Ravin's transport, the shared cargo hold). The commit-step landing is an option of the existing commit choice. All are in `tests/decision_delivery_inventory.rs`.

## Results

* `cargo test -p ti4-engine -j1 -- --test-threads=12`: lib 2667 passed / 1 ignored (42 of them `factions::crimson*`), then 1, 4, 5 passed, 0 failed (`out/crimson_engine_full3.log`).
* `cargo test -p ti4-policy -p ti4-sim -j1 -- --test-threads=12`: 273 and 51 passed (1 ignored), 0 failed (`out/crimson_policy_sim.log`).
* Ledger (`print_faction_ledger --ignored`): crimson 14/14; the other 29 lines identical to `out/dw_ledger3.log` (`out/crimson_ledger.log`).
* Soak `crimson 0 25 10` (release, final code): 25 games, 0 failures (`out/crimson_soak2.log`).

## Review

Implemented by one Sonnet implementer. Tier C material (timing windows, legality of movement and destruction, production gating): the independent frontier review is still owed; nothing here claims it. Review attention points: the `Game::step` reconcile call and the `holds` clause in `game.rs`; `MovementRules`'s wormhole copy; the `announce_landing` extraction in `invasion.rs`.

## Operator rulings 2026-10-07 (recorded by coordinator)

1. Breach tokens: 7, each with an active and an inactive side (`BREACH_SUPPLY` = 7).
2. At most one breach per system (as implemented).
3. Quietus does not suppress DEPLOY or printed text abilities (as implemented).
4. Revenant places its mech from reinforcements, once per activation (as implemented).
5. Ahk Ravin is exhausted even when the chosen player declines the swap (as implemented).
