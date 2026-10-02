# BF-00c-ground: invasion and ground-combat routes

Files: `crates/ti4-engine/src/invasion.rs`, `crates/ti4-engine/src/factions/hooks_ground.rs`. No
faction card is implemented. Hook fields live in `GroundHooks` (`Hooks::ground`), dispatched from
`hooks_ground.rs` over `MODULES` in order. Events are emitted by the live path (`InvasionWindow`,
through `Resolving::emit`); the synchronous test-only entry points (`invasion::ground_combat`,
`commit_ground_forces`, `resolve` without a timing handle) emit nothing. All events are new
type names; no action card listens to them (`SUSTAIN_DAMAGE_USED`, `SHIP_DESTROYED` untouched).

## Items

| # | Consumer | Status | Added | Call site | Tests |
|---|---|---|---|---|---|
| 1 | `sardakk_mech` | done | event `GROUND_FORCE_SUSTAINED`: `system`, `planet`, `player` (owner), `unit` (type id), `cause` | `ground_hit_logged` -> `flush_events`, after the hits of a ground round / Harrow / bombardment step / space cannon defense | `a_ground_combat_announces_its_start_sustains_destruction_and_end`, `a_timing_effect_can_assign_ordinary_hits_and_the_events_follow` |
| 2 | `lw2`, `arborec_infantry2`, `yinagent` | done for every ground-force destruction in `invasion.rs` | event `GROUND_FORCE_DESTROYED`: `system`, `planet`, `player` (owner), `unit`, `damaged` (bool), `cause` | same | same, plus `a_defender_who_holds_...` |
| 3 | `indoctrination`, `greyfire`, `yin_mech` | done | event `GROUND_COMBAT_STARTED`: `system`, `planet`, `attacker`, `defender` | `InvasionWindow::start_ground_combat`, from `advance_fighting` and the coexistence "fight next" answer; emitted before the feat occurrence is begun and before round 1 | first test |
| 4 | `sardakkbt` | done | event `GROUND_COMBAT_ENDED`: `system`, `planet`, `attacker`, `defender`, `winner` (absent if both sides were wiped out), `control_changed` | `finish_ground_round`, after Dacxive/feats, before the next planet | first two tests (winner invader with control change; winner defender without) |
| 5 | `vpw` | done | hook `ground_rolls_extra_hits(state, content, sources, roller, system, planet, hits) -> usize` (additional hits, summed) | `resolve_ground_round`, after the reroll windows and X-89, per side; the opponent removes the hits as ordinary hits via `remove_ground` | `an_added_hit_is_assigned_by_the_opponent_like_any_other` (with and without hook) |
| 6 | Mentak mech/flagship | done (ground side) | hook `may_sustain(state, content, sources, &CombatUnit) -> bool`, all modules must allow; `context == "ground"` | `can_sustain_here`, used by `apply_ground_hit` (rounds, Harrow, bombardment, space cannon defense) and the sync `absorb_ground` | `a_prohibition_stops_sustain_damage_and_the_unit_dies_to_the_first_hit` |
| 7 | `sardakkcommander` | done | hook `commit_candidates(state, content, sources, invader, active_system, already: &[(SystemId, PlanetId)]) -> Vec<CommitCandidate{system, planet, unit}>`; engine validates candidates, offers `commit|<n>|<planet>` after the ordinary options (existing ids unchanged) with `from_system`/`from_planet`, moves the unit and records the origin in the window (so `already` limits the hook); `UNITS_COMMITTED` gains `from_system`/`from_planet` for these | `commit_pool`, `commit_options`, `landing_options`, Committing arm of `resolve` | `extra_commit_sources_follow_the_ordinary_ones_and_the_engine_moves_the_unit`, `a_candidate_that_is_not_really_there_is_not_offered` |
| 8 | `blood_ties` | done | hook `custodians_free(state, content, player) -> bool` (any module); `invasion::custodians_cost` (pub); prompt reads "(no influence required)" when free; 27.2a still applies | `custodians_removable`, `pending_choice` preview, `resolve` (skips `production::pay`) | `the_custodians_token_can_come_down_free_from_a_window_opened_at_the_commit_step` |
| 9 | Yin hero, Sardakk hero | partial | `InvasionWindow::at_commit_step(state, invader, system)` (no bombardment, no dice) and event `GROUND_COMMITMENT_FINISHED`: `system`, `player`, `planets` (array) | `finish_committing` | same test as 8 |

Extra: `invasion::assign_ground_hits_in_timing(resolver, ctx, system, planet, player, hits, cause)`
(`pub(crate)`): lets a timing effect "produce a hit" (Sardakk mech) with the ordinary hit rule and
the events above. Test: `a_timing_effect_can_assign_ordinary_hits_and_the_events_follow`.
Test-only: `hooks_ground::test_support::with_hooks` installs one extra hook set after the modules
(thread local), since `MODULES` is const.

`cause` values: `ground_combat`, `harrow`, `space_cannon_defense`, `bombardment`.
Event order inside a round: both sides' hits are applied first (42.2), then the round's events are
emitted in application order (defender's casualties first, then invader's, then Harrow); handlers
see the board after the whole round.

### Item 9: what is still missing (blocked part)

Yin hero ("Commit up to 3 infantry from your reinforcements to any non-home planets and resolve
ground combats on those planets. Players cannot use SPACE CANNON against these units. Then, purge")
and Sardakk hero (skip to the commit step; after committing, purge and return ships). An invasion
cannot be launched as an effect cleanly today:

1. `InvasionWindow` is driven only by `game.rs` (`Aftermath::Invading`, `InvasionWindow::new*` at
   game.rs ~339, ~8450, ~8578). `use_leader` hooks receive only a `TimingContext`, which has no
   `Resolver`/`TimingHandle`, so a synchronous invasion run from there would have `timing: None`
   and emit none of the events above (Indoctrination, Greyfire etc. would not fire). A resumable
   window needs a new `Aftermath` entry in `game.rs`: not writable here.
2. Yin hero needs commitment from reinforcements (a source with no origin planet; placement is
   supply-capped via `action_cards::place_units`), planets in any system (the window is bound to one
   system), non-home filtering, and a "no space cannon defense" flag on the window (the
   `space_cannon_defense` call in `finish_committing`). None exists; adding them without a caller
   would be speculative.
3. Sardakk hero: `at_commit_step` now provides the window. The decision point that lets the player
   skip movement/space combat/bombardment in a tactical action is in `game.rs`'s tactical-action
   flow, and the purge/ship return is the faction's, after `GROUND_COMMITMENT_FINISHED`.

## Requests

| File | Function | Need | Why |
|---|---|---|---|
| `game.rs` | tactical action / `Aftermath` | an entry that opens `InvasionWindow::at_commit_step` (offered when an unlocked hero's hook says so), and a way for a leader effect to open an invasion window as an `Aftermath` | Sardakk and Yin heroes (item 9) |
| `reactions.rs` | `EMITTED_EVENTS` | nothing needed now; add the five new names only if an action-card window ever maps to them | coverage ledger counts only listed events |

## Rules questions

- Custodians (27.2) in `at_commit_step`: I start at the custodians offer, as in a normal invasion,
  because the engine models the removal as the step just before committing. If the removal is only
  allowed during the normal invasion, the Sardakk hero should skip it; confirm before wiring it.
- `control_changed` is predictive (control is established at the end of the invasion, 49.5): it is
  true when the winner is the invader and did not control the planet.
- Sardakk mech: the hit is produced "after this unit uses SUSTAIN DAMAGE"; the event is emitted at
  the end of the round, so that hit lands after the round's simultaneous removals rather than with
  them. Flag if a rule ruling wants it simultaneous.
- Each new event allocates one event id from `EventSequence` even with no listener, and appears in
  `Resolver::applied_events`/log. Game play is unchanged; anything that hashes or compares event
  ids across the change would see different numbering after the first ground event.

## Behaviour neutrality

Hooks default to neutral (`ground_rolls_extra_hits` sums to 0, `may_sustain` is an empty `all` =
true, `commit_candidates` empty, `custodians_free` false). The ground-round dice draw order is
unchanged; `take_bombard_hits`, `remove_ground`, `ground_hit` keep the same unit selection (the old
`ground_hit` was split into `apply_ground_hit` + logging). Feat occurrence numbering is unchanged:
`GROUND_COMBAT_STARTED` is emitted before `begin_feat_occurrence`. Not run: ti4-sim (forbidden here).

## Commands and results

| Command | Result |
|---|---|
| `cargo test -p ti4-engine --lib invasion::ground_routes` | 8 passed |
| `cargo test -p ti4-engine --lib invasion::` | 59 passed, 0 failed |
| `cargo test -p ti4-engine --lib factions::` | 11 passed, 1 ignored (ledger report) |
| `cargo test -p ti4-engine -q --no-fail-fast` | lib 1436 passed, 1 ignored; doc/other targets pass; **1 failed**: `decision_delivery_inventory::every_producer_and_delivery_site_matches_the_reviewed_registry`, caused by other agents' files (`action_cards.rs::place_units_choosing` Choice+AskObserved, `production.rs::produce_by_ability` AskObserved are unregistered); no `invasion.rs` site differs |
| `cargo clippy -p ti4-engine --all-targets` | no warnings in `hooks_ground.rs`; `invasion.rs`: two left, both pre-existing code I did not change (`roll_bombard_plan` too_many_lines 106/100, a redundant closure in `space_cannon_defense`); other files' warnings are other agents' |

## Review fixes

- B4: `ground_rolls_extra_hits` now takes `(.., hits, opponent_hits)`. Real `vpw` text (technologies.json): "After making combat rolls during a round of ground combat, if your opponent produced 1 or more hits, you produce 1 additional hit." Both sides' final (post-reroll, X-89) hits are read before either side adds anything, so module order cannot matter. Hook doc quotes the real text. Tests: `an_added_hit_is_assigned_by_the_opponent_like_any_other` (reworked: invader misses, defender hits, invader adds one) and `the_pre_hook_hits_of_the_opponent_are_what_a_weave_reads` (both seats; added hits are not visible to the other side).
- Nit: test hook helper is now `hooks_ground::with_test_hooks<T>(hooks, run) -> T`, restoring the previous value (drop guard, panic safe).
- Doc: `GROUND_FORCE_DESTROYED` covers invasion paths only.

### Known gap: ground-force destruction outside `invasion.rs`
Not emitting `GROUND_FORCE_DESTROYED`/`GROUND_FORCE_SUSTAINED`: action-card effects (e.g. Direct Hit is ships; ground-force kills by cards such as Reactor Meltdown-like/structure effects), agenda/law effects, other combat-adjacent code in `combat.rs`, `action_cards.rs`, `laws.rs`, `leaders.rs`, `faction_*.rs`. Not audited exhaustively; those files are outside this package's writable set.

### Results (after fixes)
| Command | Result |
|---|---|
| `--lib invasion::` | 60 passed, 0 failed |
| `--lib factions::` | 11 passed, 1 ignored |
| `-q --no-fail-fast` | lib 1453 passed, 2 failed (`combat::space_routes_tests::hits_produced_*`, other agent's in-progress combat.rs); `decision_delivery_inventory` still fails (other agents' sites); other targets ok |
| clippy | nothing in `hooks_ground.rs`; `invasion.rs` only the 2 pre-existing (L401 `roll_bombard_plan`, redundant closure L3209) |
