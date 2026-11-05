# Kernel perf: the ranked plan implemented (items 6, 5, 3, 1, 2 of `docs/PERF_KERNEL.md`)

Lane `core-perf-impl`, branch `core-perf-impl` from `core-perf-kernel` (a21a6a240), 2026-11-05. One commit per item, in the order 6, 5, 3, 1, 2. **Behaviour-neutral: every check below
is identical after every item** (rules version stays 36). No heavy gate was run (dual-all, ratchet, soak, census, golden dual): they run later on the merged tip.

## Result per item (base = a21a6a240, instructions from `perf stat -e instructions:u`, exact; wall = min of 3 interleaved suite runs at load 1.6 to 3, +-5 %)

| step | commit | 4p s1 instr M | 6p s2 instr M | suite instr G (setup incl.) | suite wall ms | 14-game suite allocated | note |
|---|---|---|---|---|---|---|---|
| base | a21a6a240 | 1698.0 | 2671.2 | 26.48 | 2914 | 7.84 M allocs, 2479 MB | |
| (6) `begin_paid` existence check | ce4999f86 | 1685.2 (-0.8 %) | 2655.5 | 26.33 | 2884 | | no site list, no sort |
| (5) `AbilityTable` behind `Arc` | 84a829240 | 1666.9 (-1.1 %) | 2635.1 | 26.11 | 2852 | 7.49 M, **1711 MB (-31 %)** | specs, forbids, card plays |
| (3) board indexes, unit/tech tables | 25af3b0cc | 1160.0 (-30.4 %) | 1699.5 (-35.5 %) | 15.97 | 1976 | | owner/system sets, per-base units, replaced techs |
| (1) lazy play facts | 486210476 | 1076.1 (-7.2 %) | 1542.5 (-9.2 %) | 14.52 | 1836 | | snapshot + first-read computation |
| (2) kept `Env` | fa9458168 | **577.0 (-46.4 %)** | **866.3 (-43.8 %)** | **7.94** | **1106** | **2.25 M, 869 MB** | registry and table reused while the key holds |
| **total** | | **-66.0 %** | **-67.6 %** | **-70 %** | **-62 %** (4p s1 201 -> 85 ms; 5.1 k -> 13.3 k steps/s) | **-71 % allocs, -65 % bytes** | |

Identical after every item: `perf_kernel suite` hash `775f4136e60a99d5` (14 games, chain and final-state hash of each), `native_play --quick` fingerprint `538dc68f23dddcc9` / 1165 steps,
the 24 `perf_ab core smart` games (3p to 6p, seeds 1 to 6: chain, steps, decisions, finish, `bad` equal to the base binary's, all 24 finished, 0 errors), `wrapper_native` 9 of 9,
`cargo test --profile lockstep -p ti4-engine-core -p ti4-engine-core-factions -p ti4-engine-core-bridge -p ti4-engine-core-tests -p ti4-engine -p ti4-engine-legacy`
(8698 passed, 0 failed, at the tip; every item ran the tests of the crates it touched), `check_layers.py core|thin|setup` 0 violations, `check_production_graph.sh` 0 violations.
(`ti4-wasm` has an unrelated pre-existing test import error; `torch-sys` crates do not build here.)

## What was done and why it is safe

**(6)** `strategy_cards::has_infantry_site`: the same loops as `infantry_sites`, the first hit ends it. Used by `begin_paid` and the trade stage.

**(5)** `AbilityTable::{specs, forbids, card_plays}` are `Arc<Vec<_>>`; `add`, `add_forbid`, `add_card_play` use `Arc::make_mut`. A clone (five per step) is a reference count. Test
`a_clone_shares_the_specs_and_registering_copies_on_write`.

**(3) Board indexes.** `Board` keeps two derived indexes next to its chunked cells, `PlanetIndex` (placed, exhausted, `controlled[seat]`, `with_units[seat]`) and `SysIndex`
(`with_units[seat]`, `with_token[seat]`), as fixed bitsets (16 owners). Design choices:
* **Lazy, shared, dropped on write.** Built on the first query (one pass over the cells), stored as `OnceLock<Arc<_>>`: a board clone shares it (still no allocation; the state clone is
  as before, `size_of::<CoreState>` unchanged), and `planet_mut` / `sys_mut` (the only way to write a cell: the fields are private) drop the index of that half. So an index is never read
  after an edit it does not show, on every path (tile replacement, Nova Seed purge, Nexus, Mallice, tokens, fleets, codec load all write through those two methods). A maintained index
  was rejected: 615 call sites hand out `&mut PlanetCell` (`let list = &mut b.planet_mut(p).units`), so a guard type would have changed all of them; the lazy form costs one pass per
  edit-then-query (the cost of one of the scans it replaces).
* `Debug` and `==` of the board see the cells only (the final-state hash is a `Debug` of the state: unchanged).
* Queries moved to the index: `planets_of` / `placed_planets` / `planets_in`, `controlled_planets`, `controlled_count`, `systems_with_{token,units_of,units_other_than}`, `held_on_board`,
  `presence` (are_neighbours), new `systems_with_units_anywhere` / `systems_with_any_units` / `planets_with_any_units`. Loops over all systems now loop over the systems where the
  seat has a unit: `infantry_sites`, `has_infantry_site`, `sling_candidates`, Ghost `mech_destinations` and `extra_wormholes`, Empyrean `sites`, Mitosis `moves`, `space_cannon_kinds`.
* `query::tech`: `unlocked_upgrade` iterates the units of the base type (table built once, id order), `replaced_for_faction` is a table.
* **Test:** `perf_kernel` plays 13 games (3p to 6p, 11 factions) with `checks::after_step` after every step: the index equals a scan of the cells, every query above equals its
  old scan definition for every seat (every system for `planets_of` every fourth step), `space_cannon_kinds` equals `space_cannon_kinds_by_systems`. Run with
  `PERF_KERNEL_CHECK=1 perf_kernel suite 1` or `cargo test --profile lockstep -p ti4-engine-core-bridge --bin perf_kernel`. **Mutation check:** removing the invalidation in
  `planet_mut` fails it at step 10 of the first game. Plus `unlocked_upgrade_agrees_with_the_scan_of_all_units` (all scopes, bases, factions, held sets).

**(1) Lazy play facts.** `AdapterFacts::plays` is now `Plays` (reads like the `Option`: `as_ref`, `as_mut`, `is_some`, `is_none`; all 40 readers compile unchanged, the nine writers
use `Plays::of`). `table_at` -> `arm_plays` stores a clone of the position (persistent: three allocations) with the provider; the facts are computed on the first read and shared by the
table's clones. This keeps the contract "the facts of the position the call starts from" (the position changes during the call, hence the snapshot). A provider is deferred only when
the live Env says what it reads (`PlaysSource`: a live `'static` kernel, same object as the call's, and the roster); a kernel built by hand keeps the eager call. `turn::ask::roster_of`
called the whole provider for one field: it now answers from the source. **Test mode:** with the checks on, `perf_kernel` arms the facts before each step and compares them after the step
(position changed) with the eager value, and compares `provider_roster` with the provider's.

**(2) Kept `Env`.** `env_with_plays` keeps registry and table in the `Render` (`Template`) and reuses them while the `Key` holds. What the registry reads (audit of all
`Applies::Held` (5), `Applies::Faction` and `Spec::pair` (12) of the core and the faction modules): a seat's **faction**, the **leader cards it holds** (`leaders.status`: key, and the
status where a pair reads it), the **rules version**; nothing else (not board, hand, techs, laws). The key is the seat count, per seat the faction and its `(leader, status)` list, the
rules version, the kernel, the scope and a hash of the render's seat/faction texts. A new `Render` (fork, rollback of a failed step) starts empty; the code table only grows, so the
codes in the template stay valid. `exploring_dual::codes` is memoized per `Render`. **Test mode:** `env::verify_cache(true)` (set by the checks) rebuilds the `Env` from scratch at every
reuse and compares registry rows, spec slots, table codes, facts and the provider's answer on the position; it passed over all 14 suite games (14 738 steps). `env_with_plays_uncached`
is the definition.

## Final profile (`docs/perf_kernel_data/profile_instr_suite_impl.txt`, `instructions:u` every 2 M, suite, 4 092 samples in the loop)

Phase split of the suite loop (Instant probes, ms; base in brackets): advance 845 (1810), texts 135 (162), ask 60 (172), env 32 (750), env_clone 5 (15), arm 4 (5): **the rules
(`advance`) are now 76 % of the loop**, the env 3 % (was 26 %).

Top self-time (share of loop instructions): `MarkKey::cmp` 4.1 %, `SmallList::as_slice` 3.8 %, `CoreState::clone` 3.5 %, memcpy in `Interner::intern` 3.2 %, `Board::sys` 2.4 %,
`Board::planet` 1.7 %, `find` in `RulesHost::condition` 1.5 %, `SmallList::push` 1.4 %, memcpy in `run_window` 1.1 %, `MarkKey::atoms` 1.0 %, `Log::push` memcpy 1.0 %, `sorted_planets` 0.8 %.
Layers: containers (small_list, interner, set, log) 17 %, keys (mark compare) 6.8 %, board 6.6 %, state 6 %, rules::production 4.4 %, kernel (ability + resolver) 8.5 %, query::{board, fleet, tech, transit} 13 %.

## Remaining hotspots, ranked (share of current instructions)

| # | what | share | idea | risk |
|---|---|---|---|---|
| 1 | `run_reconcile` (every step, up to 8 times): `MarkMap::get` with `MarkKey::cmp` (a sorted map with a multi-field key compared by family and atoms) | 6.0 % | read the few reconcile marks once per call, or key the map by a packed integer; `sync_wormhole_tokens` also walks the board | low |
| 2 | `combat_texts` (adapter, every live call): about 100 `Render::code` interns (`Interner::intern` memcpy 6.3 % inclusive) | 8.6 % inclusive, 4.9 % own | memoize the fixed words per `Render` like the table codes; only the unit-dependent groups vary | low |
| 3 | `Driver::reask` (nested question replays the step from the start) | 17 % inclusive | inherent to the replay design; cache what the replay recomputes | high |
| 4 | conditions of `EverySeat` specs: `RulesHost::condition` + `Resolver::run_window` (154 of 186 specs are `EverySeat`, a registry row per seat) | 5 to 8 % | `Applies::Faction` for the faction-only units (changes the registry: dual and goldens needed) | medium to high |
| 5 | fleet queries: `standing_in`, `transit::activatable`, `held_on_board` / `remaining_all` per unit type (production windows) | 2.7 + 3.2 + 1.7 % | per-position cache of `held_on_board` by (seat, base); `activatable` per system pair | low to medium |
| 6 | `CoreState::clone` (about 9 per step now: `before` checkpoints, snapshots of the lazy facts, scratch for `reask`) | 3.5 % self | skip the snapshot when no reader can run (cheap precheck), share checkpoints | low |
| 7 | `deepwrought::oceans`, `has_commander_ability_in`, `payment_faces`, `sorted_planets`, `systems_with_any_units` (index rebuilt after edits: `SysIndex::of` 0.6 %) | 1 to 1.7 % each | local fixes; incremental index patch if rebuilds grow | low |
| 8 | memory: `Emit` queue drain (M1, up to 1 MB per game), `Log::push` reserve (M3) | 0 % time | see PERF_KERNEL.md | medium / very low |

Dependency note: 1, 2, 5, 6 are independent and behaviour-neutral by construction; 4 needs the heavy gates.

## Reproducing

```text
CARGO_TARGET_DIR=... CARGO_INCREMENTAL=0 cargo build --profile gate -p ti4-engine-core-bridge --bin perf_kernel --bin native_play --bin perf_ab
perf_kernel suite 1 | tail -1                                    # suite_hash 775f4136e60a99d5
PERF_KERNEL_NOSTATE=1 perf stat -e instructions:u perf_kernel game 4 1 sol      # 577.0 M at the tip (1698.0 M at a21a6a240)
PERF_KERNEL_CHECK=1 perf_kernel suite 1                          # board indexes, deferred facts and the kept Env checked at every step
perf_ab core smart <3..6> <1..6>                                 # 24 games, compare chains with a base binary
```
