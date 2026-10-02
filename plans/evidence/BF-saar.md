# BF-saar: The Clan of Saar

Branch `wp/base-factions`, uncommitted. Code: `crates/ti4-engine/src/factions/saar.rs`. Card text: `crates/ti4-content/content/*.json` at DEFAULT.

## Items

| Kind | Id | Status | Tests |
|---|---|---|---|
| ability | `scavenge` | done (timing `PLANET_CONTROL_GAINED` after; fires from the invasion emit site only, see Hook requests 4) | `scavenge_pays_a_trade_good_for_each_planet_gained_and_only_to_the_gainer` |
| unit | `saar_mech` Scavenger Zeta DEPLOY | done (same event; optional, 1 TG, mech placed on that planet, box-checked) | `scavenger_zeta_costs_a_trade_good_and_places_a_mech_or_is_declined` |
| unit | `saar_flagship` | done: statistics only (corpus has no ability text); AFB comes from data | `the_stats_of_the_flagship_are_data_driven` (checks only ship/flagship; AFB 6x4 is in `units.json`, not asserted by a combat test) |
| promissory | `ragh` | done (holder, `UNITS_COMMITTED` after; Saar player picks the destination planet; note returned) | `ragh_s_call_moves_the_saar_forces_to_a_planet_they_control_and_returns_the_note`, `ragh_s_call_is_not_offered_without_the_note_or_without_saar_forces_there` |
| leader | `saarhero` Gurno Aggero | done (leader_action/use_leader; needs galaxy in `TimingContext`) | `the_hero_destroys_other_players_infantry_and_fighters_next_to_a_dock_only` |
| leader | `saarcommander` | partial: unlock (3 space docks) done and tested; effect NOT implemented, so not claimed | `the_commander_unlocks_at_three_space_docks` |
| technology | `cm` Chaos Mapping | partial: start-of-turn production implemented (offer gate tested, decline tested; a full production run is not tested); activation bar NOT implemented; not claimed | `chaos_mapping_production_needs_the_technology_the_action_phase_and_a_producer` |
| ability | `nomadic` | blocked (hook request 1) | |
| technology/unit | `ffac2`, `saar_spacedock`, `saar_spacedock2` | blocked: production and capacity already work in the space area (BF-00d), but moving/retreating as a ship and blockade destruction are not wired in `tactical.rs`/combat; not claimed | |
| leader | `saaragent` | blocked (hook request 3) | |
| breakthrough | `saarbt` | not started (see request 5) | |
| regression | no Saar seat | `a_game_without_saar_is_offered_no_saar_ability` (empty script fails on any choice; board and TG unchanged) | |

## Hook requests

1. `objectives.rs::controls_home_system` (61.16, the gate in `scoreable_on`): Nomadic needs it to be true for a player with `nomadic`. Request: `Hooks::ignores_home_control(state, content, player) -> bool` (any module), or have the function return true when `faction_abilities::has(.., "nomadic")`.
2. `tactical.rs::activatable`: Chaos Mapping "Other players cannot activate asteroid fields that contain 1 or more of your ships". Needs a hook `cannot_activate(state, content, activator, system) -> bool` (any) consulted per system; the activation choice is generated from this list, so no late rejection.
3. Captain's Genome (`saaragent`): "When a player activates a system: exhaust to increase the move value of 1 of that player's ships to match the highest move value of a ship on the board." Needs `movement::move_bonus` (exists) plus `game.rs::begin_one_move` using `effective_move_value_for_ship` with the ship index (BF-00e Request 1, still unwired), so a chosen ship's bonus is honoured; the module would store the chosen index in `faction_marks`.
4. `PLANET_CONTROL_GAINED` is emitted only from `invasion.rs` (landing). Other gains of control (Reparations/Infiltrate-style effects, relic or action-card captures, `legendary`, `thunders_edge` placement, Integrated Economy paths) call `faction_abilities::control_gained` or nothing and emit no window, so Scavenge and the DEPLOY do not fire there. Request: emit the typed event at every control change, or pass the existing `Hooks::control_gained` through all of them (that hook is state-only and cannot ask a choice, so the typed event is preferable).
5. Mobile dock (BF-00d Request 3): `tactical.rs:285` use `UnitType::moves_as_ship()`, retreat, and `production::destroy_blockaded_mobile_docks` after movement and combat. Deorbit Barrage (`saarbt`) and the commander's effect are otherwise implementable once their routes exist: the commander ("When you produce fighters or infantry: place each at any of your non-blockaded space docks") needs a placement hook in `production.rs` (override the placement spot per unit); the breakthrough ACTION needs `breakthrough` component-action plumbing for the module (the shared `breakthroughs` registry) and a 2-system distance helper from asteroid-field systems.
6. Destruction of infantry by the hero is a direct removal (no destruction event exists for ground forces); fighters go through `combat::destroy_units` which stages `SHIP_DESTROYED` in `pending_destructions` but nothing announces them from a leader hook (leader hooks get no `Resolving`).

## Rules questions

* Ragh's Call: who chooses the destination planet? Implemented as the Saar player. Is the planet being invaded a legal destination? Excluded (the forces leave it).
* Hero: "adjacent" uses `Galaxy::adjacent` (hex neighbours plus wormholes), not player-specific links.
* Scavenger Zeta: "place 1 mech on that planet" ignores nothing; refused when the mech box is empty (31.4).

## Decision sites to register

| Function | Choices |
|---|---|
| `saar::ragh_call` (via `ask`) | 1 |
| `saar::chaos_mapping_production` (via `ask`) | 1 (system pick, only when several) |
| `saar::hero` (via `ask`) | 1 |

(The helper `ask` builds the `Choice`; the coordinator should register it as one producer, as with `yin.rs::ask_one`.) `produce` runs the ordinary production window (`production::produce_by_ability`), which is registered already.

## Commands run

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- factions::saar` | 9 passed, 0 failed |
| `cargo test -p ti4-engine --lib -- factions::` | 251 passed, 3 failed (all `factions::naaz::tests`, another agent's work), 1 ignored |
| `cargo clippy -p ti4-engine --all-targets` | no warning in `saar.rs` (type_complexity allowed with reason on `ragh_targets`) |
| `rustfmt --edition 2024 crates/ti4-engine/src/factions/saar.rs` | run |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1760 passed, 3 failed (naaz), 1 ignored; `decision_delivery_inventory` 1 failed (unregistered sites, including mine above); other binaries ok |

Ledger line: `saar       5/13 implemented` (missing: nomadic, cm, ffac2, saar_spacedock, saar_spacedock2, saaragent, saarcommander, saarbt).

## Review fixes (Opus review)

| Finding | Fix |
|---|---|
| B2 | Hero now also destroys other players' infantry in the space area (`units_of_base` yields space infantry; removed with `remove`). Test pushes space infantry. |
| B3 | Hero removes fighters directly like infantry, everything decided before the first removal; nothing is staged in `pending_destructions` (asserted). Hook request: leader effects cannot announce destructions (no `Resolving` in `use_leader`). |
| S1 | `production_systems` requires non-empty `production::producers`. Chaos Mapping's production is registered under `cfg!(test)` only until `cm` can be claimed whole (activation bar waits on `MovementHooks::cannot_activate`, not implemented). |
| S4 | `scavenge` and `saar_mech` unclaimed (partial): they fire only from the invasion landing path; the hook request (typed event at every control change) is Hook request 4 above. Abilities stay registered. |
| N6 | Hero adjacency uses `movement::PlayerAdjacency`. |
| N7 | Scavenge gated on `faction_abilities::has(.., "scavenge")`. |

Decision sites: unchanged (`ask` callers `ragh_call`, `chaos_mapping_production` (test-only registration), `hero`; none renamed).

Checks: `factions::saar` 9 passed; clippy no warning in `saar.rs`; rustfmt run. Ledger: `saar 3/13 implemented` (claimed: saar_flagship, ragh, saarhero).
