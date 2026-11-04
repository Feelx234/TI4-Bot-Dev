# Heavy run plan (prepared 2026-11-04 by lane core-heavy-prep)

Base: `core-integration-23` (6d66acab5; deployed 54a15dba). Nothing in this document was run except the cheap
checks named as "measured"; no heavy step has been started. Start only on the user's go, on a quiet machine (the perf A/B
lane must be finished), and never in the hour before a nightly sweep (20:30 Berlin; the nightly cron is currently
disabled, restore at 22:30 Berlin is a separate task; check `TZ=Europe/Berlin date`).

## 0. Decisions in force (user, 2026-11-04)

* The heavy run may start (zero-bail playthrough and Z-gate are green).
* Mutation floors (`core/driver` 90, `factions/cards` 80, `factions/top` 60) MAY be lowered if a module cannot reach them. Every
  lowering is recorded in section 7 and in the commit message (module, old, new, measured score, reason, survivors left).
  Never lower silently; `UPDATE=1` still only raises.
* The `alpha` step is skipped with a recorded reason (all 14 alpha saves are rules version 0 and refused).
* Old saves need no recovery. The nightly cron comes back later (not part of this run).

## 1. State of the preparation (measured today)

| item | result |
|---|---|
| disk | 38 GB free at start; 127 GB free now (301 GB disk, 57 % used). Freed about 90 GB, see section 8 |
| wasm (cargo `wasm-release` + binaryen 123 via `web-shared/scripts/build-wasm.mjs`) | 6 432 131 raw / 2 041 438 gzip / 1 469 073 brotli; old budget 7.0 / 2.4 / 1.8 MB: within; `size-budget.json` ratcheted to 6.63 / 2.11 / 1.52 MB (real +3 %); wasm-opt 7.83 -> 6.43 MB; build 2 min 17 s, 1.94 GB RSS |
| `lint` step | base retargeted (section 4); passes in 1 s; negative control (base = live tip `nightly-fixes-2026-10-26`) fails as it must |
| `alpha` step | skipped by default (`ALPHA_SKIP=1`), logged `SKIPPED: ...`, shown as `skip alpha` in `summary.txt` |
| bench baselines | re-record plan and flag in section 5 (`BENCH_RECORD=1`); not run |
| `end_of_d1.sh --dry-run` | 25 steps, ids below; ok |
| locks | `/root/.ti4-heavy.lock` and `.lock.2` both free at 20:03 Berlin (probe with `flock -n`) |

wasm-opt: it is the `binaryen` devDependency of `web/` (123.0.0); `cd web && npm ci --ignore-scripts` installs it, and
`build-wasm.mjs` finds `web/node_modules/.bin/wasm-opt` by itself. It is installed in the worktree `core-heavy-prep` only (any
fresh worktree needs the same `npm ci`; `deploy.sh` runs it). Without it the raw size is 7.83 MB, over the old 7.0 MB budget.

## 2. The base ref of the `lint` step

`lint` runs `lint_core.sh` and `git diff --exit-code <base> -- web/ crates/ti4-model/src/state.rs ':!docs/RULES_VERSION.md'`
(plus the "only the refusal note may change" rule for `docs/RULES_VERSION.md`; this is the wording of G7 row 2 in
`docs/D29A_RESUME.md`). The old default base `core-integration-6` is hopelessly stale (42 files of `web/` differ), and the
live tip `nightly-fixes-2026-10-26` cannot be the base either: the P1 commits of the flip changed `web/` (50 files vs the live
tip) and `state.rs` (+65 lines) on purpose. Decision: the base is the immutable surface of the deployed int-23, the local tag
**`deployed-int23-54a15dba`** (commit 54a15dba; the tip of int-23 differs from it only in `docs/`). Fallbacks: branch
`core-integration-23`, then `origin/core-integration-23`; `--base-ref REF` / `BASE_REF` override. Meaning: after the deployed
int-23 nobody may touch `web/` or `state.rs` through the heavy-run tip (a fix that must, needs `--base-ref` and a note).
The tag is local (not pushed).

## 3. Before the go: the exact preflight (cheap)

```sh
cd /root/TI4-Bot-Dev/.claude/worktrees/<candidate worktree>      # clean tree at the tip to be tested
TZ=Europe/Berlin date; df -h /root | tail -1                      # >= 60 GB free
for f in /root/.ti4-heavy.lock /root/.ti4-heavy.lock.2; do flock -n $f true && echo free $f || echo BUSY $f; done
uptime; ps aux | grep -E 'cargo|rustc|perf_ab' | grep -v grep     # nothing heavy running
tools/engine_core/end_of_d1.sh --dry-run                          # 25 steps
```

## 4. Bench baselines at rules version 35 (do this BEFORE run A, or let run A do it with `BENCH_RECORD=1`)

What the benches are: `tools/rulebot/selfplay.sh bench, bench-v1 ... bench-v5` = 6 configs x 200 games (4p and 6p, seeds 1..100
on two maps each), rule bots version 4 (`bench`, golden.json), 5, 4, 1, 2, 3, compared hash by hash with
`tools/rulebot/baseline{,-v1..v5}.json`. The configs pin the rules version: golden.json 23, golden-v5.json 18, golden-v2.json and
golden-v4.json 11. The native engine only plays 35, so the stored baselines can no longer be verified; they must be re-recorded
by the frozen string engine at 35, then the native selfplay must reproduce them 200/200 (this is precondition 10 of the FLIP
runbook and R5 of D2-8). The `completion*` baselines are another tool and are not touched.

The frozen recorder is a `selfplay` built at **`core-integration-20`** (the last line before the atomic swap of I21; there
`ti4-engine` is still the string engine, `RULES_VERSION` is 35).

```sh
# 1. recorder (one release-class build, selfplay only; about 10-15 min at 2 jobs; private target ~6-8 GB, delete afterwards)
git worktree add --detach /root/deploytmp/core-rec core-integration-20
cd /root/deploytmp/core-rec && CARGO_TARGET_DIR=/root/deploytmp/core-rec-target CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 \
  cargo build --profile gate -p ti4-selfplay --bin selfplay
REC=/root/deploytmp/core-rec-target/gate/selfplay
# 2. record (in the candidate worktree): sets rules_version 35 in the 4 configs, writes the 6 baselines; JOBS=3 benches at a time,
#    6 workers in total; expected 15-30 min wall (same cost as one verify pass, plus game-play on the string engine)
cd <candidate worktree> && tools/engine_profile/record_benches.sh $REC 6 35
# 3. verify with the native binary (the step bench-off of the run does this); then commit configs + baselines in ONE commit:
git add tools/rulebot && git commit -m "bench baselines re-recorded at rules version 35 by the string engine (core-integration-20)"
```

Alternatively one command: `BENCH_RECORD=1 BENCH_RECORD_BIN=$REC tools/engine_core/end_of_d1.sh ...`; the step `bench-off`
then records first (`bench-record.txt` in the out dir), then verifies the fresh `selfplay-new` against the new baselines; the other
three bench steps (`TI4_CHURN` resolver / state / both) only verify. In that case run `bench-off` before the long steps if
the baselines are wanted early: `--only build bench-off`. The new files stay uncommitted in the worktree: commit them
before the tag of the run. Expected: the native engine reproduces them 200/200 x6. A difference is a core-vs-string bug
(first debug with `selfplay replay`), never a reason to edit hashes (R5).
Reasons a hash may legitimately differ: none planned. The plain `selfplay.sh bench` of int-23 is red until this is done
(rules version 23 cannot be played natively).

## 5. Run A: everything but the ratchet

```sh
cd <candidate worktree>
export CARGO_TARGET_DIR=/root/deploytmp/eod CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
OUT=/root/end-of-d1-$(date -u +%Y%m%d)
ZERO_BAIL=enforce SHARDS=3 WORKERS=6 GAMES=20 SOAK_GAMES=40 \
  nohup tools/engine_core/end_of_d1.sh --skip ratchet --out $OUT > $OUT.nohup 2>&1 &
```

(start through the harness with `run_in_background`; the script re-executes itself under `/root/heavy2.sh` and takes ONE of
the two lock slots for the whole run; if both are held it waits silently with a 20 s retry.) Add `BENCH_RECORD=1
BENCH_RECORD_BIN=$REC` to the env if the baselines were not recorded before. `ALPHA_SKIP` stays 1.

Steps in order, with expected wall time (ESTIMATES from the FLIP runbook and `ENGINE_COMPUTE.md`; the first run replaces them
with the numbers in `summary.txt`). Total A: about 12-20 h; start in the evening, read the summary in the morning.

| # | id | what | est. wall |
|---|---|---|---:|
| 1 | layers | `check_layers.py core/thin` + engine report | 1 min |
| 2 | prod-graph | production graph strict + self-tests | 1-2 min |
| 3 | lint | `lint_core.sh` + immutable surfaces vs `deployed-int23-54a15dba` | 1 min (measured: 1 s) |
| 4 | build | gate-profile bins (selfplay + every bridge bin), cold | 25-50 min |
| 5 | clippy | `clippy_core.sh` | 15-25 min |
| 6 | tests | packages, legacy lib all=dual (profile lockstep), D4 crate, gen-cards, xtask, wasm ts | 45-90 min |
| 7 | dual-all | sharded golden dual, all areas, bail probe | 90-150 min |
| 8 | dual-drv | driver=dual | 60-90 min |
| 9 | dual-wide | the wide mask | 60-120 min |
| 10 | dual-lent | lent set, 39 games | 60-100 min |
| 11 | board-t | templates | 5 min |
| 12 | board-g | golden positions | 10-20 min |
| 13 | sync | `core_sync_gate --strict` + marks census | 20-40 min |
| 14 | alpha | SKIPPED (reason logged) | 0 |
| 15-18 | bench-off, bench-resolver, bench-state, bench-resolver+state | 6 benches x 200 identical, per churn mode | 15-30 min each; +15-30 min for the record in `bench-off` if `BENCH_RECORD=1` |
| 19 | census | bail table + golden check core / string | 15-30 min |
| 20 | corpus | `decision_corpus verify` | 15-40 min |
| 21 | zero-bail | `zero_bail.py --enforce` | 1 min |
| 22 | strict | `TI4_CORE_STRICT=1` golden + driver dual | 60-100 min |
| 23 | soak | one round, 40 games per slice | 60-120 min |
| 24 | d4gate | cheap D4 steps | 5 min |
| 25 | ratchet | skipped in A | - |

Pass criteria: `summary.txt` ends `END OF D1 RUN OK (24 steps)`, every line `ok` or `skip alpha`; each bench log has six
`200 identical, 0 different`; `zero-bail.txt` has no cause; `golden check` core vs string identical.
Disk: the private target grows to 15-25 GB (gate + lockstep profiles), logs and shard outputs a few GB; peak use of run A
about 30-40 GB; run B (ratchet scratch) adds 5-10 GB. 127 GB free is enough for both.

## 6. Run B: the mutation ratchet

```sh
ZERO_BAIL=enforce nohup tools/engine_core/end_of_d1.sh --only ratchet --out $OUT > $OUT.ratchet.nohup 2>&1 &   # same --out, same target
```

It runs `mutation_ratchet.sh` over the 20 modules of `tools/engine_core/d4/mutation_modules.tsv` with `JOBS=4`,
`EXTRA_TEST_CRATES=ti4-engine-core-tests`, scratch under `$OUT/mut` (the scratch target is deleted after every module), results
in `$OUT/mutation/<module>.summary.json` and `.survivors.txt`. Estimate 2-14 h (the first baseline of the core alone took 33
minutes with the 336 core tests; the D4 crate adds about 2000 tests per mutant, so the big modules dominate: `core/driver`,
`core/rules-top`, `factions/top`, `factions/cards`).
A single module (after test work): `ROOT=/root/deploytmp/mut-x OUT=/root/deploytmp/mut-x/out EXTRA_TEST_CRATES=ti4-engine-core-tests JOBS=4 tools/engine_core/mutation_ratchet.sh core/driver` (hold a heavy slot: `/root/heavy2.sh <command>`).
Survivor workflow: FLIP runbook 2.5 (read the mutant, write a regression test that kills it, or mark equivalent mutants with
`// mutants:skip <reason>`; theory -> implementation -> Haiku runner). If a module cannot reach its floor even then: lower the
floor in `tools/engine_core/mutation_floor.json` by hand in its own commit and add a row to section 7.

## 7. Floor lowering log (fill in; empty = nothing lowered)

| date | module | floor before | floor after | measured | why it cannot reach the floor | commit |
|---|---|---:|---:|---:|---|---|
| | | | | | | |

## 8. Disk cleanup done on 2026-11-04

Removed with `git worktree remove` (no force; dirty worktrees refuse) 448 worktrees whose HEAD is an ancestor of
`core-integration-23`, `nightly-fixes-2026-10-26` or `main`, clean, and not used by a running process. Kept: everything dirty or
with unmerged commits, `nightly-reports/*` worktrees and targets, `phone-play`, `public-playtest`, `core-integration-23`,
`/root/deploytmp/*` worktrees, the main checkout and its `target/` (18 GB). No branch deleted. Candidates left for later
(not touched, need a decision): `/root/TI4-Bot-Dev/nightly-reports/target-fixer` 14 GB, `target-slot-1` 7.7 GB, `target-slot-2` 6.2 GB
(nightly build caches; the first nightly after the cron restore would rebuild them), `/root/ti4-target-bunker` 22 GB,
`/root/TI4-Bot-Dev/target` 18 GB, `/root/rulebot-results` 14 GB, `/root/scratch-undo` 2.5 GB, `/root/podcast-scratch` 0.7 GB.

## 9. Monitoring

```sh
tail -f $OUT/summary.txt                       # one line per step start / ok / FAIL / skip, with seconds
ls -t $OUT/*.log | head -3; tail -n 20 $OUT/<id>.log
df -h /root | tail -1; du -sh $CARGO_TARGET_DIR $OUT/mut 2>/dev/null
ls /root/.ti4-heavy.lock*; ps aux | grep -E 'dual_run|golden|selfplay|cargo|mutants' | grep -v grep | head
```
A run waiting for the lock prints nothing (check `ps` for `heavy2.sh`). Shards write their own progress under
`$OUT/dual-*-shards/`.

## 10. Resuming after a failure

A failing step does not stop the run; the summary names all failures and the exit status is 1. Per failure: read
`$OUT/<id>.log`, write a regression test that guards the fix (feedback rule), fix, then rerun only that step on the same out
dir and target: `tools/engine_core/end_of_d1.sh --only <id> --out $OUT` or everything from there on `--from <id>`. `census`
needs `$OUT/bail-probe.tsv` from `dual-all` (same `--out`). If the machine dies mid-run: `--from <first step without an
`ok` line in summary.txt>` with the same `--out` and `CARGO_TARGET_DIR` (the build cache survives). A full rerun happens once
at the end only if the fixes touched code reaching earlier steps (judge with `git diff --stat`). A killed ratchet leaves
scratch under `$OUT/mut`: delete it and rerun the module.

## 11. Cleanup at the end

`rm -rf /root/deploytmp/eod /root/deploytmp/core-rec /root/deploytmp/core-rec-target $OUT/mut $OUT/mut-work`
(keep `$OUT` itself: summary, survivors, bench logs, the record of the flip).
