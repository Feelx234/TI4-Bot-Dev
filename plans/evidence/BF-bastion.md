# BF-bastion (Last Bastion, alias `bastion`)

Code: `factions/bastion.rs` (Galvanize core, Liberate, Phoenix Standard, Raise the Standard, The Icon, commander unlock), `factions/bastion_units.rs` (Egeiro, A3 Valiance, Helios docks, Proxima Targeting VI, Dame Briar, Lyra Keen). Ledger: bastion 14/14 implemented.

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | galvanize | done | `galvanizing_marks_one_unit_and_the_seven_tokens_run_out`, `galvanize_candidates_are_distinct_values_of_one_owner`, `a_galvanized_ship_rolls_one_extra_combat_die`, `a_galvanized_unit_rolls_one_extra_die_for_each_unit_ability`, `galvanized_ground_forces_roll_an_extra_die_in_ground_combat` |
| ability | liberate | done | `liberate_places_one_infantry_until_they_match_the_resources_then_readies`, `liberate_counts_the_helios_resource_bonus_and_is_the_bastions_alone` |
| ability | phoenixstandard | done | `phoenix_standard_*` (space, ground, no token) |
| technology | proxima | done | `proxima_cancels_one_bombardment_hit_per_galvanized_unit_on_the_planet`, `proxima_bombards_at_the_start_of_a_live_ground_combat_round_and_back`, `proxima_is_optional_and_never_emitted_without_the_technology` (live `InvasionWindow`) |
| technology / unit | helios2, bastion_spacedock(2) | done (data-driven + resource bonus) | `the_helios_docks_add_to_resources_production_and_free_fighters` (PRODUCTION, 3 free fighters, real upgrade route) |
| unit | bastion_flagship | done | `the_egeiro_adds_one_per_non_home_system_with_a_controlled_planet`, `a_nekro_flagship_with_the_z_token_gains_the_egeiros_bonus` |
| unit | bastion_mech | done | `a_destroyed_galvanized_mech_galvanizes_up_to_three_infantry_in_its_system` |
| promissory | raisethestandard | done | `raise_the_standard_galvanizes_for_the_holder_and_returns_home`, `..._returns_even_when_no_token_is_left` |
| breakthrough | bastionbt | done | `the_icon_places_every_ship_of_one_use_in_one_qualifying_system_and_exhausts`, `the_icon_needs_a_token_a_ground_force_and_no_rival_ship_and_is_all_or_nothing` (real `ProductionWindow`) |
| leader | bastionagent | done | `dame_briar_*` |
| leader | bastionhero | done | `lyra_keen_*`, `a_galvanized_ship_lost_in_a_real_space_combat_is_reported_galvanized_and_gives_the_hero_a_window` (real `CombatWindow`) |
| leader | bastioncommander | done | `the_commander_unlocks_with_three_galvanized_units_of_any_player`, `the_commander_shields_action_cards_from_sabotage_including_through_alliance` (incl. faceup Alliance), `nekro::nip_and_tuck_bars_every_assimilator_token_on_its_owners_components` |
| neutrality | no Last Bastion | done | `games_without_the_last_bastion_are_untouched_by_every_window` |

## Galvanize: interpretation

* State: `Unit::galvanized` (model, pre-existing); helpers `bastion::{galvanize, galvanized_on_board, tokens_left, galvanizable_in_system, extra_die}`.
* Effect: +1 die for each combat roll and each unit-ability roll the unit already makes (combat, ANTI-FIGHTER BARRAGE, SPACE CANNON offense and defense, BOMBARDMENT; matches `combat_modifiers.json`). Added at the six roll sites; a unit with no such roll gains none. Proxima's technology bombardment is not a unit ability: no extra die.
* End: no text ends it; it stays with the unit and goes back to the 7-token supply when the unit leaves the board. Shared supply: no galvanizing at 7 galvanized units on the board (any player's); an already galvanized unit takes no second token.
* The token is gone with a destroyed unit, so `SHIP_DESTROYED` / `GROUND_FORCE_DESTROYED` carry `galvanized: true` (key present only when true; ships through `bastion:lost:*` marks written by `combat::remove_combat_ship`, ground forces from the unit in `invasion.rs` and the staged rows of `hooks_ground.rs`).

## Other decisions

* Liberate: planet resource value is `production::planet_value_now` (includes the Helios +1/+2). Space stations are not planets. Infantry come from reinforcements; none placed if none left.
* Phoenix Standard / Raise the Standard: "units that participated" = the player's ships in the space area (space combat) or ground forces on the planet (ground combat) still standing at `SPACE_COMBAT_ENDED` / `GROUND_COMBAT_ENDED`. Raise the Standard has no "may": it resolves at the first combat end the holder fights in, and returns even with no token or no candidate.
* The Icon: all ships of one use of PRODUCTION go to one system (the first remote placement fixes it; a ship placed at home first rules it out for the use). Remote spots are `<system>@space`, exhaust happens on placement; readies at `STATUS_PHASE_ENDED`. Fleet and capacity are checked in the final system (existing remote-placement route).
* A3 Valiance: "up to 3" is as many as tokens and infantry allow; infantry in the system's space area and on its planets; not optional.
* Dame Briar: window "When"; offered only if the victim has another galvanizable unit in that system.
* Lyra Keen: dice in seat order (rivals' space-area units, then each planet's); a die at or above the destroyed unit's printed combat value destroys that unit (ships via `combat::destroy_units_with_context`, ground forces via staged `GROUND_FORCE_DESTROYED`, structures removed with no event).
* Proxima: cancellation applies to the one bombardment of a planet (budget spent over the unit groups), to Harrow, and to its own roll back at the holder. New event `GROUND_COMBAT_ROUND_BEGAN`, emitted by the live invasion window only while a side holds Proxima.
* Helios: PRODUCTION and fighter support were already data-driven; `planet_value_now` adds the planet bonus (`bastion_units::resource_bonus`).
* Nip and Tuck: Sabotage immunity pre-existed in `reactions.rs` through `has_commander_ability`; Nekro `assimilation_options` now offers no X/Y/Z token for a source with the commander ability (a plain `gain` stays). `bastion_flagship` added to `nekro::LENDABLE_FLAGSHIPS`.

## Open questions

1. "Make an identical roll against your ground forces" (Proxima): read as a second roll of BOMBARDMENT 8 (x3) with fresh dice, hits assigned to the holder's own ground forces, after the technology's own cancellation. The alternative (reuse the same faces) is not implemented.
2. Retreated units are not tracked as "participated" (Phoenix Standard, Raise the Standard): only units still in the combat location qualify.
3. Proxima's cancellation is applied only when one player's ground forces are on the planet (single victim); coexistence with several victims is not reduced. The synchronous test-only `ground_combat` does not open the Proxima round-start window (the live window does).
4. Destroyed structures have no destroyed-event in the engine, so Dame Briar and Lyra Keen do not react to a destroyed PDS or dock.
5. Raise the Standard resolves automatically (operator ruling: only "may" is optional), at the first combat the holder fights, which may not be the one the holder would have chosen.
6. The Nekro tech deck (`techs_pok_c4`, `technology::active_aliases`) lacks Thunder's Edge faction techs, so a Nekro is never offered an X/Y token on Proxima; the Egeiro is lendable through Z.
7. Saar's commander also places fighters remotely; if such a dock is also an Icon system the Icon exhausts on that placement.
8. Content: `leaders.json` also tags `orlandohero` with faction `bastion` (source `pok`); it is not on the sheet and is not a ledger asset.

## Shared files touched

`combat.rs` (extra dice, `remove_combat_ship` note, `ship_destroyed_payload` now `&mut GameState` + `galvanized`), `invasion.rs` (extra dice, payloads, Proxima cancellation, round-start event), `production.rs` (`planet_value_now` bonus, Icon spots and exhaust), `reactions.rs` (one call site), `factions/{mod,hooks_ground,nekro}.rs`, `tests/decision_delivery_inventory.rs` (`bastion.rs ask`), `ti4-sim/examples/base_faction_soak.rs`. `game.rs` untouched.

## Results

* `cargo test -p ti4-engine -j1 -- --test-threads=12`: lib 2571 passed / 1 ignored, content_ids_resolve 1, decision_delivery_inventory 4, 5 more, 0 failed.
* `cargo test -p ti4-policy -p ti4-sim -j1`: 273 and 51 passed (1 ignored), 0 failed.
* Ledger: all factions full; bastion 14/14.
* Soak `bastion 0 25 10`: 25 games, 0 failures.

## Operator rulings 2026-10-07

1. **Retreated units did participate.** `combat::retreat_to` now calls `bastion::note_retreat` (only when a Last Bastion seat exists): mark `bastion:retreated:<system>:<owner>` = `<destination>|<type>:<damaged>,...` for the retreating **ships** (fighters are ships; carried ground forces did not take part in a space combat and are not recorded). `combat` clears it with `bastion::clear_retreated` right after `SPACE_COMBAT_ENDED`. `bastion::participants` adds the matching ungalvanized ships standing in the destination (Located there) for Phoenix Standard and Raise the Standard. Open question 2 above is resolved. Other "participated" consumers: Devour, Reveal Prototype, Ipswitch and Brother Milor read participation at combat start or are ground-only, so they already count retreaters; Nekro Alastor ground forces still leave the combat on retreat (unchanged). No other card uses the end-of-combat definition.
   Tests: `the_last_bastion_can_galvanize_a_ship_that_retreated_from_the_combat` and `a_raise_the_standard_holder_that_retreated_galvanizes_a_retreated_ship_and_returns_it` (real `CombatWindow` route, retreat announced), `a_retreated_ship_is_a_candidate_located_in_its_destination` (ships and carried fighter yes, infantry no, cleared), `a_game_without_a_last_bastion_keeps_no_retreat_note`.
2. **Thunder's Edge technologies are researchable.** `technology::active_aliases(content, sources)`: deck `techs_pok_c4` plus every `thunders_edge`-source technology record when `sources` has Thunder's Edge. Content check: the 13 TE records are all faction techs (no generic card or replaced printing; Keleres `executiveorder` was already in the deck), `decks.json` has no TE deck beyond `techs_basete` (base only). Callers: `researchable` (has sources), Nekro `assimilation_options`/`can_take_from` (sources now threaded from `context.sources`), `nekro_units::gainable_technologies` (sources from Flayesh), Sardakk test. A PoK-only set is unchanged (test). Tests: `thunders_edge_faction_techs_are_active_only_when_the_expansion_is_in_play`, `a_thunders_edge_faction_can_research_its_own_faction_tech_and_no_one_else_can`, `a_nekro_is_offered_a_thunders_edge_faction_tech_through_the_singularity` (offer x|b|proxima, real SHIP_DESTROYED route, text gained not owned). Open question 6 above is resolved. Soak: TE factions now research their own faction techs in FULL-source games, so soak trajectories for TE seats shift.

### Results (rulings)

* `cargo test -p ti4-engine -j1 -- --test-threads=12`: lib 2578 passed / 1 ignored, then 1, 4, 5 passed, 0 failed.
* `cargo test -p ti4-policy -p ti4-sim -j1`: 273 and 51 passed (1 ignored), 0 failed.
* Ledger print: bastion 14/14, nekro 12/12.
* Soaks `bastion 0 25 10` and `nekro 0 25 10`: 25 games each, 0 failures. Logs `out/te_tech_*.log`.

## Operator rulings 2026-10-07 (recorded by coordinator)

* Proxima "identical roll": a fresh roll of BOMBARDMENT 8 (x3) against the holder's own ground forces (as implemented).
* `orlandohero` (F.S.S. Orlando): variant piece, disregarded; not a ledger asset.
* Retreated units participated; Thunder's Edge technologies join the active deck (implemented above).
