# BF-00d-strategy: initiative, capture, mobile dock, dual-form unit, value swap

Implementer: Sonnet subagent (wave B). Branch `wp/base-factions`, nothing committed or staged.
Normative sources: card texts in `crates/ti4-content/content/*.json`; LRR cited where known.
Historical Python reference: not used.

Files touched: `ti4-model/src/state.rs` (one typed field), `ti4-content/src/units.rs` (two accessors),
`ti4-engine/src/{strategy,supply,production,fleet,phase}.rs`, `factions/hooks_strategy.rs`.
`strategy_cards.rs` is unchanged (nothing needed, see item 2).

## Items

| # | Item | Status | Added | Call point | Tests |
|---|---|---|---|---|---|
| 1 | Initiative override (Naalu `telepathic`, `gift`) | done | `GameState::initiative_overrides: BTreeMap<PlayerId,i32>` (`serde(default, skip_serializing_if empty)`, in `eq`); `initiative_order` replaces a player's printed number with it. `strategy::{set_initiative_override, clear_initiative_override, clear_initiative_overrides}`. Hook `StrategyHooks::strategy_phase_ended(&mut GameState)`. | Hook: `phase::advance_phase`, leaving the strategy phase, just before `initiative_order()` is read. Cleared in `phase::begin_next_round` (token lasts the round; Gift returns "at the end of the status phase"). Every reader of order goes through `GameState::initiative_order` (phase, status, objectives, choice, game.rs), so none needed editing. Per-card `card_initiative` (policy features, review panels) is unchanged. | `an_initiative_override_puts_a_player_first_until_cleared`, `two_zero_tokens_tie_by_seating`, `the_override_is_invisible_in_serialization_until_used`, `the_strategy_phase_ends_through_the_hook_and_the_round_clears_the_token` |
| 2a | `acq` Acquiescence | done as hook, **text is not a swap** | Printed text: "When the Winnu player resolves a strategic action: You do not have to spend or place a command token to resolve the secondary ability of that strategy card. Then, return this card to the Winnu player." Hooks `StrategyHooks::secondary_waivers(state, content, follower, primary, card_name) -> Vec<SecondaryWaiver>` and `secondary_waived(state, content, follower, primary, waiver_id)`. A waiver is offered as an extra option `follow|waived|<module index>|<id>` beside `no`/`yes`, only for a card whose secondary costs a token; a tokenless follower holding one is still offered the secondary. Taking it spends no token, calls `secondary_waived` (the module returns the note there), and resolves as `Followed`. | `strategy.rs` `StrategySecondaryWindow::{pending_choice,next_choice,take_choice}`; `STRATEGY_KIND`, prefix `WAIVED_SECONDARY_PREFIX`. Neutral: no hook means identical options. The existing pure `secondary_is_free` hook cannot express a "may", which is why the option route was built. | `a_waiver_lets_a_tokenless_follower_follow_for_free_and_is_paid_by_its_module`, `an_unoffered_waiver_is_refused_and_changes_nothing`, `hooks_strategy::tests::*` |
| 2b | Card swap API | done (no card in scope needs it) | `strategy::exchange_strategy_cards(state, a, card_a, b, card_b) -> bool`, atomic, rest of each holding kept, initiative order kept, each card keeps its own exhausted state. | New function; the model's `swap_strategy_card` (one-sided, does not check holding) is used inside. | `exchanging_cards_moves_each_with_its_own_exhaustion`, `a_bad_exchange_changes_nothing` |
| 2c | Resolve another card's primary (`winnuhero`) | done | `strategy_cards::primary(state, content, sources, galaxy, table, player, card)` is already public and does not read who holds the card (test resolves Leadership for a player who does not hold it). New `StrategySecondaryWindow::foreign(primary_player, card, followers)`: the "choose any number of other players, they may perform the secondary" window, usual token cost, exhausts no card on completion (a hero effect is not a held card). | The Winnu module calls `primary`, asks which followers, then drives `foreign` like a strategic-action window and calls `strategy_cards::secondary` per `Followed`. It must also purge the hero. | `a_primary_resolves_for_a_card_nobody_holds_and_followers_may_follow_it` |
| 3 | Capture (`mentakhero`, later Vuil'raith) | done | State already existed: `Player::captured_units: Vec<(owner, unit type)>` on the captor, and `supply::held` already counts it against the owner (31.4). Added in `supply.rs`: `captured_by`, `captured_of`, `capture_from_board(state, captor, system, planet, &Unit)`, `capture_from_reinforcements(state, content, sources, captor, owner, &UnitTypeId)` (needs a model in the box, uncapped fighters/infantry always), `return_captured(.., captor, owner, base_type) -> Option<UnitTypeId>`, `release_all_captured(state, captor)`. All atomic (false/None, state untouched). Announced variants `capture_from_board_announced` / `return_captured_announced` emit **new** events `UNIT_CAPTURED` and `CAPTURED_UNIT_RETURNED` (payload `player` = captor, `owner`, `unit`, `source`). | Callers, not hooks. The caller removes a unit from a destruction and captures it instead. | `a_captured_ship_leaves_the_board_but_not_its_owners_plastic`, `a_returned_model_goes_back_to_its_owners_reinforcements`, `a_ground_force_is_captured_off_its_planet`, `an_impossible_capture_changes_nothing`, `capturing_from_reinforcements_needs_a_model_in_the_box`, `a_captor_leaving_returns_everything`, `announced_capture_and_return_report_success_and_stay_quiet_otherwise` |
| 4 | Mobile space dock (Saar `saar_spacedock`) | done where the data allows; movement/retreat is a Request | Most already worked: `production::producers`/`capacity` read space-area units, so a Floating Factory gives its flat PRODUCTION 5 with no planet; capacity 4 already counts in `fleet::standing`; same plastic as a space dock (`base_type spacedock`); placement of infantry in space vs planet already tested (`obs008c2b`). Added: content accessors `UnitType::{is_space_only_structure, moves_as_ship}` (from `isSpaceOnly`, `isStructure`, `moveValue`; true only for the Saar docks); `production::mobile_docks`; `production::destroy_blockaded_mobile_docks(state, content, sources, system) -> Vec<Unit>` ("If this unit is blockaded, it is destroyed": enemy ship present and owner has none); `sling_relay_candidates` now counts a space-area dock as a dock. Not a ship for fleet supply, capacity use or combat (the corpus does not flag it `isShip`). | Blockade destruction and movement need other files: Requests 3 and 4. | `a_floating_factory_in_the_space_area_is_a_producer_and_counts_for_capacity`, `it_is_still_the_same_plastic_as_a_space_dock`, `an_enemy_ship_with_none_of_its_own_destroys_a_floating_factory`, `sling_relay_sees_a_floating_factory_as_a_space_dock` |
| 5 | Dual-form mech (`naaz_mech`/`naaz_mech_space`) | done; flip timing is a Request | `fleet::other_form(types, id)` (twin by `_space` suffix, same base type and faction; only the Naaz mech pair has one, tested as a census) and `fleet::flip_form(state, content, sources, player, system, planet, from) -> Option<UnitTypeId>`: in place, keeps `sustained_damage`/`galvanized`, one plastic pool (supply cannot change), refuses a ship form on a planet, atomic. Reads checked after a flip: `combat::ships_of`/`combatants` now include the unit (it fights the space combat), `fleet::standing` consumption and fleet charge unchanged. `naaz_voltron` has no twin (one record that is both ship and ground force) so `other_form` is `None`. | Caller decides when (start of space combat in the active system, end of the space battle, landing). Nothing flips by itself. | `only_the_naaz_mech_has_another_form`, `flipping_in_the_space_area_makes_the_mech_a_ship_without_touching_the_supply`, `both_forms_are_one_model_for_the_cap`, `a_ship_form_is_never_made_on_a_planet_and_the_ground_form_may_be`, `a_flip_that_cannot_happen_changes_nothing` |
| 6 | `htp` value swap (Winnu) | done | The typed field `production_value_swapped_planet` already existed, unused. Added `production::{begin_value_swap, end_value_swap, swapped_planet, swapped_value}`. A swap is scoped to one use: it records `production_seq` in `faction_marks["production:value_swap_seq"]` and `swapped_planet` returns `None` once the sequence has moved on; `end_value_swap` runs when a production window reaches `Done`, and at the end of `resolve_timed` and `produce_by_ability`. `swapped_value(state, content, sources, planet, kind)` returns the other kind's value (`planet_value_now`, attachments included) only for the swapped planet. | Module: in its `PRODUCTION_USED` timing ability (after `production_seq` is advanced, before `window.refresh`: `game.rs` order checked at lines 374 to 415) exhaust the card, choose the planet, call `begin_value_swap`; its `EconomyHooks::planet_spend_value` returns `swapped_value(..).unwrap_or(value)`. Read by `capacity`, `barred_capacity` and `payment_faces`. | `a_swap_reads_the_other_value_for_the_swapped_planet_only_and_only_during_its_use`, `the_swap_reaches_a_docks_production_through_the_planet_value_hook`, `a_finished_production_use_ends_the_swap` |

Neutrality: with no modules, `hooks_strategy` dispatch does nothing; no new event is emitted by existing code;
the model field is skipped when empty (test proves the serialized state has no `initiative_overrides` key and that a
pre-field snapshot loads). `faction_marks` and `production_value_swapped_planet` are untouched unless `begin_value_swap`
runs. Two shared-path edits are no-ops without such modules: `sling_relay_candidates` also accepts a
space-only dock, and the production window's `Done` path calls `end_value_swap`. Not run: `ti4-sim` (coordinator).

## Requests (files outside this package)

1. `game.rs` `step_phase`: Gift of Prescience is a promissory note played "at the end of the Strategy Phase" by a
   holder; the timing needs a typed event before `advance_phase` leaves the strategy phase (e.g.
   `STRATEGY_PHASE_ENDED`, new name) so the note can open a window and call `strategy::set_initiative_override`
   for the holder. `phase::advance_phase` does not clear overrides, so one set in that window survives into the hook.
   Telepathic itself needs nothing in `game.rs`.
2. `game.rs` tactical production: a window that is `Stage::Done` at open (no capacity) never calls
   `resolve`, so a swap begun for it is only cleared by the seq guard. Call `production::end_value_swap(state)` where
   the tactical-action production use ends. `resolve_timed` (Warfare secondary, Construction) does not emit
   `PRODUCTION_USED` unless the Harrugh hero is ready, so Hegemonic Trade Policy cannot act in those uses until it does.
3. `tactical.rs` (line 285, `if !kind.is_ship()`) and the retreat code: use `UnitType::moves_as_ship()` so a Floating
   Factory (move 1) is movable and can retreat; its capacity then travels with it. After each movement into a system
   and at the end of a space combat call `production::destroy_blockaded_mobile_docks` and announce the destruction.
4. `combat.rs` / `tactical.rs` / `invasion.rs`: call `fleet::flip_form` at "start of a space combat" (ground form in the
   active system's space area becomes the ship form), "end of a space battle in the active system" (back), and when a
   space-form mech is committed to land (to ground form before it reaches the planet).
5. Player elimination (wherever the engine removes a player): call `supply::release_all_captured`.
6. `decision_delivery_inventory`: `cargo test -p ti4-engine --no-fail-fast` fails only that one test, for
   `sardakk.rs` (`exotrireme`, `supremacy`) and `yin.rs` (`ask_one`) sites other agents added; none are from this package.

## Rules questions (not guessed silently)

* Capture: the PoK "Capture" rule text is not in `rules.json`. Implemented from the brief: captured units sit on the
  captor's sheet, are out of the owner's reinforcements (31.4), and the captor can return them. Open: may a player
  capture their own unit (refused here); does a captured unit's owner get them back when the captor is eliminated
  (`release_all_captured` exists, nothing calls it); can a capture be refused for a unit with no plastic left (not for
  `capture_from_board`, a model on the board always exists).
* Naaz flip: per unit here. The text says "flip this card", which may flip every Eidolon of the player at once.
* Z-Grav Eidolon in space: still charged to ship capacity (ground force) and not to the fleet pool (37.1a carried
  units). If "also a ship" means it counts toward fleet supply too, `fleet::counts_against_supply` needs a
  dual-form exception. `naaz_voltron` (TE) is both and moves itself; unchanged.
* Floating Factory: not a ship, so not counted for fleet supply; may it be assigned hits, retreat as a fleet member,
  or be carried? Blockade destruction uses "enemy ship present, none of the owner's"; the printed text gives no timing.
* Winnu hero: do the chosen followers pay a strategy token for the secondary (implemented as yes, the usual cost).
* Hegemonic Trade Policy "during that use of Production": whether ability-driven production (flagships, Warfare
  secondary through `resolve_timed`) is "a use" is the open question already recorded in BF-00b-economy.

## Commands run and exact results (all with `LIBTORCH=D:/Projects/ti4-engine-rs/out/libtorch-2.9.1-cpu`)

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- strategy:: strategy_cards:: supply:: production:: fleet:: phase:: factions::` | 231 passed, 0 failed, 1 ignored (the ledger printer) |
| `cargo test -p ti4-engine --lib -- capture_tests dual_form_tests mobile_dock_and_swap_tests bf00d_tests hooks_strategy` | 31 passed, 0 failed (the 28 new tests plus 3 `hooks_strategy`) |
| `cargo test -p ti4-model -q` | 81 passed, 0 failed |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1556 passed, 1 ignored; all other binaries ok **except** `decision_delivery_inventory` 3 passed, 1 failed (Request 6, other agents' sites) |
| `cargo clippy -p ti4-engine -p ti4-model -p ti4-content --all-targets` | no warning in any file of this package except `ti4-model/src/state.rs:961` (`struct_excessive_bools`, pre-existing on `GameState`) |
| `cargo check --workspace` | Finished, no errors |

Note: `cargo test ... --lib strategy:: strategy_cards::` as written in the brief is rejected by cargo (one filter before `--`);
filters were passed after `--`. `LIBTORCH=out/...` (relative) fails in the `torch-sys` build script; the absolute path works.
