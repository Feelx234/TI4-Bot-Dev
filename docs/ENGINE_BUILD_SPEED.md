# Engine build speed: the `gate` profile

Release builds of the gate binaries (`selfplay`, the six `ti4-engine-core-bridge` bins) took
about 15 minutes at `CARGO_BUILD_JOBS=3`. Cause (cargo `--timings`): `[profile.release]` has
`lto = "thin"` and `codegen-units = 1`, and every bridge bin (`decision_corpus`, `state_size`,
`core_sync_gate`, `core_equivalence`, `core_movement_bench`, `dual_run`) links the whole engine,
host, wasm and selfplay stack, so each one repeats a 180-270 s single-threaded LTO codegen
(about 1370 s CPU of 1220 s user time in total). Merging the bins would not help: the cost is the
per-binary LTO, not linking. No mold, lld or sccache is installed (not installed by this change).

## The profile

`[profile.gate]` in `Cargo.toml`: inherits release; `lto = "off"`, `codegen-units = 16`,
`debug = 0`, `incremental = true`, `opt-level = 3`. Output goes to `$CARGO_TARGET_DIR/gate/`.

## Commands for agents

```
export CARGO_TARGET_DIR=<private dir> CARGO_BUILD_JOBS=3
cargo build --profile gate -p ti4-selfplay -p ti4-engine-core-bridge --bins
$CARGO_TARGET_DIR/gate/dual_run tools/engine_profile/profile-20.json 20 movement=dual
tools/engine_profile/verify_benches.sh $CARGO_TARGET_DIR/gate/selfplay 3
```

`tools/engine_core/verify_package.sh` builds and runs everything with `gate`. The CPU A/B and the
wasm size gate are off by default (`RUN_AB=1`, `RUN_WASM=1` bring them back; they need
release/wasm-release builds and a release baseline). Timing claims (CPU A/B) must still be taken
with release builds.

## Measured (3 jobs, shared machine, clean target dir)

| | release | gate |
|---|---|---|
| selfplay + 6 bridge bins, clean | 14 m 55 s | 2 m 54 s |
| six benches (verify_benches.sh, 3 workers), CPU | 1.2k s | 2.5k s (about 2.1x) |

Cost: gate binaries run slower (no cross-crate inlining): `dual_run` on the 20 golden games took
4 m 32 s instead of 2 m 19 s wall. Decision hashes and bench results are identical (all six benches `200 identical, 0 different` against `baseline*.json` with both profiles; dual_run output identical).

Rule of thumb: the gate profile wins for one verification pass (build 12 min shorter, run 6 min
longer) and for every rebuild after a code change (incremental). For a long campaign of benches on one
unchanged commit, or any CPU timing, build release once.
