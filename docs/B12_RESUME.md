# B12 resume note (combat and invasion area, item 4 + 5 of `ENGINE_PACKAGE_PREREQS.md`)

Branch `core-b12` (from `core-integration-3` @ 1e55ab28; `core-dual-shards` and `core-b8c` @ 8fa4ddb2 merged). **The area is not
complete and the heavy gate has NOT been run** (no verify_package, no golden dual after the changes, no benches, no board_equivalence,
no clippy). The single up-front census ran (pre-B12 binary): `docs/ENGINE_REACTIONS.md` 10.4.

## Done (all tests green: `cargo test -p ti4-engine-core -p ti4-engine-core-factions -p ti4-engine`)

Round 1: Technological Singularity (6 abilities, the `Assimilate` question), the `combat::settle` silent bail on `nekro:ts:` marks,
The Terror Between registered for Nekro seats, Instinct Training carried by an assimilator, Sh'val's finish.
Round 2 (this note's session): `pay_in` (a payment loop inside an ability), the map pairs as an input, Dynamo, the Watcher, Salvage
Operations' produce half, pair rules and the four Ssruu copies (Milor, Evelyn, Viscount, Trilossa), the round agents' extra die read from
the core state. Ids, slots, tests: `docs/ENGINE_REACTIONS.md` 10.1 and 10.1b; bails: 10.2.

## Next, in this order (each = Spec + Named line + lockstep test with real factions, notes cleared (trap: fixture faction `generic`)
## + mutation check + registry parity test with the seats seated)

1. **Mutation survivors (documented, not closed)**: glue with no observable effect in the tested windows: Dynamo's flagship owner
   filter (two Empyrean seats), `pay_in`'s up-front `available < cost` guard (the conditions guard it), the restore of the state after
   a failed payment/placement of Salvage Operations (needs a bill that fails after the question), `when_by`'s `lent` flag of the Argent
   copy and the `borrowable` call in `round_when`/`milor_copy` (the pure `borrowable` is tested; the glue is covered by the positive
   lockstep tests only), `extra_die_for`'s type filter (equivalent to the mark's own type field). The helper is
   `/root/deploytmp/core-b12-agent/mutate.py` (`MUT_ROOT`/`MUT_TARGET` run it in a second worktree so development and mutation do not
   share files; filters are `a|b` substrings, one `cargo test` per mutation).
2. **5b `finish_committing` remainder**: `offer_research_team` (+ `_defense`) and `add_forced_combats` are string functions between the
   windows already ported (`announce_and_defend_windowed`, `advance_fighting`). The queries exist in the core
   (`deepwrought_research_team_open`, `_defender_open`, hook `forced_combat_planets`); the coexistence write (`begin_coexisting`) is
   `state/board.rs` + the Deepwrought module. Minimal DoD: two dual windows that compare the "nothing offered" case and bail
   (named) on a Deepwrought, Firmament or Titans seat.
3. **5a `FinalizingControl`** (`invasion.rs:5014 finish_control_gain`): `fx::gain_control` (+ the `control_gained` hook, L1Z1X
   Assimilate), `PLANET_CONTROL_GAINED` through the resolver (Infiltrate/Reparations bail: `Unknown::HarnessOrInfiltrate`), the
   structure destruction order by `RULES_INFILTRATE_FIX`, Integrated Economy (`EconomyWindow`), Jupiter Brain, Fracture relic, exploration
   (B5 windows). Minimal DoD: a capture with none of those compared as one window; the rest bails.
4. **5c Bastion/Crimson typed reads**: `ENGINE_REACTIONS.md` 10.3 (hooks declared in `kernel/hooks/direct.rs:78-127`, implemented nowhere;
   `Spot` has no card variant; `fleet::remaining(.., on_card)` is always 0 in the core).
5. Cards: Fire Team, Scramble Frequency (a `RerollSource::Card`), Courageous to the End, Reflective Shielding (`Child<W>`), Reveal Prototype
   (the research as a callable piece; `cost_is_payable` is `None` for it), Sardakk mech (a child hit assignment), Technological
   Singularity's unit-upgrade gain (a core `unit_swaps`), Winnu commander unlock, Ambuscade.
6. Then the gate, once, on the final commit, under `flock /root/.ti4-heavy.lock`: `tools/engine_core/verify_package.sh` for the combat and
   invasion areas (`SKIP_AB=1 SKIP_WASM=1`, `--profile gate`, `SHARDS=4`), the golden dual with `driver=dual` (and the wide mask
   `driver=dual,invasion=dual,production=dual,leaders=dual,exploration=dual,agenda=dual`), `board_equivalence`, `bench` + `bench-v1..v5`
   200/200, clippy. Expect new golden findings: nothing of round 2 has seen the 20 golden games yet; the typical finds so far were a
   fixture-only blind spot (the oracle snapshot), a `context.optional` flag and an id shape.

## Commands

```
worktree: /root/TI4-Bot-Dev/.claude/worktrees/core-b12 (branch core-b12); mutation worktree core-b12-mut (detached)
export CARGO_TARGET_DIR=/root/deploytmp/core-b12 CARGO_BUILD_JOBS=3
cargo test -p ti4-engine --lib -- ssruu salvage dynamo watcher singularity
```
Housekeeping: the private targets were deleted at the end of the session (rebuild: about 4 minutes with `CARGO_BUILD_JOBS=3`); the
scratch directory `/root/deploytmp/core-b12-agent` keeps the scripts and the census.
