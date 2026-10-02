# BF-00c-space — space-combat engine routes for faction modules

Branch `wp/base-factions`. Writable set: `combat.rs`, `factions/hooks_combat.rs`, this file. No faction
card is implemented. With every module empty the new code paths are no-ops (see "Neutrality").

## Items

| # | Item | Status | Route |
|---|---|---|---|
| 1 | Round-ended event (`exo2`) | done | typed event `SPACE_COMBAT_ROUND_ENDED` |
| 2 | Combat-ended event (`so`, Sardakk BT) | done | typed event `SPACE_COMBAT_ENDED` |
| 3 | Destroy chosen ships as an effect (`exo2`, Yin flagship) | done | `combat::destroy_units`, `destroy_ships_announced`, `announce_staged_destructions` |
| 4 | May-sustain gate, space (Mentak flagship) | done (space) | `CombatHooks::may_sustain` |
| 5 | Direct Hit immunity (`sardakk_dreadnought2`) | done in combat.rs; one call-site Request | `CombatHooks::direct_hit_immune`, `combat::direct_hittable_at` |
| 6 | Hit production (Ambush, Impulse Core, Devotion) | done | `CombatHooks::produced_hits` at three `CombatMoment`s |
| 7 | AFB excess (Raid Formation) | done | `CombatHooks::afb_excess` |
| 8 | Per-unit identity | recorded, no code | see Requests |

### 1. `SPACE_COMBAT_ROUND_ENDED`
Payload: `system`, `round` (int, 1-based), `attacker`, `defender` (player id strings).
Emitted from `CombatWindow::settle_stages` when the round's hit queue is empty: after every hit is
assigned, after Duranium Armor, after `CombatMoment::RoundEnded` hits are assigned, and **before** the
"over / round limit" check and the retreat step. It fires for the round that finished the fight too, so
`count == outcome.rounds`. Staged destructions are drained (announced) right after.
Text: Sardakk Exotrireme II "After a round of space combat, you may destroy this unit to destroy up to 2
ships in this system."

### 2. `SPACE_COMBAT_ENDED`
Payload: `system`, `attacker`, `defender`, `rounds` (int), `winner` (string, **absent** for a draw or mutual
wipe-out), `losers` (array, the sides that did not win; empty without a winner), `reason`
(`"destruction"`, `"retreat"`, `"draw"` = Skilled Retreat, `"round_limit"`), `retreated` (array of players
who left), `destroyed` (array of `{player, unit}`: ships either side had when the fight opened that are
neither still in the system nor arrived with a retreating fleet; includes fighters lost to barrage and ships
an effect destroyed).
Emitted once, from `CombatWindow::settle` (wrapper over `settle_stages`) the first time the stage is
`Done`; guarded by `end_announced`. A window that never fought (fewer than two sides) emits nothing. It
fires **before** `game.rs` emits `SPACE_COMBAT_WON` and before the invasion. Text: Mentak Salvage Operations
"After you win or lose a space combat, gain 1 trade good; if you won ..., produce 1 ship ... of any ship
type that was destroyed during the combat" (`destroyed` is the list of types).
Limitation: when a retreat strands carried units (no capacity), those are counted as arrived, not destroyed
(`retreat_to` returns only a count).

### 3. Destroy API
`destroy_units(state, content, sources, player, system, victims: &[Unit]) -> usize`: every victim must be a
ship of `player` in the space area (matched by value, multiset); otherwise nothing changes and `0` is
returned (atomic). Removes them and stages one `(system, owner, type)` per ship in
`state.pending_destructions` (the existing route window effects use, since a `TimingContext` holds no
resolver). `announce_staged_destructions(state, ctx)` drains it as `SHIP_DESTROYED` (`system`, `player`,
`unit`, `last`), repeating until empty so chained destructions (Van Hauge) resolve; it also sets
`last_ship_destroyed` like the combat's own announcements. `destroy_ships_announced` = both, for callers
holding a `Resolving`. The combat window runs the drain after every event or hook where an effect can stage
one (round-ended, combat-ended, each `produced_hits` call, `afb_excess`). `announce_ship_destroyed` was
refactored onto the same `announce_ship_destroyed_type` (no behaviour change; unused `content`/`sources`
params dropped).
**A module whose effect runs from a `timing_abilities` window outside combat.rs's drains must have its
staged destructions announced by whatever emits that window** (as `reactions.rs::announce` does for action
cards). Windows on the two new events and on `SHIP_DESTROYED` are covered by combat.rs.

### 4. `may_sustain`
`fn(&GameState, &ContentStore, SourceSet, &CombatUnit<'_>) -> bool`; **all** modules must allow. `unit.player`
= sustaining unit's owner, `unit.system` = combat system, `unit.context == "space"`, `unit.unit_type`. Asked
as the last clause of `sustains_in_space` (which now also takes `content, sources, system`), used by both
sustain offers (`CombatWindow::sustainers`, `offer_sustain`/`absorb_hits`). Mentak flagship text: "Other
player's ships in this system cannot use SUSTAIN DAMAGE" — the module checks that *another* player owns the
flagship in `unit.system`.
**Ground path (for the ground agent):** `invasion.rs::can_sustain_here` (kind-level check at ~L1121) and
`apply_ground_hit` / `absorb_ground` (~L1142, ~L1294). Mentak mech: "Other player's ground forces on this
planet cannot use SUSTAIN DAMAGE." A ground hook needs `(state, content, sources, &CombatUnit{ planet: Some(..) })`
with `context: "ground"`; `CombatUnit` already supports it.

### 5. `direct_hit_immune`
`fn(&GameState, &ContentStore, SourceSet, &CombatUnit<'_>) -> bool`; **any** module may make a ship immune.
Gate: Direct Hit's window (`reactions.rs::direct_hit_guard`) reads the `direct_hittable` payload of
`SUSTAIN_DAMAGE_USED`, which `emit_sustain_used` now computes with the new
`combat::direct_hittable_at(state, content, sources, player, system, unit)` = printed `canBeDirectHit` **and**
no module immunity. Note `sardakk_dreadnought2` already has no `canBeDirectHit` in `units.json`, so Exotrireme
II is covered by data (the corpus carries the rule); the hook is for what a flag cannot express.
The effect `action_cards.rs::direct_hit` (L1033) still asks the printed `combat::direct_hittable(...)` only:
see Requests.

### 6. `produced_hits`
`fn(&mut TimingContext<'_>, &HitSite<'_>) -> ProducedHits`, `ProducedHits { any_ship, non_fighter }`
(`non_fighter` = "must be assigned to a non-fighter ship if able", Impulse Core; sums over modules). Called
for each participant (attacker, then defender), for every module, at:

| `CombatMoment` | Where | Cards |
|---|---|---|
| `CombatStart` | round 1, after AFB (and after its scoring pause), before the retreat announcement; hits get a queue of their own ("`HitPhase::CombatStart`") resolved before any die | Mentak Ambush, Yin Impulse Core ("At the start of a space combat") |
| `AfterRoll` | after both sides rolled, rerolls and War Funding, folded into the round's simultaneous hits (78.6) | spec item 6(b) |
| `RoundEnded` | after the round's hits and Duranium Armor, own queue, before `SPACE_COMBAT_ROUND_ENDED` | Yin Devotion ("After each space combat round") |

Hits are queued as `Pending { producer = the module's player }`, so the opponent gets the ordinary SUSTAIN
DAMAGE offer and casualty choice, `HITS_TO_ASSIGN`, `SUSTAIN_DAMAGE_USED` (`producer`) and Direct Hit keys on
the producer. The hook receives a real `TimingContext` built from the combat's `Resolving` (`with_timing`):
same state, `table`, `dice`, `rng`; with no timing machinery (sync `combat::resolve`, tests) the event
allocator is detached and `galaxy` is `None`. **Dice for Ambush come from `context.dice` with a reason label
chosen by the module** (e.g. `context.dice.roll_by(context.rng, n, "ambush", Some(value), player)`); the hook
runs inside the stream, so the draw order is part of the contract (attacker's hook, then defender's).
A hook pays its costs itself (Devotion/Impulse Core destroy their cruiser via `destroy_units`) before
returning hits.
Rules question: Devotion's printed window is "after each space combat round" while the spec asked for "after
a participant's roll"; both moments exist. Whether Devotion resolves before or after a retreat is announced is
open (implemented: before, i.e. directly after the round's hits).

### 7. `afb_excess`
`fn(&mut TimingContext<'_>, &AfbExcess { producer, target, system, excess })`. Called from `apply_barrage` after
`destroy_fighters`, when `hits > fighters_before`, for the producer's barrage; not for a Waylay barrage (those
hits are ordinary combat hits). AFB lives in combat.rs, so no Request. The module damages the target's SUSTAIN
DAMAGE ships itself (Raid Formation: "for each hit produced in excess of your opponent's fighters, choose 1 of
your opponent's ships that has SUSTAIN DAMAGE to become damaged"); the hook must verify its own ownership of
the ability (it is called for every module and every barrage).

### 8. Unit identity
`CombatUnit` (mod.rs, not edited) carries `unit_type` only. Damaged/sustained state per unit is **not**
available to `unit_roll_modifier`/`unit_dice`/`may_sustain`/`direct_hit_immune`. If a card needs it (none of
the above does), `CombatUnit` needs a `damaged: bool` field set by each constructor in `combat.rs`
(`fleet_groups`, `effective_hits_on`, `sustains_in_space`, `direct_hittable_at`) and `invasion.rs`. `Unit` has
no identity either (values; `remove` takes the first equal), so `destroy_units` names ships by value.

## Neutrality
- New events are new type names (no existing listener). Emission with no registered ability is
  `Resolver::emit`, which consumes one event id from the game's `EventSequence` per emit: event ids after a
  combat shift by `rounds + 1`. Any saved-state or trace comparison that includes event ids is affected;
  game *play* (choices, dice) is not. Coordinator: confirm with ti4-sim.
- No dice are drawn by new code; hook dispatch with no hooks returns neutral values and builds no queue
  (`produced_hits` -> NONE, `may_sustain` -> true, `direct_hit_immune` -> false, `afb_excess` no-op).
- `an_empty_hook_table_changes_nothing` replays a 9-ship fight with an installed empty table against none and
  compares outcome, the full probed event log and the board.
- The only behaviour-visible refactor is `announce_ship_destroyed` (same payload, same order).

## Requests (files I do not own)
| File | Request |
|---|---|
| `action_cards.rs::direct_hit` (~L1033) | Replace `crate::combat::direct_hittable(context.content, context.sources, unit_type.as_str())` with `crate::combat::direct_hittable_at(context.state, context.content, context.sources, &victim, &system, unit_type.as_str())` so the effect honours module immunity like the window guard does. |
| `invasion.rs` (ground agent) | Ground SUSTAIN gate for Mentak mech at `can_sustain_here` / `apply_ground_hit` / `absorb_ground`; add a ground hook beside the space one and reuse `CombatUnit { planet: Some(..), context: "ground" }`. |
| `factions/mod.rs` | Optional: `CombatUnit` damaged flag (item 8) if a later card needs it. |
| `reactions.rs` | Optional: if a module's `timing_abilities` effect on a non-combat event stages destructions with `destroy_units`, that event's emitter must call `combat::announce_staged_destructions` (the action-card drain does its own). |

## Checks (exact)
| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib combat::` | 82 passed; 0 failed (11 are the new `combat::space_routes_tests`) |
| `cargo test -p ti4-engine --lib factions::` | 11 passed; 0 failed; 1 ignored (3 new in `hooks_combat::tests`) |
| `cargo test -p ti4-engine -q` | lib 1444 passed, 1 ignored; one integration binary fails: `decision_delivery_inventory::every_producer_and_delivery_site_matches_the_reviewed_registry`. The scan differs only by `action_cards.rs::place_units_choosing` (Choice, AskObserved) and `production.rs::produce_by_ability` (AskObserved) — other agents' unregistered new asks; nothing from combat.rs. |
| `cargo clippy -p ti4-engine --all-targets` | no warnings on lines I added; 5 pre-existing warnings remain in combat.rs (L1644, 1657, 1819, 3520, 6685) |
| `rustfmt --edition 2024 --check` on both files | clean |

New events/hook tests: round/ended payloads; retreat reason and non-destruction of the retreating fleet; start
hits before any die with sustain offered and `producer`; forced non-fighter hit; after-roll hit adds exactly one
hit with identical dice; round-end hit lands before `SPACE_COMBAT_ROUND_ENDED`; sustain barred; Direct Hit
immunity in the payload and `direct_hittable_at`; empty-table equivalence; `destroy_units` atomicity and
announcement; AFB excess. Hook dispatch semantics (all/any/sum/empty) in `factions::hooks_combat::tests`.
Call-site tests install a table through the `#[cfg(test)]` thread-local `hooks_combat::TEST_HOOKS` (module
const tables are empty), which does not exist in non-test builds.

## Review fixes (independent Opus review)

| Finding | Fix |
|---|---|
| B1 start hits came after AFB | `CombatMoment::CombatStart` hits are now produced right after Assault Cannon, before AFB. When their queue empties the window sets `resuming_round` and re-enters `open_round`, which skips the round-start events and Assault Cannon and resumes at the AFB step. Test: first `HITS_TO_ASSIGN` precedes `ANTI_FIGHTER_BARRAGE_STARTED`; barrage announced once per side; one `COMBAT_ROUND_STARTED` for round 1. |
| B2 producer assigns Devotion's hit | `ProducedHits.producer_assigned`; queued as `Pending.producer_assigns`. The producer is the chooser of the "assign a hit" decision (owner still gets the SUSTAIN offer first; neutral shortcut skipped). Test with a recording decider: producer asked for producer-assigned, owner for ordinary. |
| B3 round end before retreat | `SPACE_COMBAT_ROUND_ENDED` and `RoundEnded` hits now come after the retreat step (Retreating with nobody left to move), and on the conclude paths (fight over, round limit) the event is emitted before concluding. Test: in a retreat the last ROUND_ENDED sees 0 `b` ships. A mid-roll conclude (a reroll window empties a fleet before the round's hits) emits no round-ended event: the round never completed. |
| S2 error channel | `produced_hits` and `afb_excess` return `Result<_, IllegalChoice>`; errors propagate as `CombatError` (staged destructions are announced first). Tests: combat fails; barrage fails. |
| S7 | `open_round_end_hits` returns without asking when `over`. Test: hook asserts both sides have ships on every `RoundEnded` call. |
| Nit | `#[cfg(test)] hooks_combat::with_test_hooks(hooks, run)`, drop-guard restore; `TEST_HOOKS` is private. |

Rules question (replaces the earlier one): Devotion/"after a round" is implemented after retreats (78.7). The
moment is still not explicitly ordered against SPACE_COMBAT_ROUND_ENDED listeners: hits first, then the event.

Checks: `--lib combat::` 85 passed; `--lib factions::` 11 passed, 1 ignored; `cargo test -p ti4-engine -q
--no-fail-fast`: lib 1455 passed, 1 ignored, all binaries ok except `decision_delivery_inventory` (production.rs
`resolve`/`resolve_timed` asks from another agent; no combat.rs site); clippy: no new warnings, the 5
pre-existing combat.rs ones remain; rustfmt clean.

## Coordinator follow-up (2026-10-02)

The agent was cut off by a rate limit before the verification fix. Applied by the coordinator:
producer-assigned hits skip the neutral auto-answer only in the Assigning stage, so a neutral
side's SUSTAIN DAMAGE answer stays automatic (`combat.rs`, `producer_picks`). Test
`a_producer_assigned_hit_on_neutral_ships_never_asks_the_neutral_side` (sustain-capable and
non-sustaining neutral fleets) fails with the old condition and passes with the fix.
