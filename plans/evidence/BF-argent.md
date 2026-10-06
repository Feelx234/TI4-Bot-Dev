# BF-argent — Argent Flight (`factions/argent.rs`)

Branch `wp/base-factions`. Python not used. Card texts from `crates/ti4-content/content/*.json` (latest printing).
Second pass (2026-10-03) after the BF-F2..F5 routes landed.

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | `raid_formation` | done | `raid_formation_damages_one_sustain_ship_per_excess_hit`, `..._lets_the_producer_choose_between_types`, `..._needs_the_ability_and_an_undamaged_sustain_ship` |
| ability | `zeal` | done: `vote_bonus` (+1 vote per player) and `CardHooks::votes_first` | `zeal_casts_one_extra_vote_per_player_for_the_argent_seat_only`, `zeal_seats_the_argent_player_first_in_the_voting_order` |
| tech | `swa2` | done | `each_nine_or_ten_destroys_an_infantry_in_the_space_area`, `alpha_ii_stops_at_the_last_infantry_and_ignores_low_dice_and_the_base_unit`, `alpha_ii_does_not_touch_infantry_on_planets_or_the_owners_own` |
| tech | `ah` | done: `blocks_passage` + `extra_production_planet` | `aerie_hololattice_blocks_passage_and_adds_production_only_for_its_owner` |
| unit | `argent_destroyer`, `argent_destroyer2` | done (stats data-driven; ability = swa2) | `the_first_strike_wing_has_the_printed_statistics` |
| unit | `argent_flagship` | done: `space_cannon_barred` | `the_flagship_bars_other_players_space_cannon_against_its_owner_in_its_system` |
| unit | `argent_mech` | done: `free_cargo` + `capacityUsed 0` in data | `the_mech_rides_free_and_no_other_unit_does` |
| promissory | `ambuscade` | done | `ambuscade_adds_one_die_to_a_chosen_unit_and_returns_to_argent`, `ambuscade_may_be_declined_and_needs_a_unit_ability_roll` |
| leader | `argentcommander` | done: unlock hook + extra die | `the_commander_unlocks_with_six_armed_units`, `the_commander_adds_a_die_only_once_unlocked_and_keeps_no_card_to_return` |
| leader | `argenthero` | done: `leader_action` + `use_leader` over `relocate_ships_with_cargo` | `flock_migration_moves_a_ship_and_its_cargo_to_a_tokened_empty_system`, `flock_migration_declined_changes_nothing_and_is_not_offered_without_a_destination` |
| breakthrough | `argentbt` | done: SYSTEM_ACTIVATED (WHEN) token placement + ACTION_COMPLETED move, `faction_marks` bridge | `wing_transfer_places_tokens_then_moves_ships_among_the_tokened_systems`, `wing_transfer_needs_the_breakthrough_and_systems_with_only_the_players_units` |
| leader | `argentagent` | implemented; focused test run pending | cross-seat ProductionWindow placement, decline/exhaustion/eligibility/scope and cleanup cases |
| regression | no Argent seat | done | `a_game_without_an_argent_seat_is_offered_no_argent_ability` |

Design notes. Extra die (Ambuscade, commander) and Alpha II hang on `UNIT_ABILITY_ROLLED` and edit
`reroll_staging` (Scramble Frequency's route); the roll sites read hits back after the window. Extra die
is WHEN, Alpha II AFTER, so an added die can score a 9 or 10. The synchronous
`combat::anti_fighter_barrage` path emits no event (test/standalone only). Alpha II also requires the
`swa2` technology. Hero and breakthrough share `migrate`: one ship (hero: plus cargo, set down in
space) per step, destinations = systems holding the player's token and no foreign ships; declining the
first question changes nothing. Relocation events (`SHIPS_RELOCATED`) are not announced (no resolver
inside a timing hook).

## Hook requests

A. **Agent** ("When a player produces ground forces in a system ... place those units on any planets they control in that system and adjacent systems"): no WHEN event before ground-force placement; `production.rs::placements` needs an extra-planets hook (and a typed `GROUND_FORCES_BEING_PRODUCED` window carrying player, system, count).

A (third pass 2026-10-03). Brief premise corrected: `argentagent` is not a combat-start card. Exact text (leaders.json): window "When a player produces ground forces in a system:" / "You may exhaust this card: that player may place those units on any planets they control in that system and any adjacent systems." The production window now emits `GROUND_FORCES_BEING_PRODUCED` {player, system, count} from the pre-placement step through `Resolving::emit`, so the WHEN resolver can exhaust Argent's ready agent. Accepting records adjacent controlled planets for the current batch; `production_destinations` exposes those planets as the production window's placement options, and the mark is cleared after placement, window exit, or an answer error. The event is emitted for every ground-force production, including when no Argent seat can react.

Acceptance tests added in `production.rs`: `argent_agent_redirects_another_players_infantry_to_their_adjacent_planet`, `argent_agent_decline_and_exhaustion_leave_only_local_ground_force_placement`, `argent_agent_has_no_offer_for_another_players_adjacent_planet_or_for_a_ship`, and `argent_agent_temporary_destination_is_cleared_when_production_errors`. These drive `ProductionWindow`/`resolve_timed` with the faction timing resolver, including cross-seat ownership and the adjacent galaxy. They have not yet been run; the coordinating agent requested one shared low-memory build.

Fixture correction (2026-10-04): the producing seat is Sol, so its infantry offer names the faction unit `sol_infantry`, and the offered batch count is derived from current production capacity. Acceptance lookups now identify infantry by the unit catalogue's `base_type` and assert the full selected batch is redirected (or remains local on decline); the production-error test scripts the real offered option ID.

Readiness correction (2026-10-04): Argent's agent can be used only while `LeaderStatus::Readied`; `Unlocked` is for a leader that has not yet been readied. The timing condition now checks Readied, the effect records a destination only after `leaders::exhaust` succeeds, and acceptance coverage includes ready/decline, exhausted, and invalid-unlocked cases.

Adjacency review follow-up (2026-10-04): `agent_planets` now asks `movement::PlayerAdjacency` for the producing seat's neighbors rather than using raw `Galaxy::adjacent`. This preserves ordinary geometric neighbors and also includes rule-granted wormhole links for the recipient; the regression fixture checks a non-geometric alpha/beta destination and excludes controlled planets outside the recipient's legal adjacency.

## Rules questions

- Alpha II's "your opponent" is the combat opponent (`combat::opponent_with_ships`); several opponents: first only.
- Alpha II/Ambuscade order: Ambuscade/commander (WHEN) first.
- Wing Transfer: "contains only your units" is read as at least one unit and no one else's (space and planets); an empty system does not qualify. Moves "among" the active and adjacent tokened systems use no adjacency between the destinations; destination must hold no foreign ships. Cargo is not carried (the text names none).
- Flock Migration applies 95.5 (no cargo pick-up from a tokened system but the active one) via `relocate_ships_with_cargo`; on failure it retries without cargo (BF-F4 note).
- Raid Formation counts the Metali Void Shielding relic's granted sustain.

## Decision sites to register

| Function | Choices |
|---|---|
| `argent.rs::afb_excess` | 1 (`raid_formation_damage`, only with 2+ ship types) |
| `argent.rs::strike_wing_effect` | 1 (`swa2_infantry`, only with 2+ infantry types) |
| `argent.rs::extra_die_effect` | 1 (`extra_die_unit`, only with 2+ rolling units) |
| `argent.rs::ask_option_for` | 1 (builds the hero/breakthrough questions: ship, destination, cargo, token placement) |

## Commands run

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- factions::argent` | 21 passed, 0 failed |
| `cargo test -p ti4-engine --lib -- factions::` | 308 passed, 1 ignored |
| `cargo clippy -p ti4-engine --all-targets` | no warnings in `argent.rs` |
| `rustfmt --edition 2024 crates/ti4-engine/src/factions/argent.rs` | applied |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1878 passed, 1 ignored; `decision_delivery_inventory` 2 failed (unregistered sites, incl. the ones above); other binaries ok |

Ledger: `argent 12/13 implemented`; missing: Leader argentagent.

Third pass: `cargo test -p ti4-engine --lib print_faction_ledger -- --ignored --nocapture` -> `argent 12/13 implemented`, missing Leader argentagent (1 passed). No code change to argent.rs.
