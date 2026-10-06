# BF-empyrean (part A): abilities, technologies, hero, commander unlock, breakthrough

Branch `wp/base-factions`, Claude Sonnet subagent. Part B (flagship, mech, agent, Blood Pact, Dark
Pact) is `factions/empyrean_units.rs` and its own evidence. Card text is quoted at the top of
`crates/ti4-engine/src/factions/empyrean.rs`.

## Items

| Kind | Id | Status | Tests (`factions::empyrean::tests::`) |
|---|---|---|---|
| ability | `aetherpassage` | done | `aetherpassage_lets_the_activator_pass_empyrean_ships_for_that_activation_only`, `aetherpassage_is_optional_and_does_not_lift_a_third_players_blockade`, `aetherpassage_is_not_offered_without_an_empyrean_seat_or_for_their_own_activation` |
| ability | `voidborn` | done | `voidborn_lets_empyrean_ships_cross_and_leave_a_nebula_only_theirs` |
| ability | `dark_whispers` | done | `dark_whispers_gives_the_empyrean_both_notes_and_nobody_else` |
| technology | `as` (Aetherstream) | done | `aetherstream_adds_one_move_for_a_neighbour_activating_next_to_an_anomaly`, `aetherstream_also_serves_the_empyrean_and_needs_the_tech_an_anomaly_and_a_neighbour` |
| technology | `vw` (Voidwatch) | done | `voidwatch_takes_a_note_the_mover_chooses_when_they_move_in_beside_empyrean_units`, `voidwatch_may_take_the_movers_support_for_the_throne`, `voidwatch_needs_the_tech_an_empyrean_unit_and_a_move_and_is_silent_for_other_tables`, `voidwatch_by_the_empyrean_itself_takes_nothing` |
| leader | `empyreanhero` | done (coordinator adds the id to `empyrean_units::LEADERS`) | `the_hero_places_frontier_tokens_then_explores_those_under_its_ships`, `the_hero_is_the_owners_alone_and_needs_the_map` |
| leader | `empyreancommander` | unlock done (effect already in `borrowed_commanders_b.rs`; coordinator adds the id to `LEADERS`) | `the_commander_unlocks_only_beside_every_other_player` |
| breakthrough | `empyreanbt` (Void Tether) | done, see the token-count question | `void_tether_places_a_token_on_a_border_of_the_activated_system`, `void_tether_closes_the_border_to_other_players_but_not_to_the_empyrean_or_when_allowed`, `void_tether_moves_a_token_once_all_are_placed_and_needs_a_nearby_unit` |
| neutrality | all | done | `a_table_without_the_empyrean_is_untouched_by_every_hook` |

## How each is delivered

* **Aetherpassage**: optional `SYSTEM_ACTIVATED` after-ability for the Empyrean seat. Its effect
  writes `faction_marks["empyrean|passage"] = "<activation_seq>|<mover>|<empyrean>"`. New movement
  hook `MovementHooks::passable_owners`, read by `MovementRules::apply_faction_modules`, removes a
  system from `Board::enemy_ships` only when **every** foreign ship in it belongs to an allowing
  player, so a third player's ship in the same system still blocks (58.4b). Scoped to the
  activation by the sequence number.
* **Voidborn**: new hook `MovementHooks::ignores_nebulae` sets `MovementRules::nebulae_ignored`,
  which lifts 59.1, 59.1a and 59.2 for that mover only.
* **Dark Whispers**: `promissory::deal` already deals both corpus notes from `factions.json`, but
  the usual flows (`start_game_seeded` then `seating::deploy`, as in `ti4-sim` and
  `fixtures::seated_game`) deal before the faction is set. `seating::deploy` now calls
  `empyrean::deal_notes`, which inserts any of the faction's notes missing from the map (a no-op for
  every other faction; a note already lent out is left where it is).
* **Aetherstream**: optional `SYSTEM_ACTIVATED` after-ability; condition: owner has `as`, the
  activator is the owner or a neighbour (`transactions::are_neighbours`), the activator has a ship
  outside the active system, and the active system is adjacent (activator's `PlayerAdjacency`) to an
  anomaly (printed, or made a gravity rift by a module via the new
  `hooks_movement::extra_gravity_rifts`). Writes `empyrean|aetherstream = "<seq>|<player>"`; the
  module's `move_bonus` hook adds +1 to every ship of that player while the sequence matches.
* **Voidwatch**: mandatory `MOVEMENT_FINISHED` after-ability (`ships_moved > 0`, once per movement
  step, not per ship). The mover chooses a note from their hand (`promissory::available_notes`, plus
  their own Support for the Throne, which goes through `promissory::receive` so the VP follows as in
  a transaction). Faceup notes, an Alliance whose commander is locked and notes held by nobody are
  not in hand.
* **Hero**: `leader_action` (always true for the owner) and `use_leader`: places a frontier token in
  every `exploration::frontier_systems` system, then `explore_frontier` for each token in a system
  holding one of the owner's ships (sorted order, game dice and RNG). Shared code purges. With no
  map the hook returns `None` and nothing changes.
* **Commander**: `commander_unlocked` = at least one other player and `transactions::are_neighbours`
  with every one of them; `false` without a map.
* **Void Tether**: tokens are `faction_marks["empyrean|tether|<player>"]`, a public list of borders
  (`a|b;c|d`). Optional `SYSTEM_ACTIVATED` ability for the breakthrough holder when activating a
  system that contains or is next to one of their units or planets: the choice lists every
  placement on a hex border of the system (not a wormhole link) and, when a token is already on the
  board, every move of one onto such a border. New hook `MovementHooks::blocked_borders` is applied
  by `MovementRules` and `PlayerAdjacency::neighbours` for every viewer except the holder.
  "Unless you allow it": a second optional ability asks the Empyrean after another player activates
  a system whether to allow that player; the permission lasts for that activation.

## Decision sites to register (`tests/decision_delivery_inventory.rs`)

| Function | File | Choices |
|---|---|---|
| `voidwatch` (the effect closure) | `factions/empyrean.rs` | 1 (`Voidwatch: which promissory note to give`, asked of the mover) |
| `tether_place` (the effect closure) | `factions/empyrean.rs` | 1 (`Void Tether: which border`) |

The three other optional abilities (`aetherpassage`, `aetherstream`, `tether_allow`) are yes/no
windows asked by the resolver, with no `Choice` built here.

## Hook requests / changes in files outside the nominal list

None needed. Shared-file edits made inside my allowed files: `hooks_movement.rs` (three new
`MovementHooks` fields `ignores_nebulae`, `passable_owners`, `blocked_borders`, dispatch functions,
`extra_gravity_rifts`), `movement.rs` (two `MovementRules` fields, `PlayerAdjacency::blocked`),
`seating.rs` (one call in `deploy`).

Coordinator actions: add `"empyreanhero"` and `"empyreancommander"` to `empyrean_units::LEADERS`.

## Rules questions

1. **Void Tether token count.** The text says "1 of your Void Tether tokens" and never says how many
   exist. `empyrean::TETHER_TOKENS = 3` is an assumption; with all placed the activation moves one.
   Change the constant if the component set says otherwise.
2. **"Unless you allow it"** is modelled as a per-activation yes/no asked of the Empyrean after
   another player activates a system. It does not cover Void Tether's effect on non-movement
   adjacency questions that call `Galaxy::adjacent` directly (see below).
3. **Hero "does not contain any planets"** follows `exploration::frontier_systems`, where a system
   with only a space station counts as planetless (space station rule 14, same as setup).
4. **Voidwatch "from their hand"**: includes the mover's own notes and their own Support for the
   Throne; excludes faceup notes. Fires once per movement step, not once per ship.
5. **Aetherstream "neighbors"**: `transactions::are_neighbours` (units or controlled planets in or
   next to a system of the other player), not seating order.
6. **Aetherpassage** is offered only when both the Empyrean and the activator have a ship outside the
   active system; otherwise it could change nothing.

## Known gaps outside this package's files

* Many rules sites ask `galaxy.adjacent(..)` directly instead of `PlayerAdjacency`
  (`transactions.rs`, `combat.rs`, `action_cards.rs`, ...). Void Tether closes a border for movement
  and for everything that already uses `PlayerAdjacency` (Argent, Muaat, Naalu, Saar, Ghost,
  borrowed commanders); the direct callers still see the border. Moving them is a change in files
  this package does not own.
* The ordinary sim flow (`start_game_seeded` then set faction then `deploy`) never deals any
  faction's own promissory note to the seat (only the four generic notes) except the Empyrean's
  pair, which `deploy` now provides. That general gap predates this package and is left alone because
  fixing it would change every faction's game.

## Commands run

All with `env CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_INCREMENTAL=false
CARGO_PROFILE_TEST_CODEGEN_UNITS=16` and `-j1`; `rustfmt --edition 2024` run on `empyrean.rs`,
`hooks_movement.rs`, `movement.rs`, `seating.rs` only.

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -j1 empyrean` | 42 passed, 0 failed (18 in `factions::empyrean::tests`, the rest part B and `borrowed_commanders_b`) |
| `cargo test -p ti4-engine -j1 --no-fail-fast -- --test-threads=12` | lib 2384 passed, 0 failed, 1 ignored; `content_ids_resolve` 1 passed; doc-tests 5 passed; `decision_delivery_inventory` 3 passed, **1 failed** (expected: unreviewed sites below, for the coordinator) |
| `cargo test -p ti4-engine --lib -j1 print_faction_ledger -- --ignored --nocapture` | `empyrean  11/13 implemented`; missing `Leader empyreancommander`, `Leader empyreanhero` (the coordinator adds them to `empyrean_units::LEADERS`) |

Registry failure, exactly: unreviewed decision sites `empyrean.rs` `tether_place` (Choice 1,
AskObserved 1) and `voidwatch` (Choice 1, AskObserved 1).

Clippy was not run (memory); `-D warnings` cleanliness of the new code is unchecked.

## Review-fix notes

* Void Tether allow mark: the card gives no lifetime, so the mark lasts until the next activation
  (`activation_seq` changes). It therefore also affects adjacency questions asked between
  activations, not only the activation's own movement. The allow question is now asked only when a
  placed tether could matter to the activator: an end within 2 hexes of the active system or holding
  one of the activator's units (`tether_relevant`, a deliberate over-approximation).
* `TETHER_TOKENS = 3` is an assumption pending operator confirmation.
* Blood Pact's +4 is credited to whichever of the pair votes second. The tally is identical;
  per-player attribution differs only for effects keyed on a player's own votes.
* Dynamo and the Watcher use `Galaxy::adjacent` directly and ignore Void Tethers (known gap).
* Hero: a failed exploration (`explore_frontier` returning `None`) restores state, dice and rng and
  returns `Some(false)`, so the card is not purged. Aetherpassage clears an `enemy_ships` entry only
  when it holds at least one foreign ship and all foreign ships belong to allowing players.
