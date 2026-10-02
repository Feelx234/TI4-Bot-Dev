# BF-argent — Argent Flight (`factions/argent.rs`)

Branch `wp/base-factions`. Python not used. Card texts from `crates/ti4-content/content/*.json` (latest printing).

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | `raid_formation` | done (claimed) | `raid_formation_damages_one_sustain_ship_per_excess_hit`, `..._lets_the_producer_choose_between_types`, `..._needs_the_ability_and_an_undamaged_sustain_ship` |
| ability | `zeal` | partial, NOT claimed: extra votes done via `vote_bonus`; "always vote first" has no route | `zeal_casts_one_extra_vote_per_player_for_the_argent_seat_only` |
| tech | `swa2` | done (claimed); needs the tech AND the staged `argent_destroyer2` barrage | `each_nine_or_ten_destroys_an_infantry_in_the_space_area`, `alpha_ii_stops_at_the_last_infantry_and_ignores_low_dice_and_the_base_unit`, `alpha_ii_does_not_touch_infantry_on_planets_or_the_owners_own` |
| unit | `argent_destroyer`, `argent_destroyer2` | done (claimed): stats data-driven, ability = swa2 | `the_first_strike_wing_has_the_printed_statistics` |
| promissory | `ambuscade` | done (claimed) | `ambuscade_adds_one_die_to_a_chosen_unit_and_returns_to_argent`, `ambuscade_may_be_declined_and_needs_a_unit_ability_roll` |
| leader | `argentcommander` | done (claimed): unlock hook + extra die | `the_commander_unlocks_with_six_armed_units`, `the_commander_adds_a_die_only_once_unlocked_and_keeps_no_card_to_return` |
| regression | no Argent seat | done | `a_game_without_an_argent_seat_is_offered_no_argent_ability` |
| tech | `ah` | blocked (hook requests 3, 4) | |
| unit | `argent_flagship` | blocked (request 1) | |
| unit | `argent_mech` | partial, not claimed: `capacityUsed 0` in data covers "in a space area with your capacity ships"; "if being transported" needs request 2 | |
| leader | `argentagent` | blocked (request 5) | |
| leader | `argenthero` | not done: needs a multi-move relocation plan including cargo (request 6) | |
| breakthrough | `argentbt` | not done (request 7) | |

Design notes. Extra die (Ambuscade, commander) and Alpha II hang on `UNIT_ABILITY_ROLLED` and edit
`GameState::reroll_staging` (Scramble Frequency's route); the roll sites read the hits back from the
staging after the window (`combat.rs` ~3445, `game.rs` ~210, `invasion.rs` ~1866), so the added die
counts. Extra die is a WHEN ability, Alpha II AFTER, so a die added to the destroyer is seen by Alpha II.
Not reached: the synchronous `combat::anti_fighter_barrage` path emits no event (test/standalone only).
The Metali Void Armaments relic's barrage dice name no unit and are not offered an extra die.

## Hook requests

1. **Flagship** ("Other players cannot use SPACE CANNON against your ships in this system"): `combat.rs::space_cannon_offense`, the `may_fire` closure, plus the adjacent-reaching guns (`reaching_guns_by`): a `CombatHooks::space_cannon_barred: Option<fn(&GameState,&ContentStore,SourceSet, firing: &PlayerId, target: &PlayerId, system:&SystemId)->bool>`; any `true` stops that firing player's guns, where `target` is the active player whose ships are in `system` (needs flagship present in system).
2. **Mech transported**: `transit.rs::CargoWindow` counts each loaded unit as 1 slot (`is_complete`, `loaded.len() >= capacity`). Need `EconomyHooks`/`MovementHooks::free_cargo: fn(&GameState,&ContentStore,SourceSet,&Unit)->bool` so a mech does not use a slot.
3. **Aerie Hololattice, movement**: `MovementHooks::blocks_passage: fn(&GameState,&ContentStore,SourceSet, mover:&PlayerId, system:&SystemId)->bool` read in `MovementRules::path_from_ship` for intermediate systems (other players' structures; not the destination).
4. **Aerie Hololattice, production**: `production.rs::producers`/`capacity` need `EconomyHooks::extra_production: fn(&GameState,&ContentStore,&PlayerId,&SystemId,&PlanetId)->i64` (PRODUCTION 1 "as if it were a unit", for each planet with 1+ of the owner's structures).
5. **Agent** ("When a player produces ground forces in a system ... place those units on any planets they control in that system and adjacent systems"): no WHEN event before placement; `production.rs::placements` needs an extra-planets hook and a typed `GROUND_FORCES_BEING_PRODUCED` (player, system, count) window with an exhaust-and-widen answer.
6. **Zeal voting order**: `vote.rs::VoteWindow::new` needs a hook (`fn(&GameState,&PlayerId)->bool`, "votes first") placing that seat at the head of `final_order`.
7. **Hero/breakthrough**: a ground-force/fighter cargo parameter for `transit::relocate_ships` (hero text transports cargo "up to capacity"), and for the breakthrough a command-token placement API plus an `ACTION_COMPLETED`-after relocation among tokened systems.

## Rules questions

- Alpha II's "your opponent" is the combat opponent (`combat::opponent_with_ships`); with several opponents in the system only the first is chosen.
- Alpha II/Ambuscade order inside one roll: Ambuscade/commander (WHEN) first, so an added die can score a 9 or 10.
- Raid Formation does not damage via Metali-granted sustain unless the relic is held (implemented: it counts).

## Decision sites to register

| Function | Choices |
|---|---|
| `argent.rs::afb_excess` | 1 (`raid_formation_damage`, asked only with 2+ ship types) |
| `argent.rs::strike_wing_effect` | 1 (`swa2_infantry`, only with 2+ infantry types) |
| `argent.rs::extra_die_effect` | 1 (`extra_die_unit`, only with 2+ rolling units); the optional use itself is the resolver's |

`decision_delivery_inventory` currently fails; this file's three new `Choice` builders are among the
unregistered ones (other agents' sites also present).

## Commands run

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- factions::argent` | 13 passed, 0 failed |
| `cargo test -p ti4-engine --lib -- factions::` | 82 passed, 1 ignored |
| `cargo clippy -p ti4-engine --all-targets` | no warnings mentioning `argent.rs` |
| `rustfmt --edition 2024 crates/ti4-engine/src/factions/argent.rs` | applied |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1610 passed, 4 failed (not argent: `hooks_movement::empty_tables_are_neutral`, `winnu::trade_policy_...`, `production::the_mc_technology_...`, `strategy::bf00d_tests::a_waiver_...`); `decision_delivery_inventory` 1 failed (unregistered sites); other binaries ok |

Ledger: `argent 6/13 implemented`; missing: Ability zeal, Technology ah, Unit argent_flagship, Unit argent_mech, Leader argentagent, Leader argenthero, Breakthrough argentbt.
