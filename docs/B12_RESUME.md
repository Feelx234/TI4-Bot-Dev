# B12 resume note (combat and invasion area, item 4 + 5c of `ENGINE_PACKAGE_PREREQS.md`)

Branch `core-b12` (from `core-integration-3` @ 1e55ab28, `core-dual-shards` merged). **The area is not complete and the
heavy gate has NOT been run** (no verify_package, no golden dual, no benches, no board_equivalence, no wide dual run).
The one allowed up-front census was started (`/root/deploytmp/core-b12-agent/census.out`, queued behind other agents on
`/root/.ti4-heavy.lock`, on the binary of the merge base before any B12 change); it may have finished by now: read it.

## Why it stopped

The definition of done ("every combat/invasion window and nested reason ported or a documented, tested bail") needs, by the
prerequisites document's own estimates, 12-20 h for item 4 and 4-6 h for 5c, plus the child-window machinery that three of
the biggest remaining families share (Dynamo, Salvage Operations, the Sardakk mech, Reflective Shielding). That did not fit
one run. What fits was done and is committed; everything else is listed with its plan in `docs/ENGINE_REACTIONS.md` 10.

## Done (all tests green: `cargo test -p ti4-engine-core -p ti4-engine-core-factions -p ti4-engine`, 4228 + 142 + 93)

* Technological Singularity (6 abilities + the `Assimilate` question and its renderer), the `combat::settle` silent bail on
  `nekro:ts:` marks, The Terror Between registered for Nekro seats, Instinct Training carried by an assimilator, Sh'val's
  finish. Details, ids and tests: `docs/ENGINE_REACTIONS.md` 10.1.
* Bail tests: `reactions_core::tests::bail_every_family_that_stays_in_the_string_is_named` and the specific ones in 10.2.
* Mutation checks: see the final report of the run (listed per core half; one equivalent mutant: `is_unit_upgrade(t)` in
  `changes_unit_forms` is covered by the `Unit::ALL` clause, kept as a guard).

## Next (in this order; every item = Spec + Named line + lockstep test with real factions and `promissory` notes cleared
(trap: fixture faction `generic`) + mutation check + registry parity test)

1. **Payment helper inside an ability** (`Eff` runs `PayStep` through `Eff::ask`; mirror `production::pay_offering_agent`).
   Unlocks Empyrean Dynamo (15) and Salvage Operations' produce half (8, needs a `produce` relation) and probably Bribery.
   Dynamo also needs the raw map adjacency as an `AdapterFacts` entry (the string uses `galaxy.adjacent`, not
   `PlayerAdjacency`).
2. **Ssruu pair registry** (46 runs): `Applies::Borrowed(agent)` registered per (borrower, source) pair, `slot_of` on
   `{source}:yssarilagent:{agent}`; effects are the originals'. Port the family whole (a window bails if any eligible
   ability of its signature is outside the table).
3. Empyrean Watcher (7), Sardakk mech (child hit assignment), Reflective Shielding (design in `ENGINE_REACTIONS.md` 9.2),
   Courageous, Fire Team, Scramble Frequency, Reveal Prototype.
4. 5c Bastion/Crimson typed reads: see `ENGINE_REACTIONS.md` 10.3 (hooks declared, not implemented; `Spot` has no card
   variant; `fleet::remaining(.., on_card)` always 0 in the core).
5. 5a `FinalizingControl` and 5b `finish_committing`: unblocked now (B5/B7 are in `core-integration-3`), not started.
6. Then the gate, once, on the final commit, under `flock /root/.ti4-heavy.lock`: `tools/engine_core/verify_package.sh`
   for the combat and invasion areas (`SKIP_AB=1 SKIP_WASM=1`, `--profile gate`, `SHARDS=4`), the golden dual with
   `driver=dual` (and the wide mask `driver=dual,invasion=dual,production=dual,leaders=dual,exploration=dual,agenda=dual`
   with `TI4_NESTED_LOG=1` for the census), `board_equivalence`, `bench` + `bench-v1..v5` 200/200, clippy.

## Commands

```
git worktree: /root/TI4-Bot-Dev/.claude/worktrees/core-b12 (branch core-b12)
export CARGO_TARGET_DIR=/root/deploytmp/core-b12 CARGO_BUILD_JOBS=2
cargo test -p ti4-engine --lib singularity          # the Nekro tests
/root/deploytmp/core-b12-agent/mutate.py FILE FILTER 'old=>new' ...   # one mutation at a time, expects a failure
```
Open decision: none for the user. Remove `/root/deploytmp/core-b12` (debug 3-5 GB, gate 1.5 GB) when done.
