# BF-F1: ground and control routes (packages A, B, G, I-ground)

Branch `wp/base-factions`, uncommitted. Writable scope used: `invasion.rs`, `factions/hooks_ground.rs`,
`action_cards.rs`, `agenda_effects.rs`, `legendary.rs`, `thunders_edge.rs`, this file, plus the
signature (and one test call) line of `commit_candidates` in `factions/sardakk.rs` as briefed.

## Status

| Item | Status |
|---|---|
| A. `GROUND_FORCE_DESTROYED` from every ground-force destruction path in scope | Done (staged; needs coordinator flush) |
| B. `PLANET_CONTROL_GAINED` from every control gain in scope | Done (staged; needs coordinator flush) |
| G. `commit_candidates` gets `Option<&Galaxy>` | Done; window gets the map (see below) |
| I (ground half). `ground_combat_round_started` hook and landing flip | Done |

## A. GROUND_FORCE_DESTROYED

Invasion paths already emitted it live (unchanged). New staged sources (cause string in payload):

| Site | Cause |
|---|---|
| `action_cards::destroy_on_planet` (infantry/mech only; structures stage nothing) | `action_card:plague`, `action_card:unstable_planet` (`reactor_meltdown` kills docks: nothing staged) |
| `agenda_effects::clear_planet` | `agenda:redistribution`, `agenda:disarmament`, `agenda:core_mining`, `agenda:demilitarized_zone` |
| `agenda_effects` plowshares (For) | `agenda:plowshares` |

Not emitted, by design: `invasion::absorb_ground` / `ground_combat` / `establish_control` (synchronous
test-only entry points, documented as emitting nothing); `replace_unit`, Parley, Ghost Squad moves
(not destruction); mech-to-infantry conversion (Minister/agenda, not destruction). No ground-force
destruction exists in `legendary.rs` or `thunders_edge.rs`.

### Staging mechanism and flush request (coordinator)

A card/agenda effect has no resolver, and `GameState` cannot take a new field from my files, so
events are staged as rows in `faction_marks` under the invisible namespace `private:#staged:ground:<n>`
(`mark_visible_to` shows it to no seat; `cards:staged:` numbering/draining is not disturbed).

**Flush function:** `crate::factions::hooks_ground::announce_staged_events(&mut GameState, &mut Resolving)`
(drains in staging order, loops until empty, ignores cancels). `has_staged_events(&GameState)` is the
cheap guard. Please call it:

1. inside `Game::announce_staged_destructions` (beside `combat::announce_staged_destructions`), which
   already runs after component and leader actions;
2. after an action card finishes resolving (card plays go through the timing resolver);
3. after an agenda outcome resolves (`agenda_effects::resolve_with` callers);
4. after the Thunder's Edge expedition `perform` and after legendary `pass` (Maxis) -- the
   component-action flush in (1) covers expedition; passing needs the same call.

Until flushed the rows sit in `faction_marks` (they make the state non-default for hashing/saves,
unlike a game that never plays these cards). An unflushed row is harmless but the events do not fire.

## B. PLANET_CONTROL_GAINED

`reactions::window_table` listens on it: "When you gain control of a planet" (`actor_is`, Infiltrate) and
"After another player gains control of a planet you control" (`actor_is_not`, Reparations). Effect of
this package on play (approved, recorded): Infiltrate/Reparations can now be played off control gains
from Maxis Central Control and Thunder's Edge placement, which previously never opened those windows.
Invasion landings already emitted it. The staged announcement also sets `state.last_control_gained`
(which Infiltrate/Reparations read), as the invasion does.

Control-change sites found in my files (non-test): `legendary.rs` Maxis Central Control (staged, with
`previous_owner` captured first; offered planets are unheld so none in practice), `thunders_edge.rs`
`complete_expedition` (staged, no previous owner), `invasion.rs` `finish_control_gain` (already live).
No other non-test `set_control` exists in `action_cards.rs`/`agenda_effects.rs`. Colonial Redistribution
does not change control at all today (it clears the planet and places an infantry for the trailing
player): unchanged, no event; flagged as a rules gap, not touched.

## G. commit_candidates + galaxy

`GroundHooks::commit_candidates(state, content, sources, galaxy, invader, system, already)`; dispatcher
and `invasion::commit_pool` updated. `InvasionWindow` now stores an `Arc<Galaxy>`: set by the new
`InvasionWindow::with_galaxy(&Galaxy)` (coordinator: call it in `game.rs` where windows are built, lines
~341/8683/8811, to have the map before the first answer) or, failing that, captured from the resolver
handle at the first `resolve`. The commit ask built before any answer (`pending_choice`, no resolver)
sees `None` unless `with_galaxy` was used. `sardakk.rs`: signature line `_galaxy: Option<&Galaxy>`
added and one test call passes `None`; nothing else touched.

## I (ground half)

* `GroundHooks::ground_combat_round_started(&mut GameState, &ContentStore, SourceSet, &mut Table,
  &PlayerId, &SystemId, &PlanetId)`: called for invader then defender at the start of each ground combat
  round, in the live window (`resolve_ground_round`, before Dunlain Reaper and the dice) and in the
  synchronous `ground_combat`. Does not itself advance `combat_round_seq`.
* `invasion::land_unit` (both commit paths) flips a ship-form unit (`isShip`) to its ground form on
  landing with `fleet::flip_form`. `parley` (action_cards) flips a landed ground form back to its ship form when
  returning it to space. Space-side flips (start/end of space combat) are `combat.rs`, not mine.

## Tests (all new ones pass)

* `hooks_ground::tests`: staging order/payload/once/invisibility, hooks neutral when empty.
* `action_cards`: plague stages per kill (payload, cause), no victims stage nothing, a space dock stages
  nothing, Parley flips Eidolon to `naaz_mech_space`, Parley leaves infantry alone.
* `agenda_effects`: disarmament and plowshares stage with agenda cause; nothing staged when empty.
* `legendary`: Maxis stages the control gain; `thunders_edge`: expedition stages it (assert added to the
  existing completion test).
* `invasion::ground_routes`: round-start hook once per participant per round (live) and in sync combat,
  neutral without the hook; ship-form unit lands as ground form (helper and sync commit); commit hook
  receives the window's map.

## Commands and results

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib -- invasion:: action_cards:: agenda_effects:: legendary:: factions:: thunders_edge::` | 552 passed, 0 failed, 1 ignored |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1845 passed, 1 ignored; all binaries pass except `decision_delivery_inventory` (2 of 4 fail). I added no `Choice` site; the failure is the unregistered faction-module sites other agents are adding (not diagnosed further) |
| `cargo clippy -p ti4-engine --all-targets` | no warning in `hooks_ground.rs`, `legendary.rs`, `thunders_edge.rs`, `sardakk.rs`; remaining in my files are pre-existing (`invasion.rs` 401 too_many_lines, 3273 redundant closure; `agenda_effects.rs` 1335/1346; `action_cards.rs` 3369) |
| `rustfmt --edition 2024` on each file I edited | applied |

New decision sites: none. Request to others: none beyond the coordinator flush wiring above.
