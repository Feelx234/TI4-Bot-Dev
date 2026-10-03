# Base-faction seated soak — Codex, 2026-10-03

The new-file-only harness is `crates/ti4-sim/examples/base_faction_soak.rs` (commit `fb730a8f`). It seats each target in one of six rotating positions against five of the six original in-scope factions, deploys faction assets through the seating API, uses `DEFAULT` content including Thunder's Edge, seed-shuffles 30 filler tiles with `map_filler`, and runs seeded-random choices. A game fails on setup/deploy error, `Game::run` error, 25,000-step limit, unfinished 50-round horizon, or a mismatch when the same seed is replayed. Replay compares final serialized state, ordered event labels, and every decision record. Failures print faction, seed, last/pending decision or first divergent replay choices. The default command is the only output labeled `FULL 12x200 SOAK`; individual/sliced runs are explicitly diagnostic.

Independent Terra review identified three issues in the first draft: short slices could look like full acceptance, a fixed map reduced coverage, and replay mismatch diagnostics lacked the divergent records. All three were corrected before `fb730a8f`. The reviewed version passed `cargo check -p ti4-sim --example base_faction_soak -j 1` on clean committed head `e2b7d484`. A one-seed release smoke also passed on an earlier committed head. The first ordinary debug build failed with rustc memory exhaustion; a low-debug build completed, so that failure was environmental rather than a game failure.

## Clean committed-tree results at e2b7d484

The existing detached checkout `.worktrees/m03-015-timing-properties` was used. Its source was exactly committed `e2b7d484` plus the identical new harness file; Claude's shared uncommitted `game.rs` income experiment and later edits were excluded. The release binary was built with absolute `LIBTORCH=D:\Projects\ti4-engine-rs\out\libtorch-2.9.1-cpu`.

| Faction | Command suffix | Games | Engine/replay/horizon failures |
|---|---|---:|---:|
| Arborec | `arborec 0 200` | 200 | 0 |
| Argent | `argent 0 200` | 200 | 0 |
| Ghost | `ghost 0 200` | 200 | 0 |
| Mentak | `mentak 0 200` | 200 | 0 |
| Muaat | `muaat 0 200` | 200 | 0 |
| Naalu | `naalu 0 200` | 200 | 0 |
| Naaz-Rokha | `naaz 0 200` | 200 | 0 |
| Saar | `saar 0 200` | 200 | 0 |
| Sardakk | `sardakk 0 200` | 200 | 0 |
| Winnu | `winnu 0 200` | 200 | 0 |
| Yin | `yin 0 200` | 200 | 0 |
| Yssaril | `yssaril 0 200` | 200 | 0 |

These twelve individual runs cover the same 12 x 200 faction/seed cases as the default full campaign: 2,400 games and 2,400 deterministic replays, with zero engine errors, unfinished horizons, or replay differences. They are a diagnostic at e2b7d484, not a BF exit gate: that snapshot has unclaimed assets and Claude is still changing engine/faction code. The ten-seed smoke checks preceded the full runs and are subsumed by them.

The independent ledger command `cargo test -p ti4-engine --lib print_faction_ledger -j 1 -- --ignored --nocapture` passed on this snapshot: 132/145 claimed, with Arborec, Ghost, Mentak and Sardakk at 12/12. The full `cargo test -p ti4-engine -q --no-fail-fast -j 1` also passed: 1,914 library tests, one ignored, integration binaries 1/1, 4/4 and 5/5, and doctests. Later Claude commits claim more assets; their source was not part of this soak.
