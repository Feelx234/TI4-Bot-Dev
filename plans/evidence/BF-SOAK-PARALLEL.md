# P1: bounded parallel base-faction soak execution

## Scope

Parallelize the existing base-faction soak at the seed-case level. Each indexed job runs one faction/seed pair, executing two paired runs sequentially and comparing the same complete serialized game state, event sequence, and decision records from two consecutive runs of that same faction/seed case. Each run pair uses the same substituted-faction assignment; this is a replay-determinism check, not a comparison against the original six-faction assignment. The set of factions, seed range, CLI arguments, count/range validation, and 12 × 200 campaign label remain unchanged.

Use a bounded worker pool of at most 16 workers with dynamic indexed jobs. Retain only compact per-case outcomes or failure strings after each case; do not retain the full replay results for all 4,800 runs. Emit live progress at completed-case intervals, while sorting final failures and counters by canonical faction/seed order. Worker panics and case errors must remain visible and must not silently drop a job.

## Acceptance checks

- Existing CLI and validation behavior remains unchanged.
- Every faction/seed case is scheduled exactly once; its paired runs remain sequential.
- Exact existing replay equality checks remain in force.
- Worker count is bounded at 16 and jobs are dynamically claimed.
- Final diagnostics and aggregate counters are deterministic in canonical order.
- Case errors and panics surface as failures; no worker panic loses a case silently.
- The full report continues to identify the unchanged 12 × 200 one-seat-substitution campaign; no broader all-pairs claim is introduced.

## Limits

This change makes no performance claim. The root reviewer will run a small smoke check and estimate the full campaign before deciding when to launch the full soak.



## Soak reduction, 2026-10-05 (operator)

Operator approved: `--release` always; default 25 seeds per faction; replay every 10th case (`replay_every`, 4th argument; 1 = every case); 200 seeds only at the exit gate (`all 0 200 10`); BF-25 pairing soak sampled. The 12x200 debug run started before this decision used replay on every case; its result is recorded when it completes.

## Result: 12x200 soak, 2026-10-05 (Claude coordinator)

`cargo run -p ti4-sim --example base_faction_soak -q -j4` (debug profile, no args = 12x200, every case replayed), 16 case workers, full log `target/bf-claude-soak-full.log`. Every faction 200/200 games, 0 failures; total 2,400 cases (4,800 games), exit 0, 2,321.9 s. Built from the shared tree as of HEAD d159edd8 plus its uncommitted BF work, before this session's Oct 5 commander/Alliance edits (those are not covered by this run). Smoke slice before it: `naaz 0 4`, 4/4, 0 failures.
