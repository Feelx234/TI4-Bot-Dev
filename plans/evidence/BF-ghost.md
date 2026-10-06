# BF-ghost: The Ghosts of Creuss

Branch `wp/base-factions`, uncommitted. Code: `crates/ti4-engine/src/factions/ghost.rs` (only file edited besides this one).
Card text: `crates/ti4-content/content/*.json` at `DEFAULT` (latest printing).

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | `creuss_gate` | done (seating placement already existed; verified here) | `the_creuss_gate_holds_the_home_position_and_the_home_system_sits_beside_it` |
| ability | `quantum_entanglement` | done (`linked_systems`) | `quantum_entanglement_links_every_alpha_and_beta_system_for_the_creuss_player_only`, `..._survives_the_wormholes_being_switched_off` |
| ability | `slipstream` | done (`move_bonus`) | `slipstream_adds_one_from_the_home_system_and_from_alpha_and_beta_systems`, `slipstream_reaches_the_value_a_ship_is_offered` |
| tech | `ds` Dimensional Splicer | done (`produced_hits`, `CombatStart`, producer-assigned) | `dimensional_splicer_produces_a_hit_the_producer_assigns`, `..._needs_a_wormhole_the_technology_and_the_start_of_combat` |
| tech | `wg` Wormhole Generator | done (component action) | `the_wormhole_generator_places_a_token_and_exhausts`, `..._is_refused_without_the_card_or_a_legal_system_and_changes_nothing` |
| unit | `ghost_flagship` Hil Colish | done (`extra_wormholes`) | `hil_colish_gives_its_system_a_delta_wormhole_for_every_player`, `hil_colish_follows_the_ship_and_leaves_games_without_it_alone` |
| unit | `ghost_mech` Icarus Drive | done (timing ability, `SYSTEM_ACTIVATED` after) | `icarus_drive_trades_the_mech_for_a_token_in_its_own_system`, `icarus_drive_is_not_offered_without_a_mech_a_map_or_a_place_for_a_token` |
| promissory | `iff` Creuss IFF | done (timing ability, `TURN_BEGAN` after, action phase) | `creuss_iff_places_a_token_and_returns_to_the_creuss_player`, `creuss_iff_is_not_offered_to_the_owner_off_turn_or_outside_the_action_phase` |
| leader | `ghostagent` Emissary Taivra | done (timing ability + `linked_systems`) | `emissary_taivra_makes_the_activated_system_adjacent_to_every_wormhole_system`, `emissary_taivra_needs_a_ready_agent_and_a_non_delta_wormhole` |
| leader | `ghosthero` Riftwalker Meian | done (`use_leader`, `movement::apply_map_edit`) | `riftwalker_meian_swaps_two_systems_and_the_map_follows`, `riftwalker_meian_refuses_the_gate_the_home_system_the_nexus_and_fracture_systems` |
| leader | `ghostcommander` Sai Seravus | done, claimed (committed d69bba8e; uses `SHIP_MOVED.wormholes` and `MOVEMENT_FINISHED`) | `sai_seravus_places_one_fighter_per_capacity_ship_after_the_whole_move`, `sai_seravus_needs_the_unlocked_commander_a_wormhole_and_unused_capacity`, `sai_seravus_places_a_fighter_after_a_real_quantum_wormhole_move`, `sai_seravus_unlocks_with_units_in_three_alpha_or_beta_systems` |
| breakthrough | `ghostbt` Particle Synthesis | done, claimed (committed 8a9e437d; `extra_production` + `production_cost_reduction`) | tests in `factions::ghost` (particle synthesis) |

Regression: `a_game_without_a_creuss_seat_is_unchanged_and_offered_nothing` (adjacency equals `Galaxy::adjacent` for
every system and player, no `linked_systems`, no `faction|ghost|` component action, hero not offered, no Slipstream bonus,
no flagship wormhole, and `SYSTEM_ACTIVATED` / `TURN_BEGAN` windows ask nothing and change nothing).

## Hook requests

1. **Sai Seravus effect** ("For each ship that has a capacity value and moved through 1 or more wormholes, you may place 1
   fighter ... with that ship if you have unused capacity in the active system"). `SHIP_MOVED` carries `player, system,
   origin, unit` but not whether the route used a wormhole, and the route is not recoverable from origin/destination
   (a hex-adjacent pair may still have been routed through one). Request: `game.rs note_arrival`: add a payload key
   `wormholes` (count of wormhole links the ship's chosen path crossed; the route is computed in `begin_one_move`) and
   the ship's index/loaded capacity, or a typed `SHIPS_MOVED_DONE` event after the last ship. Then an `After SHIP_MOVED`
   ability can place fighters with `action_cards::place_units_counted`.
2. **Particle Synthesis** ("Each wormhole in a system that contains your ships gains PRODUCTION 1 as if it were a unit you
   control. Reduce the combined cost of units you produce in systems that contain wormholes by 1 for each wormhole in
   that system"). No production hook: `hooks_economy` has no way to add PRODUCTION capacity to a system without a unit
   nor a combined-cost reduction. Request: `EconomyHooks::extra_production: fn(&GameState, &ContentStore, &PlayerId, &SystemId) -> i64`
   (read in `production::capacity`) and `EconomyHooks::production_cost_reduction: fn(&GameState, &PlayerId, &SystemId) -> i64`
   (applied to the combined cost). `hooks_movement::wormholes_at` already gives the wormhole count.
3. **Hero swap applies at step end.** `TimingContext::galaxy` is `&Galaxy`, so `hero_swap` records the edit in the state
   (`faction_marks["map_edit|NNNN"]`, via `apply_map_edit` on a clone) and `game.rs` step replays it onto the live map
   (line ~1082, already wired). Anything later in the *same* step reads the old layout. If that matters, pass
   `Option<&mut Galaxy>` or have the caller replay after `use_leader`.

## Rules questions

- Hil Colish "may move before or after your other ships": nothing to implement; ships move individually in any order and
  every offer reads the board as it stands, so the flagship's delta moves with it. Claimed on that basis, no ordering test
  beyond the board-derived wormhole follows-the-ship test.
- Hero "other than the Creuss system, Ahk Creuxx system": read as tiles 17 and 51 both (and the Nexus `82*`, Fracture
  `fracture*`). If "Creuss system" meant only the home system, tile 17 would be swappable.
- Hero eligibility ("contain wormholes or your units") uses the engine's wormholes (`wormholes_at` or the galaxy's kinds,
  including tokens and the flagship's delta) and counts ground forces and ships.
- Emissary Taivra: "adjacent to all other systems that contain a wormhole" is implemented for **every** player and for
  every wormhole kind (delta, gamma included), not only the activating player; it is not silenced by the wormhole-off
  switches (the printed text has no "cannot be prevented", unlike Quantum Entanglement). If it should be silenced, the
  hook needs `galaxy.wormholes_off`.
- A token (Wormhole Generator, IFF, Icarus Drive) cannot go into tile 51 (off the grid; `place_creuss_token` requires a
  hex). 51 holds a controlled planet and would qualify for a placement by the printed text.
- The Wormhole Generator ACTION is offered whenever the card is ready (no map in `component_actions`); `perform_component`
  returns false with no change if the map gives no destination. Only a pathological board (every non-home system holding
  another player's ships, no controlled planet outside 51) would be offered an action that does nothing.
- The hero offer (`leader_action`, no map) counts eligible systems from the board record (systems with the owner's units
  or wormholes already in play), a subset of what `hero_swap` may pick from; with fewer than two such systems the hero
  is not offered even if the map has wormhole tiles.
- Icarus Drive's choice lists each (system, token) for every system holding a mech, so a mech in a system already holding
  one token offers only the other token.

## Decision sites to register

`crates/ti4-engine/src/factions/ghost.rs`, function `ask` (the single helper every Ghost question goes through:
Wormhole Generator destination, IFF destination, Icarus Drive, Dimensional Splicer, hero first and second system):
`Choice` 1, `AskObserved` 1. No other function builds or asks a `Choice`. (`tests/decision_delivery_inventory.rs` currently
fails `every_producer_and_delivery_site_matches_the_reviewed_registry` because this site, plus other agents' new sites, is
unregistered; I did not edit that file.)

## Commands and exact results

- `cargo test -p ti4-engine --lib -- factions::ghost` -> `21 passed; 0 failed`.
- `cargo test -p ti4-engine --lib -- factions::` -> `185 passed; 0 failed; 1 ignored`.
- `cargo clippy -p ti4-engine --all-targets`: no warning mentions `ghost.rs` (the crate shows 26 warnings elsewhere, others' files).
- `rustfmt --edition 2024 crates/ti4-engine/src/factions/ghost.rs`: clean.
- `cargo test -p ti4-engine -q --no-fail-fast`: lib `1694 passed; 0 failed; 1 ignored`; integration binaries `1`, `5` passed;
  `decision_delivery_inventory`: `3 passed; 1 failed` (`every_producer_and_delivery_site_matches_the_reviewed_registry`,
  unregistered decision sites; mine are listed above, others' are in their evidence).
- ti4-sim not run.

## Ledger (`print_faction_ledger`)

```
ghost     10/12 implemented
    Leader ghostcommander
    Breakthrough ghostbt
```

## Review follow-up

- Quantum Entanglement now lifts `nexus_wormholes_off` on a copy of the galaxy before reading kinds (test `quantum_entanglement_ignores_nexus_sovereignty`). `factions::ghost`: 22 passed; clippy: no ghost.rs warnings; rustfmt clean.
- Hook request: `Hooks::component_actions` needs a `&Galaxy` (or `Option<&Galaxy>`) so Wormhole Generator is offered only when `tokens::wormhole_generator_destinations` is non-empty. Until then it is offered when ready; `perform_component` is atomic (returns false, nothing changed, card stays ready) if no destination exists.

## Update after shared routes (F3/F4)

Hook requests 1 and 2 above are resolved: both items were implemented in commits d69bba8e and 8a9e437d. Verified now: `factions::ghost` 27 passed; no clippy warning in ghost.rs; rustfmt --check clean. Ledger: `ghost     12/12 implemented`. No new decision sites beyond `ask`.
