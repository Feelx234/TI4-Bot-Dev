# BF-obsidian (The Obsidian, alias `obsidian`) -- part B of Firmament / Obsidian

Code: `factions/obsidian.rs` (all twelve ledger assets; tests in the same file). Part A (`firmament.rs`, `firmament_flip.rs`) is unchanged except two small goods helpers in `firmament.rs`. Ledger: obsidian 12/12 implemented; firmament still 11/11; every other line unchanged (`out/ob_ledger1.log` against `out/fm_ledger1.log`: the only difference is the new `obsidian` line).

## Items

| Kind | Id | Status | Tests (real route) |
|---|---|---|---|
| ability | nocturne | done | `nocturne_keeps_the_obsidian_out_of_a_setup_roster` (`seating::seeded_faction_assignments` refuses it; printed text asserted) |
| ability | bladesorchestra | done | `the_blades_orchestra_flips_the_home_the_components_and_the_plots_and_readies_the_hollows` (real `mapped_component_actions` + `perform_component`, then the staged `FACTION_FLIPPED`), `a_firmament_game_flips_and_then_the_viper_hollow_the_reaping_and_the_notes_work` |
| ability | marionettes | done | `marionettes_make_every_player_with_a_token_on_a_plot_a_puppeted_player` (one plot, two tokens; read by The Reaping) |
| technology | planesplitter-obs | done | `planesplitter_moves_an_ingress_token_next_to_the_owners_units_when_it_performs_a_strategic_action`, `planesplitter_is_a_may_for_its_owner_alone_and_needs_the_fracture` (real `STRATEGIC_ACTION_BEGAN` window), `the_obsidian_technologies_cannot_be_researched_by_anyone` (`technology::can_research` / `researchable`) |
| technology | parasite-obs | done | `neural_parasite_destroys_an_adjacent_players_infantry_at_the_start_of_the_owners_turn`, `..._lets_its_owner_choose_among_several_players_infantry`, `..._needs_infantry_of_the_owner_nearby_and_a_target` (real `TURN_BEGAN` window) |
| unit | obsidian_flagship (Heaven's Hollow) | done | `heavens_hollow_is_statistics_only` (stats from the catalogue; no printed ability, so nothing for a Nekro Z token to lend) |
| unit | obsidian_mech (Viper Hollow) | done | `a_firmament_game_flips_and_then_the_viper_hollow_...` (coexisting Viper, real flip, announced `FACTION_FLIPPED`), `a_viper_that_was_not_coexisting_changes_nothing_when_the_card_flips` |
| leader | obsidianagent (Vos Hollow) | done | `vos_hollow_makes_the_victims_opponent_destroy_a_ship_of_the_same_type`, `..._is_a_may_and_needs_a_matching_ship_in_a_combat`, `..._lets_the_opponent_pick_which_ship_and_the_owner_pick_which_opponent` (real `SHIP_DESTROYED` window) |
| leader | obsidiancommander (Aroz Hollow) | done | `aroz_hollow_unlocks_with_units_in_the_fracture` (real `leaders::check_unlocks`), `aroz_hollow_adds_one_to_space_and_ground_rolls_in_the_fracture_only` (real `combat::effective_hits_on` and `invasion::ground_combat_value`) |
| leader | obsidianhero (Sharsiss Hollow) | done | `sharsiss_hollow_readies_every_planet_and_is_purged`, `sharsiss_hollow_is_offered_only_when_unlocked_and_something_is_exhausted` (real `leaders::use_leader` / `usable` / `check_unlocks`) |
| promissory | malevolency | done | `malevolency_is_given_to_a_neighbour_for_one_influence_even_by_the_obsidian`, `malevolency_is_a_may_that_needs_a_neighbour_and_the_influence`, `a_holder_who_is_not_the_obsidian_loses_a_fleet_token_at_the_end_of_the_status_phase`, and `the_game_announces_the_end_of_a_tactical_action_to_malevolency` (a real `Game` tactical action) |
| breakthrough | obsidianbt (The Reaping) | done | `the_reaping_takes_a_trade_good_for_each_space_combat_won_against_a_puppeted_player`, `..._also_counts_ground_combats_won_by_either_side`, `..._gains_the_card_then_as_many_again_at_the_start_of_the_status_phase` (real `SPACE_COMBAT_WON` / `GROUND_COMBAT_ENDED` / `STATUS_PHASE_BEGAN` windows) |
| home | 96b, cronoshollow, tallinhollow | done | `the_hollow_home_system_builds_onto_a_board` (real `seating::build_board`), `a_seat_that_is_the_obsidian_from_the_start_is_coherent_...` (real `seating::deploy`) |
| view | flipped plots, goods on the card | done | `once_flipped_the_plots_and_the_reapings_goods_are_public` (`ti4_model::view`) |
| neutrality | no Obsidian | done | `a_game_without_the_obsidian_is_untouched_by_every_window` |
| flip then use | Firmament game | done | `a_firmament_game_flips_and_then_the_viper_hollow_the_reaping_and_the_notes_work` (real flip, `FACTION_FLIPPED` flush through an armed resolver, then Viper Hollow, The Reaping, Neural Parasite) |

## Design

* **Hooks**: `timing_abilities` (Planesplitter, Neural Parasite, Viper Hollow, Vos Hollow, Malevolency x2, The Reaping x3), `commander_unlocked`, `leader_action`/`use_leader`. The module is in `MODULES` (27); `supply::staging_enabled` already held for an `obsidian` seat.
* **Planesplitter**: `STRATEGIC_ACTION_BEGAN` (when), optional ability (choosing it is the consent), then one question of `from>to` moves: any ingress token into a regular-map system that holds none (rule 14) and contains or is adjacent to the owner's units. Needs The Fracture in play. Without a map only systems that hold the owner's units qualify.
* **Neural Parasite**: `TURN_BEGAN` (after), mandatory; the owner is asked which only when more than one distinct infantry (system, place, owner, type) is in reach; a single one is forced. Destroys through `hooks_ground::stage_ground_force_destroyed` (planet) so the destruction is announced like any other.
* **Viper Hollow**: `FACTION_FLIPPED` (after), mandatory. On each planet where the owner has the mech and is coexisting, `deepwrought::begin_coexisting(controller, ..., Some(owner))` (coexistence 3.2: the owner gains control, the controller coexists, the planet is exhausted), then `PLANET_CONTROL_GAINED` is staged and `technology::control_gained` runs as for any gain.
* **Vos Hollow**: `SHIP_DESTROYED` (when), optional. Opponents of the victim are the other seats that still have a ship of the destroyed type in that system; with several the owner chooses, and the opponent chooses which of their ships (undamaged/damaged variants) when they differ. Destroyed through `combat::destroy_units_with_context` (announced by the game's staged destruction).
* **Aroz Hollow**: unlock in `commander_unlocked` (any unit, space or planet, in a Fracture system). The +1 is the pre-existing `borrowed_commanders::unit_roll_modifier`, already added for every seat through `promissory::has_commander_ability` (own unlocked commander, Alliance copy, Yin/Mahact grants); this module deliberately adds no second one (a first draft did and doubled it). It requires `fracture_in_play`.
* **Sharsiss Hollow**: readies every exhausted controlled planet and ocean card; the generic caller purges. Offered only to an unlocked Obsidian with something exhausted.
* **Malevolency**: give = `TACTICAL_ACTION_ENDED` (after), optional, usable by whoever holds the note (owner included): the neighbour (`transactions::neighbours`) is asked, then `production::pay_seeing(1, Influence)` (restored if unpaid), then `promissory::take`. Levy = `STATUS_PHASE_ENDED` (after), mandatory, holder not the Obsidian: `fleet_tokens -= 1` when above zero. **`game.rs`**: `TACTICAL_ACTION_ENDED` was announced only when a T'ro/Mercer agent was readied; one added condition, `|| factions::obsidian::listens_for_tactical_end(&self.state)` (true only while the note exists), makes it reach Malevolency. No other game.rs line changed and the other session's round-income hunks were not touched.
* **The Reaping**: the goods are the existing mark `firmament:sowing:<player>` (`firmament::goods_on_card`, new helpers `add_goods_to_card` and `take_goods_from_card`). +1 per combat won against a puppeted player (space: `SPACE_COMBAT_WON` `opponents`; ground: `GROUND_COMBAT_ENDED` `winner` against the other side). At `STATUS_PHASE_BEGAN` the card empties, then two staged gains of that many each (the card's goods, then the equal amount from the supply).
* **Nocturne**: `seating::seeded_faction_assignments` (the only roster-choosing primitive) returns the new `FactionAssignmentError::NotChoosable`. `seating::deploy` and `fixtures::seated_game` still seat it because tests and the soak need a direct seat.
* **The Blade's Orchestra**: its body is `firmament_flip::become_obsidian` (unchanged); the ability is claimed here and tested through the real flip. For a directly seated Obsidian there is nothing to flip.
* **Decision sites**: `obsidian.rs::ask` is the one `Choice::new` + `ask_seeing` site, registered in `tests/decision_delivery_inventory.rs` (producer and direct-delivery rows). Payment goes through `production::pay_seeing`.

## Open questions and limits

1. **Direct seat**: Nocturne forbids choosing the Obsidian at setup, so no legal game seats it. The fixture and the soak seat it directly only for coverage; a directly seated Obsidian has no plots (nothing in its own text places one), so Marionettes, The Reaping's trigger and Heaven's Eye-style puppeting never fire there.
2. **Reaping, "each time you win a combat against a puppeted player"**: one trade good per combat won, even if several opponents were puppeted in one wide space combat; a retreat that leaves one side a winner counts (the engine's own `SPACE_COMBAT_WON`), a draw does not.
3. **Neural Parasite**: infantry in a system's space area (aboard ships) count as "in" the system, both as the owner's source infantry and as the target; destroying such a unit emits no `GROUND_FORCE_DESTROYED` (the staging helper needs a planet). Adjacency uses the map; without one only the same system.
4. **Vos Hollow**: "any combat" is read as `during_space_combat` (ships exist only in space combat; Anti-Fighter Barrage and other non-combat destructions are not). The agent is offered only when an opponent still has a ship of that type.
5. **Planesplitter**: "when you perform a strategic action" is `STRATEGIC_ACTION_BEGAN`; an ingress token may not be moved into a Fracture system (ingress tokens sit on the regular map).
6. **Malevolency**: the Obsidian's Hollows print 0 influence, so the 1 influence is paid with a trade good or another planet. "Neighbors" is `transactions::neighbours` (not Hacan's Guild Ships reach). The levy does nothing with an empty fleet pool. A returned note is simply held by its owner again.
7. **Viper Hollow** acts on every planet where the owner has a Viper and is coexisting when the card flips; the 3.2 exhaustion applies to each.
8. **Sharsiss Hollow** is not offered when no planet is exhausted.
9. **Nekro Z**: `obsidian_flagship` is not added to `nekro::LENDABLE_FLAGSHIPS`: Heaven's Hollow prints no ability text, so there is nothing to lend.
10. **Found, not fixed (outside this package)**: `TACTICAL_ACTION_ENDED` is also what Ral Nel's Miniaturization hangs on, and `game.rs` announced it only for T'ro/Mercer; its tests emit the event by hand. With this package's condition it is still announced only when an agent or Malevolency is in play, so Miniaturization appears not to fire in a real game. The coordinator may want one more `listens` condition for Ral Nel.

## Shared files touched

`factions/mod.rs` (module registered, `MODULES` 27), `factions/firmament.rs` (+`add_goods_to_card`, `take_goods_from_card`), `seating.rs` (`NotChoosable`, Nocturne check), `game.rs` (one added `||` condition at the `tro_window` assignment), `tests/decision_delivery_inventory.rs` (`obsidian.rs::ask`), `ti4-sim/examples/base_faction_soak.rs` (obsidian added, 27 factions).

## Results

* `cargo test -p ti4-model -p ti4-engine -j1 -- --test-threads=12`: engine lib 2772 passed / 1 ignored, then 1, 4, 5 passed; model 86 passed; 0 failed (`out/ob_engine_full1.log`).
* `cargo test -p ti4-policy -p ti4-sim -j1 -- --test-threads=12`: 273 and 51 passed (1 ignored), 0 failed (`out/ob_policy_sim1.log`).
* Ledger: obsidian 12/12, firmament 11/11, all other lines unchanged (`out/ob_ledger1.log`).
* Soak `obsidian 0 25 10` (release): 25 games, 0 failures (`out/ob_soak1.log`); `firmament 0 25 10`: 25 games, 0 failures (`out/ob_soak_firmament.log`).
* Haiku helpers: none used.
