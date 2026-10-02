# BF-00e-movement: movement and adjacency routes for faction modules

Branch `wp/base-factions`, uncommitted. Content source: `crates/ti4-content/content/*.json` at `DEFAULT`.
Hooks live in `crates/ti4-engine/src/factions/hooks_movement.rs` (each documented there with card text,
call point and timing). All hooks are empty in the shipped modules, so play is unchanged (see Neutrality).

## Items

| # | Item | Status | What was added |
|---|---|---|---|
| 1 | Per-player extra adjacency | done (consumers outside my files not rerouted, see Requests) | `MovementHooks::linked_systems(state, content, sources, &Galaxy, &PlayerId) -> Vec<(String,String)>` (symmetric pairs for that player only, ignores `wormholes_off`); `MovementHooks::extra_wormholes(&GameState) -> Vec<(system, kind)>` (universal, for Creuss flagship). `movement::PlayerAdjacency::new(state, content, sources, galaxy, player)` with `.neighbours()/.are_adjacent()` is the one helper for player-aware adjacency queries. `MovementRules::with_laws` applies both whenever `Board.mover` is set (new field, set by `Board::for_player`). |
| 2 | Wormhole tokens, delta | done | Delta already linked by kind (tiles 17 and 51 print DELTA): tested. `tokens.rs`: `CREUSS_TOKENS`, `place_creuss_token(state, galaxy, kind, system) -> Result<Option<SystemId>, WormholeTokenError>`, `remove_creuss_token`, `creuss_token_at`, `wormhole_generator_destinations(state, content, sources, galaxy, player)` (the `wg` condition). State is the existing `GameState::wormhole_tokens` (keys `ALPHA`/`BETA`; `laws::apply_to_galaxy` already turns it into adjacency for everyone). `Galaxy::wormhole_systems()` added (galaxy.rs). `hooks_movement::wormholes_at(state, system)` and `has_alpha_or_beta` are state-only queries for modules. |
| 3 | Creuss Gate / home system | done | `seating::CREUSS_GATE = "17"`, `CREUSS_HOME = "51"`. `deploy` puts a `ghost` seat in tile 51 (the corpus `homeSystem` says 17, which has no planet); `seat.home_system = 51`. `build_board` already places tile 17 in the home slot (the corpus names it); `seating::place_creuss_home` registers 51 off the map (idempotent, only when 17 is on the grid), called from `place_wormhole_nexus`, which every map family (including the training pool path, rollout.rs:402) already calls. 51 is adjacent to 17 only. Boards without a Creuss seat: identical (tested). |
| 4 | Move value per ship | done for Slipstream; partial for `winnubt` | `MovementHooks::move_bonus(&GameState, &MoveSite{player, origin, index: Option<usize>, ship: &UnitType}) -> i32`, summed, added in `tactical::effective_move_value_for_ship` (new; `effective_move_value_with_boosts` delegates with `index: None`) before Gravleash. `movable_into` passes the index. Winnu's one-ship choice needs `game.rs` to pass the index (Requests). |
| 5 | Move through other players' ships | done in `tactical.rs`/`movement.rs`; `game.rs` call needs a one-line change | `MovementHooks::may_move_through_ships(state, content, sources, &PassSite{player, active, ship_type}) -> bool` (any module allows). Evaluated once in `with_laws` for each ship type the mover owns in space. New `MovementRules::path_from_ship / can_reach_ship / can_pass_through_ship(.., ship_type)`; `path_from`/`can_reach` stay type-blind. `movable_into` and `preview_moves` use the typed versions. |
| 6 | Out-of-turn movement | partial | `transit::relocate_ships(state, content, sources, galaxy, &Relocation) -> Result<Relocated, RelocateError>`: atomic, checks ships present (copies counted), ship-ness, both systems on the map, optional adjacency (`PlayerAdjacency`, so QE counts), optional "no foreign ships at destination", optional refusal on fleet/capacity overflow (`fleet::standing_using`; otherwise the excess is reported in `Relocated`). `transit::announce_relocation(state, &mut Resolving, &Relocated)` emits the new typed event `SHIPS_RELOCATED` (payload `player, from, to, count, ships[], reason`) through the timing windows. `SHIP_MOVED` carries `player` only (no system); adding keys is in `game.rs` (Requests). A module inside a timing hook cannot open a nested window (the resolver is mid-emit), so Foresight performs the move and the caller announces. Not done: Foresight's strategy-token spend (strategy-pool API is another agent's). |
| 7 | Supernova / tile mutation | partial | `MovementHooks::may_enter_supernova(state, content, sources, &PlayerId) -> bool` sets `supernovae_open` (86.1 lifted, as Magmus Reactor does). Map edits: `movement::MapEdit::{Swap, Replace}`, `apply_map_edit(state, galaxy, content, sources, &edit)` (atomic; changes `Galaxy` via new `swap_systems`/`replace_system`, records `faction_marks["map_edit|NNNN"]`, and for Replace moves the old tile's space units and command tokens to the new tile id, keeps the frontier token, returns Creuss wormhole tokens, purges the ion storm), `replay_map_edits` (idempotent via `Galaxy::edits_applied`), `NOVA_SEED = "81"`. Planet cards/control on the replaced tile, other players' units, and Mirage/Thunder's Edge tokens are the hero module's job (no state fields for the last). Persisting across a re-derived map needs the replay call in `laws::apply_to_galaxy` (Requests). |

## Neutrality

With `MODULES` empty every hook returns its neutral value and `with_laws` clones nothing. Verified by test
`movement::tests::empty_modules_change_no_route_for_any_player` (paths at move 0..3 for two players and
`PlayerAdjacency` equal `Galaxy::adjacent` on a hub), `hooks_movement::tests::empty_tables_are_neutral`
(also asserts no real module registers any hook), `seating::tests::boards_without_a_creuss_seat_are_unchanged`.
Option ids, order, dice and board for the six in-scope factions: no code path changed for them. Coordinator still
owns the ti4-sim integrity run (not run by me).

## Tests added (all in the owning file)

- `factions::hooks_movement`: `empty_tables_are_neutral`, `bonuses_sum_links_normalise_and_permissions_need_one_module`, `wormholes_at_reads_tile_tokens_storm_nexus_and_module_wormholes`, `apply_extra_wormholes_merges_once`
- `movement`: `empty_modules_change_no_route_for_any_player`, `a_players_extra_link_shortens_only_their_routes`, `a_wormhole_a_piece_carries_links_for_everyone` (incl. Travel Ban silences it), `supernova_passage_is_per_player_and_needs_the_hook`, `a_ship_type_that_passes_blockades_routes_through_them`, `map_edits_swap_and_replace_record_and_replay`, `a_refused_map_edit_changes_nothing`, `the_creuss_gate_and_home_are_adjacent_for_everyone_by_their_delta_wormholes`
- `tactical`: `a_hook_move_bonus_reaches_what_the_printed_value_cannot_and_is_neutral_without_one`, `a_ship_type_that_passes_blockades_is_offered_the_move_the_others_are_refused`
- `tokens`: `a_creuss_token_links_its_system_to_every_wormhole_of_its_kind`, `a_refused_placement_changes_nothing`, `wormhole_generator_offers_controlled_planets_and_ship_free_non_home_systems`
- `transit::relocation_tests`: `ships_move_between_adjacent_systems_and_are_reported`, `every_refusal_leaves_the_state_untouched`, `the_flags_relax_adjacency_and_the_foreign_ship_bar`, `a_hook_adjacency_link_makes_a_far_system_legal_for_its_player_only`, `fleet_overflow_is_refused_only_on_request`, `the_announcement_reaches_a_timing_ability_with_its_payload` (checks the event appears in the resolver log; no faction ability exists yet to consume it)
- `seating`: `the_creuss_gate_sits_in_the_seats_home_position_and_the_home_system_is_off_the_map`, `boards_without_a_creuss_seat_are_unchanged`, `placing_the_creuss_home_twice_changes_nothing`, `a_creuss_seat_starts_in_tile_51_not_on_the_gate`
- `ti4-content` `galaxy`: `swapping_two_systems_exchanges_their_hexes_and_their_neighbours`, `a_refused_swap_or_replacement_leaves_the_map_alone`, `replacing_a_system_puts_the_new_tile_on_the_same_hex`, `wormhole_systems_lists_printed_token_and_off_map_holders`

## Requests (files I may not edit)

1. `game.rs` `begin_one_move` (~2469-2495): look the route up with `rules.path_from_ship(origin, move_value, Some(ship.type_id.as_str()))` and compute the value with `tactical::effective_move_value_for_ship(.., Some(index), gravity_drive, ionian)`. Without this a Corsair/Yssaril flagship/Winnu one-ship move offered by `movable_into` would be refused late (`UnknownSystem`). The same call at ~8725 (`effective_move_value_with_gravity`) and the Gravleash anchor at ~2503 omit the hook bonus.
2. `game.rs` `note_arrival` (~2430): add `system` (the active system) and, if available, `origin` to the `SHIP_MOVED` payload; Naalu Foresight ("a system that contains 1 or more of your ships", "from the active system") needs it. Additive key.
3. `laws.rs` `apply_to_galaxy`: after the token rebuild call `crate::factions::hooks_movement::apply_extra_wormholes(state, galaxy)` (Creuss flagship's delta for non-movement adjacency: space cannon, retreat, neighbours) and `crate::movement::replay_map_edits(state, galaxy, content, sources)` (needs content/sources arguments, or call it from the three `apply_to_galaxy` call sites in game.rs/seating). Movement already applies the wormholes itself.
4. Other player-aware adjacency consumers still read `Galaxy::adjacent` and should use `movement::PlayerAdjacency`: `transactions.rs:199` (neighbours, Quantum Entanglement), `combat.rs` 1574/2482/2528/2558 (space cannon/retreat reach), `action_cards.rs` 2835/3446/5586, `faction_abilities.rs:586`, `relics.rs:549`, `leaders.rs:990`, `choice.rs:855`, `secrets.rs:514/536`, `diplomacy/candidates.rs`, `agenda_effects.rs:701`. `Observed::can_reach` already goes through `with_laws` and is covered.
5. Creuss `home_system`: `factions::get("ghost").home_system()` still returns "17" (corpus). Callers that use the faction record rather than `Player::home_system` (`objectives.rs:513/570/732`, `laws.rs:524`, `agenda_effects.rs:751/1024`) will treat the gate as the Creuss home. The Creuss Gate is "not a home system"; these should use the seat's `home_system` (now 51).
6. Winnu `winnubt`: one-ship +1 is a choice after activation; either a third boost flag on `Movable`/`MoveSelection` (like Ionian) in `tactical.rs` + `game.rs`, or the module marks the chosen ship in `faction_marks` and matches `MoveSite::index` once item 1 above is wired.
7. Typed state for map edits if wanted instead of `faction_marks["map_edit|NNNN"]` (ti4-model `GameState`), and a field for non-faction tokens on tiles (Mirage, Thunder's Edge) for the Muaat hero purge.

## Rules questions

- Creuss Quantum Entanglement says game effects cannot prevent it; implemented as links independent of `wormholes_off`. If Enforced Travel Ban should still silence the Creuss flagship's delta wormhole (it is a wormhole of any player's), `extra_wormholes` goes through the normal switch (Travel Ban only silences ALPHA/BETA anyway, so delta is untouched).
- Corsair: "If the active system contains another player's non-fighter ships" is evaluated when the route is built (`PassSite.active`), for the position at that moment.
- Wormhole Generator destinations exclude a system already holding that token ("place or move" to the same place is a no-op); the Creuss Gate counts as a non-home system.
- Muaat hero Replace keeps the frontier token on the tile (card notes: frontier and command tokens stay).

## Commands and exact results

- `cargo test -p ti4-engine --lib -- movement:: transit:: tactical:: tokens:: seating:: factions::` -> `217 passed; 0 failed; 1 ignored` (the plan's `--lib movement:: transit::` form is not valid cargo syntax; the filters go after `--`).
- `cargo test -p ti4-engine -q --no-fail-fast` -> lib `1577 passed; 0 failed; 1 ignored`, other test binaries `1`, `4`, `5` passed, 0 failed.
- `cargo test -p ti4-content -q` -> `134 passed; 0 failed` (lib), `1 passed` (second binary), doctests ran.
- `cargo clippy -p ti4-engine --all-targets`: no warnings in movement.rs, transit.rs, tactical.rs, tokens.rs, seating.rs, hooks_movement.rs (one `slice::from_ref` lint in my new test was fixed and re-run clean). `cargo clippy -p ti4-content --all-targets`: nothing in galaxy.rs.
- `cargo check --workspace --all-targets`: no errors.
- `rustfmt --edition 2024` run per file on the files I edited. ti4-sim not run.
