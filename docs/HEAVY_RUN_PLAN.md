# Heavy run plan (prepared 2026-11-04 by lane core-heavy-prep; brought to RULES_VERSION 36 by core-integration-24)

**Updated for `core-integration-27` (2026-11-06; see `docs/INTEGRATION27.md`).** `core-integration-27` = `core-heavyfix-1` (the fixes for the six steps that failed in round 2: `tests`, the four benches, `zero-bail`; `docs/HEAVYFIX1_RESUME.md`). Lint base: the tag `heavy-base-int27` (its tip; `web/` and `state.rs` equal `heavy-base-int26`, checked). A re-run uses the same command as 12.3/12.4 with that tag and a fresh out dir.

**Updated for `core-integration-26` (2026-11-05; see `docs/INTEGRATION26.md`).** Lint base: the tag `heavy-base-int26` (the tip of `core-integration-26`; `web/` and `state.rs` equal the int-25 tag, checked). The soak
lane `old` (rules versions 0..34, refused by the product) is out of the default plan (`soak_corpus.py lanes`; `lanes --all`, `LANES=old` and `gen old` still make it). Run A (`/root/end-of-d1-20261105`) failed `dual-lent`,
`zero-bail`, `soak` and `d4gate`; the fixes are merged in int-26 (`DUALLENT_FIX_RESUME.md`, `LENT_GAPS_RESUME.md`, `ZEROBAIL_RESUME.md`), and the re-run ("run A, round 2") is started per section 12.3 with the out dir `/root/end-of-d1-<date>-r2`. Everything below
that still says int-25 is the history of the first run.

**Updated for `core-integration-25` (rules version 36; bench baselines native, golden dual clean, lint base `heavy-base-int25`; see `docs/INTEGRATION25.md`).** Earlier text for `core-integration-24` follows. Candidate: the tip of branch `core-integration-24` (= `core-rv36` + `web-feedback-ui-1` + this
plan; see `docs/INTEGRATION24.md`). What changed against the first version of this file: the version is 36 (the core plays 36 only; `MIN_SUPPORTED_RULES_VERSION`
follows), the golden set `v36-std` and the ladder game 36 are recorded (v35-std, te and templates are frozen), the `lint` base is the new tag
`heavy-base-int24`, the `corpus` step is skipped with a recorded reason (section 5), and the bench baselines are re-recorded in a different way (section 4:
the string recorder is at 35, the engine under test at 36, and the games that play a Parley or an Ixthian Artifact (For) differ by design). Section 12 says
how to start run A.

Original base: `core-integration-23` (6d66acab5; deployed 54a15dba). Nothing in this document was run except the cheap
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
| wasm (cargo `wasm-release` + binaryen 123 via `web-shared/scripts/build-wasm.mjs`) | int-23: 6 432 131 raw / 2 041 438 gzip / 1 469 073 brotli; `size-budget.json` ratcheted to 6.63 / 2.11 / 1.52 MB (real +3 %). **int-24 (version 36), measured: 6 441 691 / 2 044 587 / 1 471 798, all within the budget (the version 36 code adds 9.5 KB raw); budget unchanged**; wasm-opt 7.85 -> 6.44 MB; build 1 min 27 s. **core-multiask-1-rebase (2026-11-06): budget raised to 6 660 000 / 2 126 000 / 1 532 000 (user decision; real 6 655 547 / 2 121 013 / 1 527 033), see `docs/MULTIASK1_RESUME.md` 7.3** |
| `lint` step | base retargeted (section 2; int-24: the tag `heavy-base-int24`); passes in 1 s; negative control (base = live tip `nightly-fixes-2026-10-26`) fails as it must |
| `alpha` step | skipped by default (`ALPHA_SKIP=1`), logged `SKIPPED: ...`, shown as `skip alpha` in `summary.txt` (the `corpus` step likewise: `skip corpus`) |
| bench baselines | int-25: native recordings at 36 are committed (section 4); the bench steps verify native against native |
| `end_of_d1.sh --dry-run` | 25 steps, ids below; ok (int-24: re-checked) |
| golden (int-24) | `v36-std` (4 games, 7,896 decisions) and ladder game 36 recorded under the heavy lock; `golden check` of all 200 games / 147,781 decisions green under the default mask, `all=core` and `all=string`; `every_version_has_a_set` green |
| locks | `/root/.ti4-heavy.lock` and `.lock.2` both free at 20:03 Berlin (probe with `flock -n`) |

wasm-opt: it is the `binaryen` devDependency of `web/` (123.0.0); `cd web && npm ci --ignore-scripts` installs it, and
`build-wasm.mjs` finds `web/node_modules/.bin/wasm-opt` by itself. It is installed in the worktree `core-heavy-prep` only (any
fresh worktree needs the same `npm ci`; `deploy.sh` runs it). Without it the raw size is 7.83 MB, over the old 7.0 MB budget.

## 2. The base ref of the `lint` step

`lint` runs `lint_core.sh` and `git diff --exit-code <base> -- web/ crates/ti4-model/src/state.rs ':!docs/RULES_VERSION.md'`
(plus the "only the refusal note may change" rule for `docs/RULES_VERSION.md`; this is the wording of G7 row 2 in
`docs/D29A_RESUME.md`). The old default base `core-integration-6` is hopelessly stale (42 files of `web/` differ), and the
live tip `nightly-fixes-2026-10-26` cannot be the base either: the P1 commits of the flip changed `web/` (50 files vs the live
tip) and `state.rs` (+65 lines) on purpose.

**Int-24 decision:** the base is the local tag **`heavy-base-int24`**, created on the tip commit of `core-integration-24` (the commit that
contains the three web files of `web-feedback-ui-1` and the version 36 `state.rs`). The first version of this plan used the tag
`deployed-int23-54a15dba`; against it the `lint` step is red by construction since rules version 36 changed `state.rs` and the web feedback UI
changed `web/` (both on purpose, both after the deploy). Fallbacks: branch `core-integration-24`, then `origin/core-integration-24`; `--base-ref REF` /
`BASE_REF` override. Meaning: after int-24 nobody may touch `web/` or `state.rs` through the heavy-run tip (a fix that must, needs `--base-ref` and a
note). The tag is local (not pushed). If a commit lands on `core-integration-24` after the tag (only docs/tools/golden may), move the tag only
with a diff showing that no file under `web/` or `state.rs` changed.

## 3. Before the go: the exact preflight (cheap)

```sh
cd /root/TI4-Bot-Dev/.claude/worktrees/<candidate worktree>      # clean tree at the tip to be tested
TZ=Europe/Berlin date; df -h /root | tail -1                      # >= 60 GB free
for f in /root/.ti4-heavy.lock /root/.ti4-heavy.lock.2; do flock -n $f true && echo free $f || echo BUSY $f; done
uptime; ps aux | grep -E 'cargo|rustc|perf_ab' | grep -v grep     # nothing heavy running
tools/engine_core/end_of_d1.sh --dry-run                          # 25 steps
git rev-parse heavy-base-int26 && git diff --stat heavy-base-int26 -- web/ crates/ti4-model/src/state.rs   # empty
```

## 4. Bench baselines for rules version 36 (DONE in core-integration-25: native baselines are committed)

What the benches are: `tools/rulebot/selfplay.sh bench, bench-v1 ... bench-v5` = 6 configs x 200 games (4p and 6p, seeds 1..100
on two maps each), rule bots version 4 (`bench`, golden.json), 5, 4, 1, 2, 3, compared hash by hash with
`tools/rulebot/baseline{,-v1..v5}.json`. The `completion*` baselines are another tool and are not touched.

**State since int-25.** The six baselines in `tools/rulebot/` are the NATIVE recordings at rules version 36, and the four configs (`golden.json`, `golden-v2.json`,
`golden-v4.json`, `golden-v5.json`) carry `rules_version` 36 (they were 23 / 11 / 11 / 18; the native engine plays only 36 and refuses the others). They were
recorded by lane `core-bench-stops` with `tools/engine_profile/bench_rv36/prepare_native_record.sh` (6 x 200 games at 36, 0 `engine_error`, then verified with the same binary:
`200 identical, 0 different` x6) and adopted in ONE commit (`cp native36-rb/baseline*.json native36-rb/golden*.json tools/rulebot/`). So the bench steps (`bench-off`, `bench-resolver`,
`bench-state`, `bench-resolver+state`, and `verify_benches.sh`) are native against native: each must print six times `200 identical, 0 different`. `BENCH_RECORD` stays off.

Why not a string recording (history, kept so nobody tries again): the frozen string recorder plays 35 and has the documented bugs S1 (reaction order), S6 (Ixthian Artifact (For)), S7 (Parley)
and S2-S5; the lane measured 0 of 200 `bench` games identical to the string recording with the ruling's order and 12 of 200 even with the string's order in every window
(`docs/BENCH_STOPS_RESUME.md`, last section). The equivalence with the string is covered by the golden dual (all 20 games, `docs/DUAL_REST_RESUME.md`), the hashes of the benches are a regression
guard of the native engine against itself. A change of a rule, a card or a bot that legitimately changes decision streams re-records the baselines with
`tools/engine_profile/bench_rv36/prepare_native_record.sh NATIVE_SELFPLAY OUTDIR` (about 15 min record + 15 min verify at 6 workers, hold a heavy slot) in its own commit, and says why in the commit message.

The old `verify_benches.sh` "spec includes the rules version" issue: the specs inside a recording carry the rules version, so a recording verifies only at the version it was made at. With the configs and the
baselines both at 36 this holds by construction; it only matters again when `RULES_VERSION` moves (then re-record, configs and baselines in one commit; `record_benches.sh` with `FINAL_RV` does the config part).
No script change was needed.

Old two-stage procedure (string recording at 35 plus classification of the differing games) is obsolete; `record_benches.sh` and `BENCH_RECORD=1` remain for a deliberate re-record with any recorder binary.

## 5. Run A: everything but the ratchet

```sh
cd <candidate worktree>
export CARGO_TARGET_DIR=/root/deploytmp/eod CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
OUT=/root/end-of-d1-$(date -u +%Y%m%d)
ZERO_BAIL=enforce SHARDS=3 WORKERS=6 GAMES=20 SOAK_GAMES=40 \
  nohup tools/engine_core/end_of_d1.sh --skip ratchet --out $OUT > $OUT.nohup 2>&1 &
```

(start through the harness with `run_in_background`; the script re-executes itself under `/root/heavy2.sh` and takes ONE of
the two lock slots for the whole run; if both are held it waits silently with a 20 s retry.) `BENCH_RECORD` stays off (the baselines are committed, section 4). `ALPHA_SKIP` stays 1.

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
| 15-18 | bench-off, bench-resolver, bench-state, bench-resolver+state | 6 benches x 200 identical, per churn mode | 15-30 min each |
| 19 | census | bail table + golden check core / string | 15-30 min |
| 20 | corpus | SKIPPED by default (`CORPUS_SKIP=1`, reason logged): `decision_corpus verify` fails on 9 of 10 sources by construction (the core plays only version 36; sources are versions 11/18/23/35 and TE) and `fixtures` is stale since I22; the same games replay from the golden sets in the `census` step | 0 |
| 21 | zero-bail | `zero_bail.py --enforce` | 1 min |
| 22 | strict | `TI4_CORE_STRICT=1` golden + driver dual | 60-100 min |
| 23 | soak | one round, 40 games per slice | 60-120 min |
| 24 | d4gate | cheap D4 steps | 5 min |
| 25 | ratchet | skipped in A | - |

Pass criteria: `summary.txt` ends `END OF D1 RUN OK (24 steps)`, every line `ok` or `skip alpha` / `skip corpus` (the skip lines are not failures; the end line counts steps); each bench log has six
`200 identical, 0 different` (after the native re-recording of section 4); `zero-bail.txt` has no cause; `golden check` core vs string identical.
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

## 12. Starting run A (int-25): what is known red, what the user says, what is run

### 12.1 Known before the start (int-25, 2026-11-05)

1. **Golden dual: all 20 games pass** (`dual_run tools/engine_profile/profile-20.json 20 all=dual`, exit 0, 0 differences) since `core-dual6` (B6, the old game 6 `combat::settle`: a harness scope gap) and
   `core-dual-rest` (S8 Nano-Forge is a recorded string bug, the other classes core gaps or harness gaps; `docs/DUAL_REST_RESUME.md`). The earlier int-24 text (S1 initiative order compared as a set, game 6 open) is history.
2. **Bench steps (15-18)**: native baselines at 36 are committed (section 4), the 61 native stops of the first verify are fixed (`docs/BENCH_STOPS_RESUME.md`); the steps verify native against native. Expected green.
3. `corpus` is skipped (section 5), `alpha` is skipped.
4. The `lint` base is the tag `heavy-base-int25` (the tip of `core-integration-25`; `end_of_d1.sh` tries it first, then `heavy-base-int24`, section 2). The step `tests` runs the D4 crate with debug assertions on
   (`D4FLAGS`), where `state_basics::a_state_with_a_slice_missing_refuses_to_read_it_in_debug` asserts the refusal; in the optimised profiles the test no longer claims it (the guard is a `debug_assert!`).

### 12.2 What the user says

"Start run A" (given on 2026-11-05 for the night). Nothing else is open before it.

### 12.3 What is run (the coordinator, on the go)

```sh
cd /root/TI4-Bot-Dev/.claude/worktrees/core-integration-26                     # clean tree at the tip
git rev-parse heavy-base-int26 && git diff --stat heavy-base-int26 -- web/ crates/ti4-model/src/state.rs   # tag exists, diff empty
TZ=Europe/Berlin date; df -h /root | tail -1                                  # not within the hour before 20:30; >= 60 GB free
for f in /root/.ti4-heavy.lock /root/.ti4-heavy.lock.2; do flock -n $f true && echo free $f || echo BUSY $f; done
export CARGO_TARGET_DIR=/root/deploytmp/eod26 CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0   # a fresh dir
OUT=/root/end-of-d1-$(date -u +%Y%m%d)-r2
ZERO_BAIL=enforce SHARDS=3 WORKERS=6 GAMES=20 SOAK_GAMES=40 \
  setsid nohup tools/engine_core/end_of_d1.sh --skip ratchet --out $OUT > $OUT.nohup 2>&1 &     # detached
```

Then monitor with section 9. Run B (ratchet) per section 6 after A is understood. Delete `/root/deploytmp/eod` and the recorder dirs at the end (section 11).


### 12.4 Round 2 (int-26)

Same command as 12.3 with the tag `heavy-base-int26`, target `/root/deploytmp/eod26`, out dir `/root/end-of-d1-<date>-r2`. Steps to watch (failed in run A): `dual-lent`, `zero-bail`, `soak`, `d4gate`. The
soak plan has no `old` lane any more (12.1 of the first run is history). The ratchet (run B) is not part of this run. Expected duration: run A took 3.5 h.
