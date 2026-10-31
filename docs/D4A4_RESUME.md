# D4A4 resume (lane A4 of D4: the covers gate `coverage.py --check`)

Branch `core-d4a4` (worktree `.claude/worktrees/core-d4a4`), cut from `core-eod1-c` @ `4ef8cb39`; `core-integration-8` @ `14593a23` merged (one conflict, `core_sync/keys/mark_values.rs`: both
destroyed-mark shapes kept, the space-area ground force and the galvanized one). **No production code changed.** Changed: tests (`crates/ti4-engine-core-tests`), the kit's `Rig`
(`src/rig.rs`), `tools/engine_core/d4/coverage.py`, `tools/engine_core/d4/covers_exempt.txt`, docs.

## Result of the gate step

| | before (d4gate.log) | after |
|---|---|---|
| windows covered | 89 / 124 | 120 / 123 (3 exempt, 0 uncovered) |
| areas below 80 % stage variants | 22 | 0 |
| `coverage.py --check` | FAIL | exit 0 |
| tests of `ti4-engine-core-tests` | 1105 (lane A3) / 1172 at the start of this lane | 1263 pass, 0 fail |

(The 124 -> 123: `rules::reactions::Emit` was a test fixture counted as a window, see D4A4-info-02.)

## What changed besides tests

* `Rig` (kit) registers the stage a window is in when the driver first steps it (`Start`), in addition to the stage after each step. Before, `Start` of every window was untraceable.
* `coverage.py`: a `#[cfg(test)] pub(crate) mod` is a fixture (like `pub mod`); the stage variants of an exempt window do not count toward its area.
* `covers_exempt.txt`: `DiplomacyWindow` (structured diplomacy, XF-1), `AgentOffer` (Keleres), `PlaceCard` (Firmament). All three are `unsupported::Disabled` features. Nothing else is exempt.

## Tests added (`crates/ti4-engine-core-tests/tests/`)

| file | tests | covers |
|---|---:|---|
| `driver_windows.rs` | 11 | `Head`, `FightOver` (aftermath), `EnforceAll`, `MinisterOfWar` |
| `action_cards_announce.rs` | 7 | `Announce` (simple / asking / unresolved / not announced), `DrawCards` (limit, name groups, no groups) |
| `technology_free_windows.rs` | 16 | Psychoarchaeology, Production Biomes, Vortex, Wormhole Generator, `TurnEnd` (Predictive Intelligence, Bio-Stims), `ProductionUsed` (Sarween, AI Development Algorithm), Transit Diodes, `TurnStart` |
| `promissory_windows.rs` | 11 | `PlaceUnits` (Military Support, the Nekro PDS), Stall Tactics, Mageon Implants, `C4rDraw` |
| `leader_windows_agents.rs` | 8 | Gurno Aggero, Riftwalker Meian, Nekro Malleon, Letani Ospha |
| `legendary.rs` | 7 | `Legendary` (every ability; end of turn and pass) |
| `faction_components.rs` | 7 | Orbital Drop, Star Forge and The Nucleus with Ember Colossus |
| `windows_misc.rs` | 14 | Fabrication / Black Market Forgery, `TitanBuild`, `PayStep`, Self-Assembly, `ControlGained` (+ Assimilate), Imperial secondary, `Destroy`, `ProduceHits` |
| `agenda_draws.rs` | 2 | Politics Rider and Unconventional Measures over the hand limit |
| `action_card_probe_limit.rs` | 2 | Impersonation over the secret limit, Probe on a Merchant Station |
| `combat_machine.rs` (+3), `combat_barrage.rs` (+1) | 4 | `Machine` in Rolling / Producing / Barraging, Raid Formation inside `BarrageStep` |

Expectations come from the printed card text (in the file headers); the snapshots under `tests/snapshots/` were blessed once (`BLESS=1`) and read.

## What was run, what was not

* Run: `cargo test -p ti4-engine-core-tests --no-fail-fast` in profile `lockstep` with debug assertions and overflow checks on for the three core crates (the flags of `tools/engine_core/d4/run_core_tests.sh`),
  `CARGO_BUILD_JOBS=2`, private target dir and private covers file under `/root/deploytmp/core-d4a4` (deleted at the end); `coverage.py --check` on that covers file.
* Mutation (cheap, Haiku runner on a committed snapshot in a scratch worktree, `D4_COVERS_FILE=/dev/null`): see the report of the lane; numbers are in the final message, logs in `/root/deploytmp/core-d4a4/mut-*.log` (deleted).
* Not run (heavy-phase rule): golden or wide duals, benches, soak, `mutation_ratchet.sh`, workspace clippy, `verify_package.sh`, `d4_gate.sh` as a whole.

## How to resume

1. `git log --oneline core-eod1-c..core-d4a4`; run `tools/engine_core/d4/run_core_tests.sh $(git rev-parse HEAD)` (it keeps its own covers file; `rm` it afterwards, it grows by ~100k lines per run).
2. `python3 tools/engine_core/d4/coverage.py --covers <that covers file> --check` must exit 0.
3. Still weak: stage variants that are never a suspension point (D4A4-info-01) cannot be traced; mutation survivors of the new files are listed in the final report.
