# Botstops resume (lane `core-botstops-1`)

Branch `core-botstops-1`, cut from `core-perf-ab` (4ecad8ab1, on core-integration-23). Not pushed, not deployed. RULES_VERSION 35, wire unchanged.

## Result

With the shipped rule bot reading the view (`perf_ab core smart <players> <seed>`, 3p/4p/5p/6p x seeds 1-6) **all 24 core games now play to the end, 0 step errors, 0 bails**
(before: 8 stopped). The 16 games that already worked have the identical decision chain (hash of every (player, prompt, answer)) as in
`docs/perf_ab_data/after.jsonl`. The quick native game is unchanged: 1165 steps, fingerprint 74d5675ac754ac81 (`native_play --quick`: ZERO GAP).

## The 8 stops: three root causes

| games | error | root cause | fix |
|---|---|---|---|
| 4p4, 5p4, 4p5 (second stop), 5p5 (second stop), 6p6 | `aftermath: MissingFact` | Salvage Operations (Mentak, produce half after a won space combat) read the units its owner may build from `AdapterFacts::buildable`, which only the string side's adapter fills (`reactions_core.rs` of the legacy crate, `set_buildable`). A native table lends none, so the first Mentak win with the technology was "unknown" and the core stopped. It needs a Mentak seat with the technology that wins a space combat; the `native_play` sweeps did not reach one. | `cc_ground/mentak/timing.rs::salvage_options`: when no list is lent, read it from the position with the core's own `rules::production::home::buildable_for` (the rule `production::buildable_for` ports: same bases, same order, same form overrides). Still unknown when no payment kernel is lent. |
| 4p5 (first stop) | `tactical: UnportedTactical(NovaSeedCargo)` | A counted gap: Nova Seed over another player's ground forces carried in the space area. The string stages their destruction with the SYSTEM id in the planet's place (`muaat::destroy_others`), and the core had no way to stage it. | `fx_ground::stage_destroyed_in_space` stages the typed row with the system atom in the planet slot (the staged-ground codec shape for it already existed in `core_sync/keys/mark_values.rs`); `driver/agenda/ground.rs::staged` announces it with `Key::Planet = Val::System(system)` (renders as the string's text); `muaat::destroy_others` stages the cargo of every other seat after the planets, in list order, then clears the space area, as the string; the condition no longer bails. `TacticalGap::NovaSeedCargo` and its two texts are removed. The string-to-core event converter (`kernel_sync/convert.rs`, both crates) accepts a system id where a planet is expected, so the dual no longer drops that event. Consumers of the event were checked: Yin agent/Milor and Devour need the ground-combat cause, Reanimator and the Nekro mark need a real planet (as the string, where the system id names no planet), the Terror Between needs only the system. |
| 6p3, 6p4, 6p5, 5p5 (first stop), 4p5 | `the position cannot be shown: Board: planet entry in an untouched system "atlas"` (also "hopesend", "centauri") | After Nova Seed the replaced tile's planets kept the `PURGED` flag (set by `purge_planet`) while the tile's board entry was gone (`replace_tile` takes the cell). The string removes the WHOLE board entry (`state.board.remove(old)`, its `purged_planets` with it) and only `purged_systems` keeps the tile. The position then could not be mirrored back into the string-shaped state. | `muaat/timing.rs::replace_tile` clears the `PURGED` flag of the old tile's planets once the tile is replaced. |

(4p seed 5 and 5p seed 5 hit two causes in sequence; each was only visible after the earlier one was fixed.)

## Tests (each fails without its fix)

* `ti4-engine-core-factions` lib: `ce_movement::muaat::timing::tests::a_replaced_tile_leaves_no_planet_entry_behind_and_the_position_still_shows`,
  `cc_ground::mentak::timing::tests::salvage_reads_the_buildable_list_from_the_position_when_no_adapter_lends_one` (checked red with the fix removed).
* `ti4-engine-legacy` lib, `combat_live::tests::nova_seed_replaces_the_tile_in_lockstep` now also runs the cargo cases (another player's carrier plus two infantry in the space
  area, with and without rich planets, hero used and declined) against the string run: state, questions and events equal (a bail counts as a failure there; it was the bail before).
* `ti4-engine-core-bridge` test `bot_games_native` (new): the smart bot plays 5p seed 4, 4p seed 5 and 5p seed 5 to the end through the `Game` wrapper without a step error
  (15 s in the lockstep profile).
* Run with `cargo test --profile lockstep -p <crate> ...` (`CARGO_INCREMENTAL=0`, private target dir). No Z-gate, ratchet or golden dual was run (brief).

## Divergence from the string

The smart games diverge from the string engine in most seeds already (docs/PERF_AB.md; S1-S5 of docs/STRING_ENGINE_BUGS.md); I did not diff the 8 whole games against the string.
The three fixes are each checked against the string on their own: the Nova Seed lockstep compares the whole position, and the planet-flag fix reproduces what the string's board removal leaves.

## Left / not done

* Nothing from the 8 stops. Not run: the heavy gates, the 288-game shadow, a dual of the 8 games (the string finishes them differently, see above).
* A native table still lends no `buildable` list (the rule now reads the position); any other rule that reads a fact only the string adapter fills in `AdapterFacts` stops a native game the same way (`Unknown::MissingFact`
  sites listed by `grep -rn "Unknown::MissingFact" crates`); the 24-game sweep does not reach any of them. A wider bot sweep (more seeds, other faction seats via `perf_ab core smart <p> <seed> <faction>`) is the cheap way to find the next one.
* `docs/PERF_AB.md` still lists the 8 stops in its "Core stops" section; this file supersedes it.
