# D4A resume (lane A of D4: D4-0, D4-1, D4-2a/2b/2c, D4-3)

Status file, rewritten at each stop. Branch `core-d4a` (from `core-integration-4` @ `42e69e3c`), worktree `.claude/worktrees/core-d4a`.

## What exists

* New test-only crate `crates/ti4-engine-core-tests` (kit in `src/`, tests in `tests/`), `wrapper` feature for `tests/wrapper_props.rs`.
* Kit: `Scenario` (typed builder, real factions only, permutable construction order), `Rig` (driver wrapper, transcript, replay, option probe),
  `smoke` (four answer policies + contract + conservation + determinism + snapshot), `delta`, `snap` (`BLESS=<text>`), `inv`, `generators`,
  `props` (P1-P6), `covers`.
* `tools/engine_core/d4/run_core_tests.sh <commit> [BLESS=<text>] [-- filters]`: the runner script (one build pass that reports every compile
  error, then the tests; waits while `/root/.ti4-heavy.lock` is held).

## How to resume

1. `git log --oneline core-integration-4..core-d4a` shows the WIP commits; the newest is the resume point.
2. Run the tests of the newest commit through a runner: `tools/engine_core/d4/run_core_tests.sh $(git rev-parse HEAD)`; fix compile errors and
   failing expectations (a failure is a wrong expectation or a core bug: decide with the rules text; for a core bug write the regression test,
   fix only when local, add a `D4-found-NN` row to `docs/ENGINE_KNOWN_ISSUES.md` of the `core-integration` worktree).
3. Bless the snapshots a new test names: `... BLESS=<area>` (e.g. `BLESS=tactical`), review `git diff` of `tests/snapshots/` against the rules.
4. Delete `/root/deploytmp/core-d4a`, `core-d4a-wt`, `core-d4a-logs` when done.

## Not done / open

See the final report of the agent (SubagentHandback) and the list below, kept current.
