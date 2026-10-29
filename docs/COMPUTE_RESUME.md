# Compute branch: what is left (read this, then delete it)

Branch `core-compute` is complete except for measurements that need the heavy lock (held for hours by other gates
during this work) and about 3 GB of disk (the disk was at 98-100 %). Queued detached jobs (started with `setsid`):

* `/root/deploytmp/core-compute/engine_meas.sh` waits for `/root/.ti4-heavy.lock`, then runs
  `compute_bench.sh profiles` (gate, lockstep, lockstep0 on `ti4-engine --lib`) and `compute_bench.sh seed`
  (cold warm-dir build vs seeded builds) in the scratch worktree `/root/deploytmp/cc-meas2`. Output:
  `/root/deploytmp/core-compute/engine.out` (`RESULT` lines; ends with a `done` line, or a `WATCHDOG` line when
  less than 600 MB of disk were free).
* `/root/deploytmp/core-compute/bench_full.sh` then runs `verify_benches.sh` with JOBS=1 and JOBS=3 (200 games per bench)
  with `/root/deploytmp/core-b12-gate/driver/selfplay-new`. Output: `benchfull.out`.

To finish: copy the numbers into the `PENDING` paragraphs of `docs/ENGINE_COMPUTE.md` (sections 2, 3, 4), then
`git worktree remove --force /root/deploytmp/cc-meas2` and `rm -rf /root/deploytmp/core-compute`. To cancel the jobs:
`kill` the `engine_meas.sh` and `bench_full.sh` processes (`ps -eo pid,cmd | grep -E 'engine_meas|bench_full'`).
