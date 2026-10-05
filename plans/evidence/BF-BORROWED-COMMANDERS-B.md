# BF-BORROWED-COMMANDERS-B: acquired Empyrean and Firmament commanders

Source: `crates/ti4-content/content/leaders.json`. Code: `crates/ti4-engine/src/factions/borrowed_commanders_b.rs`.
Not compiled or tested by the implementer (shared build directory was owned by another job); the coordinator must build and run the tests.

## Items

| Commander | Text | Route | Status |
|---|---|---|---|
| `empyreancommander` | "After another player moves ships into a system that contains 1 of your command tokens: You may return that token to your reinforcements." | Timing ability `leader:<owner>:empyreancommander:SHIP_MOVED:after`, optional, stateful condition. Payload `player`/`system` (emitted by `Game::note_arrival`, game.rs ~3010). Gate: mover != seat, `has_commander_ability(seat, "empyreancommander")`, seat token in `system`. Effect removes the seat's token from `board[system].command_tokens` (same as Arborec Psychospore returning a token). | Written, uncompiled |
| `firmamentcommander` | "At any time: You can treat planets in systems that contain your ships as if they were controlled by you for the purpose of scoring secret objectives." | Pure helper `firmament_planets(state, content, sources, player) -> Vec<PlanetId>`; empty unless `has_commander_ability`. Needs one wiring edit in secrets.rs (below). | Helper written; blocked on wiring |

## Wiring needed outside my file

1. `factions/mod.rs`: add `mod borrowed_commanders_b;` and register `borrowed_commanders_b::timing_abilities(owner_name, seat)` for every seat, next to `borrowed_commanders` (~line 576).
2. `crates/ti4-engine/src/secrets.rs`, `Position::controlled()` (line ~236), the single seam every planet-counting secret reads. After the `held` loop over `self.state.controlled_planets`, before the `imagined` loop, add:

```rust
for planet in crate::factions::borrowed_commanders_b::firmament_planets(
    self.state, self.content, self.sources, self.player,
) {
    if !held.contains(&planet) {
        held.push(planet);
    }
}
```

   This is secrets-only (public objectives use `objectives::Position`, untouched). It covers `planets_of_trait`, `combined` resources/influence and `legendary_planets_count`.
3. Optional (judgement call, card says "controlled by you" for scoring secrets): `secrets.rs` `mecatol_ships_while_controlling_count` (line ~399) uses `board.controls_a_planet(player)` directly and `shared_planet_systems_count` (~469) reads `planet_control` directly; neither goes through `controlled()`. If the operator wants Firmament to apply there, `mecatol_ships_while_controlling_count` should also pass when `firmament_planets(...)` contains any planet in the Mecatol system (ships there are the very condition, so Firmament makes it trivially true for Mecatol with ships). Not implemented; flag as rules question.
4. Imagined-option counterfactual (`Position::imagined`) is unaffected: planets are only added, deduplicated.

## Decision sites to register

`tests/decision_delivery_inventory.rs`: none new beyond the standard optional ability prompt (ability id `leader:<owner>:empyreancommander:SHIP_MOVED:after`, 1 optional ask, same shape as the Titans commander).

## Tests (in the file)

- `empyrean_returns_the_token_when_another_player_moves_in`
- `empyrean_is_optional` (decline leaves state identical)
- `empyrean_ignores_own_moves_missing_tokens_and_the_unentitled` (own move, no token, no ability: state identical)
- `empyrean_leaves_other_players_tokens_alone`
- `firmament_counts_every_planet_where_the_holder_has_ships`
- `firmament_needs_the_ability_and_a_ship_of_the_holder` (no ability, other player's ships, ground forces only)

The secrets-level behaviour needs a test after wiring in `secrets.rs`: grant `firmamentcommander`, put a ship in a system with N trait planets, assert `counting_progress` rises; without the grant, no change.

## Risks to check when compiling

- Tests use `fixtures::a_placed_planet()` system; negatives clear that system's space units first. If a home system holds a seated player's ships the clear is harmless.
- `Planet::id()` and `planets_in` signatures taken from ti4-content galaxy.rs.
- Empyrean sees `SHIP_MOVED` with `system` only when `active_system` is set (always true during tactical moves).

## Rules question

Does the Firmament commander also apply to board-shape secrets that read control directly (item 3)? Left as is.
