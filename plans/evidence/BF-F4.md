# BF-F4: movement and transport routes

Branch `wp/base-factions`. Files edited: `movement.rs`, `tactical.rs`, `transit.rs`, `factions/hooks_movement.rs`. No faction file, `game.rs`, or the decision-inventory test was touched. All new hooks are neutral while no module installs them.

## Items

| # | Item | Status |
|---|---|---|
| 1 (J) | Supernova split | done |
| 2 (L) | Mobile space docks move | movement/carrying done; blockade-destroy and retreat call sites are requests (below) |
| 3 | `free_cargo` | done |
| 4 | `blocks_passage` | done |
| 5 | Wormholes crossed per move | skipped: coordinator did it (`SHIP_MOVED.wormholes`, `MOVEMENT_FINISHED`) |
| 6 | Relocation with cargo | done |

### 1. Supernova split
- New `MovementHooks::may_pass_through_supernova: Option<fn(&GameState,&ContentStore,SourceSet,&PlayerId)->bool>`. Gashlai Physiology ("move through") belongs here.
- `may_enter_supernova` is unchanged in meaning and now documented as "end a step in" (Magmus Reactor); `muaat.rs` needs no signature change.
- `MovementRules::supernovae_pass_through` (pub bool) is set in `apply_faction_modules`. `can_pass_through_ship` uses a new private `enterable(id, passing=true)`. `can_enter` (the ending step) uses `passing=false`.
- `path_from_ship` now applies `can_enter` only to the active-system step. Intermediate steps are judged by `can_pass_through_ship`, whose first test is the passing form of the same check, so nothing else changes.
- Reactor (`supernovae_open`) still opens both.
- Request (muaat.rs): register `may_pass_through_supernova: Some(..)` returning `has gashlai_physiology`, and decide whether the Reactor hook should stay `mr` only. Then claim `gashlai_physiology`.

### 2. Mobile docks
- `tactical::movable_into` uses `kind.moves_as_ship()` instead of `is_ship()`. A Floating Factory is offered with its move value, `Movable.capacity` 4/5, and the same route, rift and boost rules.
- `CargoWindow::for_ship` already reads capacity from the corpus, so it carries cargo.
- A dock is not a ship, so it does not block passage (`Board`), and it does not count for fleet supply or capacity use.
- Not mine, so requests for `game.rs` and `combat.rs`:
  - **game.rs**: where `MOVEMENT_FINISHED` is emitted (the end of the movement step), add
    `let _gone = crate::production::destroy_blockaded_mobile_docks(&mut self.state, self.content, self.sources, &active);`
    for the active system. Announce a typed destruction event if wanted. Only the active system can become newly blockaded by movement.
  - **game.rs / combat.rs**: after each space combat ends (at the `SPACE_COMBAT_ENDED` emit), call the same function for the combat system.
  - **combat.rs**: `eligible_retreats` / `retreat_to` must also move own units for which `UnitType::moves_as_ship()` is true and `is_ship()` is false ("can retreat as if it were a ship"). Today only ships retreat.

### 3. Free cargo
- `MovementHooks::free_cargo: Option<fn(&GameState,&ContentStore,SourceSet,&Unit)->bool>`. Any module's `true` makes a unit ride without using a slot.
- Read in `transit::CargoWindow::for_ship`, once per candidate, only when some module installs the hook.
- `slots_left()` counts only the paying units.
- `free()` offers any unloaded unit while a slot is left, or only the free-riding units once the hold is full.
- `is_complete()` is false while a free-riding unit remains.
- The option preview delta is `-0` for a free unit.
- Neutral behaviour is identical to the old `loaded.len()` arithmetic.
- Argent request: `free_cargo: Some(|_, _, _, unit| unit is argent_mech && owner holds the faction)`. The mech's "in a space area with your capacity ships" half is already in the data (`capacityUsed 0`).

### 4. Passage block
- `MovementHooks::blocks_passage: Option<fn(&GameState,&ContentStore,SourceSet,mover:&PlayerId,system:&SystemId)->bool>`. Any `true` bars `mover` from moving through that system.
- `MovementRules::apply_faction_modules` (needs a mover) evaluates it for each system on `state.board`, only when installed. A `true` goes into the existing `barred_transit` set, so the system can still be the active system.
- The hook itself must check that `mover` is not the owner and that the owner has a structure (space area or planet) there.
- `Board`-level enemy ships are unaffected.
- Argent request: register it for `ah`. The PRODUCTION 1 half still needs `EconomyHooks::extra_production`.

### 6. Relocation with cargo
- New `transit::relocate_ships_with_cargo(state, content, sources, galaxy, &Relocation, cargo: &[Cargo]) -> Result<Relocated, RelocateError>`. `relocate_ships` now delegates with `&[]`, so existing callers are unchanged.
- Checks:
  - Each carried unit is the player's own capacity-using unit (ground force or fighter), standing in space or on a planet of `from`, with copies counted after the ships are lifted.
  - `Cargo.system == from`.
  - 95.5 is applied: refused if `from` holds the player's command token and is not the active system.
  - Paying cargo (units that are not `free_cargo`) must not exceed the summed capacity of the relocated ships.
- All checks run before the first mutation. Cargo lands in the destination's space area, never on a planet.
- New `RelocateError` variants: `NotCargo(String)`, `CargoNotThere`, `OverCapacity`, `CargoUnderCommandToken(SystemId)`.
- New `Relocated.cargo: Vec<String>`. The `cargo` key appears in the `SHIPS_RELOCATED` payload only when non-empty.
- Build `cargo` from `transit::loadable(state, content, sources, player, from)`.
- Rules question: the Argent hero reads "from any systems", which may well exempt it from 95.5. I applied 95.5 conservatively. If the hero should pick up from systems holding its command token, drop that check or add a flag.

## New decision sites
None. `CargoWindow::pending_choice` is the existing site (count unchanged).

## Commands and results
| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- movement:: tactical:: transit:: tokens:: factions::` | 395 passed, 0 failed, 1 ignored |
| `cargo test -p ti4-engine --lib -- tactical::` (after adding the dock tests) | 29 passed |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1845 passed, 1 ignored; every other binary passes except `decision_delivery_inventory` (2 of 4 fail) |
| `cargo clippy -p ti4-engine --all-targets` | no warning in my four files |

The `decision_delivery_inventory` mismatch is not from my files. Diffing the observed and registered maps shows only `leaders.rs dispatch_leader` / `use_leader` and `production.rs produce_by_ability_capped` / `produce_by_ability`, which are another session's renames.

Tests added:
- movement: `passing_through_a_supernova_is_not_ending_a_move_in_one`, `a_module_can_bar_passage_through_a_system_for_other_players_only`.
- hooks_movement: neutral asserts added to `empty_tables_are_neutral`.
- tactical: `a_floating_factory_is_offered_as_a_mover_and_an_ordinary_dock_is_not`, `a_floating_factory_out_of_range_is_not_offered`.
- transit: `a_unit_that_rides_free_still_boards_a_full_hold`, `a_relocation_carries_cargo_up_to_capacity_and_reports_it`, `cargo_beyond_capacity_or_not_cargo_refuses_and_changes_nothing`, `units_that_ride_free_do_not_use_capacity_when_relocated`.

Not run: `ti4-sim`, `ti4-replayer`, workspace check.
