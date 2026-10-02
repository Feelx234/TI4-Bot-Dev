# BF-sardakk: Sardakk N'orr (`factions/sardakk.rs`)

Implementer: Sonnet subagent. Card text: `crates/ti4-content/content/*.json`, DEFAULT sources. Tests are the
inline `mod tests` in `sardakk.rs` (22).

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | `unrelenting` | done (shared code, proven through it) | `unrelenting_adds_one_to_every_roll_through_the_shared_path` |
| tech/unit | `exo2`, `sardakk_dreadnought2` | done: stats data-driven, Direct Hit immunity (printed flag via `direct_hittable_at`), destroy ability on `SPACE_COMBAT_ROUND_ENDED` | `the_exotrireme_upgrade_applies_its_data_driven_statistics`, `exotrireme_destroys_itself_and_up_to_two_chosen_ships`, `exotrireme_may_stop_after_one_ship_or_decline_entirely`, `exotrireme_is_not_offered_without_the_card_the_ship_or_a_target` |
| unit | `sardakk_dreadnought` | done (statistics) | same stats test |
| tech | `vpw` | done (`ground_rolls_extra_hits`) | `particle_weave_adds_a_hit_only_when_the_opponent_produced_one` |
| unit | `sardakk_flagship` | done (`unit_roll_modifier`, space) | `the_flagship_helps_only_its_owners_other_ships_in_its_system` |
| unit | `sardakk_mech` | done (`GROUND_FORCE_SUSTAINED` + `assign_ground_hits_in_timing`) | `the_mech_produces_a_hit_after_it_sustains_in_ground_combat`, `the_mech_hit_needs_its_own_sustain_during_ground_combat`, `the_mech_hit_is_wasted_when_nothing_of_the_opponent_is_left` |
| promissory | `tekklar` | done (`GROUND_COMBAT_STARTED`, combat-scoped mark in `faction_marks`, read by `unit_roll_modifier`) | `tekklar_legion_helps_the_holder_hinders_the_norr_player_and_goes_home`, `..._may_be_declined_and_is_not_the_owners_to_play`, `..._does_not_hinder_a_player_who_is_not_the_norr_player` |
| leader | `sardakkagent` | done (`ACTION_COMPLETED` + activation mark) | `the_agent_lets_the_active_player_place_two_infantry_after_a_tactical_action`, `the_agent_is_the_actives_choice_to_use_and_the_owner_to_exhaust`, `the_agent_needs_a_tactical_action_a_readied_card_and_a_planet_to_place_on` |
| breakthrough | `sardakkbt` | done (space and ground "win") | `supremacy_after_a_won_combat_gains_a_command_token`, `..._researches_a_unit_upgrade`, `supremacy_needs_the_breakthrough_and_a_win` |
| leader | `sardakkcommander` | **partial, not claimed**: unlock (5 planets in non-home systems) and "1 ground force from each planet in the active system" done; adjacent-systems half blocked | `the_commander_unlocks_with_five_planets_outside_home_systems`, `the_commander_offers_one_ground_force_from_each_planet_of_the_active_system` |
| leader | `sardakkhero` | **blocked, not claimed** | none |

Regression: `a_game_without_the_card_is_never_asked_anything_by_this_module` (no Sardakk seat: no Sardakk
ability is ever offered). Every bookkeeping ability has a condition that is false unless a Sardakk seat
holds the card/mark (`is_norr_seat`, `agent_ready`).

## Hook requests

1. **`hooks_ground::GroundHooks::commit_candidates`** (`hooks_ground.rs`, `invasion.rs::commit_pool`): add
   `galaxy: Option<&Galaxy>` to the signature. The commander needs adjacency ("each planet in adjacent
   systems that do not contain 1 of your command tokens"; command tokens are `SystemState::command_tokens`),
   and `GameState` carries no map. Until then only the active system's planets are offered.
2. **Sardakk hero** (`game.rs`): an `Aftermath` entry that opens `InvasionWindow::at_commit_step` after the
   ships are moved ("After you move ships into the active system"), offered when an unlocked hero's
   `leader_action` says so; after `GROUND_COMMITMENT_FINISHED` the faction purges the card and returns the
   player's ships in the active system to reinforcements. Already recorded in `BF-00c-ground.md` item 9.
3. (nice to have) `ACTION_COMPLETED` payload `kind` ("tactical"/"strategic"/"component") and `system`, in
   `game.rs::emit_action_completed`. The agent works around it with activation marks (below).

## Decision sites to register (`tests/decision_delivery_inventory.rs`)

From the inventory test's observed set (all in `sardakk.rs`):

| function | `Choice` | `AskObserved` |
|---|---|---|
| `exotrireme` | 2 | 2 |
| `supremacy` | 1 | 1 |

(`place_units_choosing`, `gain_tokens` are existing registered sites.) The "may" of each optional ability
is the resolver's own use/decline question; this module asks nothing twice.

## Rules questions / known limits

- **Agent window.** `ACTION_COMPLETED` fires after `active_system` is cleared and does not say tactical or
  strategic, so the activating system is remembered in `GameState::faction_marks`
  (`sardakk:agent:tactical:<player>`) while a Sardakk seat holds T'ro readied, cleared by `TURN_BEGAN`,
  `STRATEGIC_ACTION_BEGAN` and the use itself. Edge: an *additional* action (Master Plan, Minister of War)
  that is a component action after a tactical action in the same turn could read the leftover mark. A
  tactical action started by a strategy card's free tactical action is not covered.
- **Exotrireme II** is offered only when another ship is in the system (a use that destroys only itself
  is not offered). "Up to 2" otherwise allows 1; it destroys any player's ships including the owner's.
- **Mech** (see `BF-00c-ground.md`): the hit is produced after the round's simultaneous removals. The
  produced hit carries cause `ground_combat`, so an opposing N'orr mech that sustains it answers in turn
  (bounded: each mech sustains once).
- **Tekklar Legion**: "invasion combat" read as the ground combat on a planet (`GROUND_COMBAT_STARTED`).
  The bonus is a mark cleared by `GROUND_COMBAT_ENDED` for that planet or the next `TURN_BEGAN`.
- **N'orr Supremacy**: research follows normal prerequisites (`technology::researchable` filtered to unit
  upgrades); "win a combat" = the `winner` of `SPACE_COMBAT_ENDED` / `GROUND_COMBAT_ENDED`.
- The commander unlock counts planets (not systems) in systems that hold no faction's home planet.

## Commands run (final)

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib factions::sardakk` | 22 passed, 0 failed |
| `cargo test -p ti4-engine --lib factions::` | 69 passed, 0 failed, 1 ignored |
| `cargo clippy -p ti4-engine --all-targets` | 0 warnings in `sardakk.rs` (others' files: warnings unchanged) |
| `rustfmt --edition 2024 crates/ti4-engine/src/factions/sardakk.rs` | clean |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib: 1562 passed, 1 failed (`transit::relocation_tests::ships_move_between_adjacent_systems_and_are_reported`, movement area, not Sardakk); `decision_delivery_inventory` 1 failed (registry out of date: includes the four `sardakk.rs` sites above and other sessions' new sites); all other binaries ok |

## Ledger

```
sardakk   10/12 implemented
    Leader sardakkcommander
    Leader sardakkhero
```
