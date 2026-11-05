# Heavy run plan (prepared 2026-11-04 by lane core-heavy-prep; brought to RULES_VERSION 36 by core-integration-24)

**Updated for `core-integration-24` (rules version 36).** Candidate: the tip of branch `core-integration-24` (= `core-rv36` + `web-feedback-ui-1` + this
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
| wasm (cargo `wasm-release` + binaryen 123 via `web-shared/scripts/build-wasm.mjs`) | int-23: 6 432 131 raw / 2 041 438 gzip / 1 469 073 brotli; `size-budget.json` ratcheted to 6.63 / 2.11 / 1.52 MB (real +3 %). **int-24 (version 36), measured: 6 441 691 / 2 044 587 / 1 471 798, all within the budget (the version 36 code adds 9.5 KB raw); budget unchanged**; wasm-opt 7.85 -> 6.44 MB; build 1 min 27 s |
| `lint` step | base retargeted (section 2; int-24: the tag `heavy-base-int24`); passes in 1 s; negative control (base = live tip `nightly-fixes-2026-10-26`) fails as it must |
| `alpha` step | skipped by default (`ALPHA_SKIP=1`), logged `SKIPPED: ...`, shown as `skip alpha` in `summary.txt` (the `corpus` step likewise: `skip corpus`) |
| bench baselines | re-record plan and flag in section 5 (`BENCH_RECORD=1`); not run |
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
git rev-parse heavy-base-int24 && git diff --stat heavy-base-int24 -- web/ crates/ti4-model/src/state.rs   # empty
```

## 4. Bench baselines for rules version 36 (do this BEFORE run A, or let run A do it with `BENCH_RECORD=1`)

What the benches are: `tools/rulebot/selfplay.sh bench, bench-v1 ... bench-v5` = 6 configs x 200 games (4p and 6p, seeds 1..100
on two maps each), rule bots version 4 (`bench`, golden.json), 5, 4, 1, 2, 3, compared hash by hash with
`tools/rulebot/baseline{,-v1..v5}.json`. The configs pin the rules version: golden.json 23, golden-v5.json 18, golden-v2.json and
golden-v4.json 11. The native engine plays only the newest version (36), so the stored baselines can no longer be verified; they must be
re-recorded (precondition 10 of the FLIP runbook, R5 of D2-8). The `completion*` baselines are another tool and are not touched.

**What version 36 changes for this procedure (int-24).** The frozen recorder is the string engine and plays 35 (its Ixthian Artifact (For) and its
Parley are the documented string bugs S6 and S7). The engine under test plays 36 and does what the cards print. Measured on the stored bench streams
(`tools/engine_core/decision_corpus`, 4 games per bench): **3 of the 4 games of every bench play a Parley** (for example `bench`: games 20, 120, 121) and one
reaches the Ixthian Artifact; so roughly half the games of every bench change course at the first Parley, and the literal criterion "200 identical x6
against a string recording" cannot hold any more. It is not a core bug: it is S7/S6 by design. The procedure therefore has two stages:

1. **String recording at 35** (independent reference). `record_benches.sh` writes the six baselines with the frozen recorder and, with `FINAL_RV=36`
   (the default of `end_of_d1.sh` through `BENCH_FINAL_RV`), sets the four configs to rules version 36 afterwards, because the native selfplay refuses 35.
   The hashes do not contain the rules version.
2. **Native verify.** `verify_benches.sh` (step `bench-off`) plays the six benches natively against that recording. Expected: games without a Parley play and without
   an Ixthian For identical, the others different. For every differing game replay it (`selfplay replay`, flags in the bench log) and confirm the
   first difference is at a Parley play (`reaction:...:UNITS_COMMITTED:after` with the card parley chosen) or at the agenda Ixthian Artifact (For), i.e. the
   `known string bug S6/S7` the shadow of `native_play` names. A difference at any other step is a core-vs-string bug (R5) and gets fixed, never
   re-recorded. Only then re-record the six baselines with the NATIVE `selfplay-new` (the committed baselines are the version 36 ones) and commit
   configs + baselines in ONE commit; the native verify of the new files is then identical by construction and the bench steps pass from then on.

```sh
# 1. recorder (one release-class build, selfplay only; about 10-15 min at 2 jobs; private target ~6-8 GB, delete afterwards)
git worktree add --detach /root/deploytmp/core-rec core-integration-20
cd /root/deploytmp/core-rec && CARGO_TARGET_DIR=/root/deploytmp/core-rec-target CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 \
  cargo build --profile gate -p ti4-selfplay --bin selfplay
REC=/root/deploytmp/core-rec-target/gate/selfplay
# 2. record at 35 (in the candidate worktree); the configs come back at rules version 36; JOBS=3 benches at a time, 6 workers in total;
#    expected 15-30 min wall
cd <candidate worktree> && FINAL_RV=36 tools/engine_profile/record_benches.sh $REC 6 35
# 3. native verify: run `tools/engine_core/end_of_d1.sh --only build bench-off ...` (or verify_benches.sh with the new selfplay), list the differing
#    games per bench, replay each, classify (Parley / Ixthian For = by design, anything else = bug)
# 4. native re-record of the six baselines (the same script, the native binary as recorder, the version the binary plays):
#    tools/engine_profile/record_benches.sh $CARGO_TARGET_DIR/gate/selfplay 6 36
git add tools/rulebot && git commit -m "bench baselines re-recorded for rules version 36 (string reference at 35 classified, native recording)"
```

Alternatively stage 1 inside the run: `BENCH_RECORD=1 BENCH_RECORD_BIN=$REC tools/engine_core/end_of_d1.sh ...` (records at `BENCH_RECORD_RV`, default 35, sets
the configs to `BENCH_FINAL_RV`, default 36, then `bench-off` verifies the fresh `selfplay-new`; `bench-record.txt` and `bench-off.txt` of the out dir are
the evidence for the classification). The other three bench steps (`TI4_CHURN` resolver / state / both) only verify. In that case run `bench-off` before
the long steps if the baselines are wanted early: `--only build bench-off`. The new files stay uncommitted in the worktree: commit them before the tag of the
run. Reasons a hash may legitimately differ: Parley and Ixthian Artifact (For) only (S7, S6), the two cards version 36 changed. The plain
`selfplay.sh bench` of int-23 is red until this is done.

**RESULT OF THE LANE (core-bench-rv36, 2026-11-05): stage 1 done, stage 2 FAILED the classification, nothing re-recorded, nothing committed to `tools/rulebot`.**
See `docs/BENCH_RV36_RESUME.md`. In short: the string recording at 35 is fine (6 x 200 victories, 10 min wall), but the native engine under test (int-24 tip) differs from it in
1 189 of 1 200 games (not about half), and 61 games (5 %) end in `engine_error` because the core stops (`Unported(HitsAddedToARoll)`, `PendingDestructions`, `NomadTcsNegotiating`
mark without a text, `UnportedAftermath(CombatUnported)`, `MissingFact`, `planet entry in an untouched system`). A first-difference scan of a sample shows that Parley / Ixthian For explain
at most a small part of it. Two procedure notes: `verify_benches.sh` looks games up by the whole spec, which carries the rules version, so a recording made at 35 shows "200 not in the
baseline" at 36 (the compare must ignore the version: `tools/engine_profile/bench_rv36/`); and the string recorder accepts `--rules-version 36` and plays it exactly as 35 (hashes identical).
The bench steps stay red until the differences are explained or ruled by-design by the user.

**UPDATE (core-bench-stops, 2026-11-05): the 61 native stops are fixed** (`docs/BENCH_STOPS_RESUME.md`): 0 `engine_error` in 6 x 200 games, the decision streams of the games that already worked unchanged. The native
re-recording of the six baselines is prepared in a copy (`tools/engine_profile/bench_rv36/prepare_native_record.sh`), not adopted. The S1 order explains an early part of the divergence from the string, not all of it (same file).

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

## 12. Starting run A (int-24): what is known red, what the user says, what is run

### 12.1 Known before the start (measured on core-integration-24, 2026-11-04 evening)

1. **Golden dual (steps 7 dual-all, 8 dual-drv, 9 dual-wide, 10 dual-lent, 22 strict) was red by construction, one cause fixed here, one open.**
   `dual_run tools/engine_profile/profile-20.json 20 all=dual --game N` on games 1, 6, 8 gave 72 / 29 / 50 differences, every one
   `[driver/Value] resolver.state`: the string's initiative list is the seating order of `Game::new` (string bug S1, user ruling 2026-11-01), the core's is the position's
   initiative order. The same numbers appear at `core-botstops-1` (before version 36), so it is not a version 36 effect: nobody had run the golden dual since the S1 ruling.
   Fixed in `compare_resolver` (`dual_window.rs`): the seats of the initiative list are compared as a set, not their order (test
   `the_order_of_the_initiative_list_is_not_compared_the_seats_are_s1`). After the fix games 1 and 8 are clean (exit 0).
   **Still open: game 6** (`winnu,titans,mentak,ghost`, 4pHyperlanes seed 7) has one `[driver/Value] combat::settle` difference: the string's roll groups of the settle are
   `[(7,[1,7]),(9,[6]),(0,[8,7])]`, the core's lack the last group `(0,[8,7])` (an extra ability roll of the string, no hit threshold). Identical at core-botstops-1.
   With `TI4_DUAL_FAIL_FAST=1` the shard holding game 6 stops there; so dual-all / dual-drv / dual-wide are likely to FAIL in run A unless that is explained first
   (string bug or core gap) -- one agent, one game: `dual_run tools/engine_profile/profile-20.json 20 all=dual --game 6`. The other 17 games were not run.
2. **Bench steps (15-18)** cannot be green on a string recording at 35. The lane of section 4 was run (core-bench-rv36) and did NOT confirm that Parley / Ixthian For are the only causes: 1 189 of 1 200 games differ, 61 end in a core stop (`docs/BENCH_RV36_RESUME.md`). The bench steps stay red; baselines were not re-recorded. Do the section 4 lane BEFORE run A, so that the committed baselines are the
   version 36 ones; otherwise expect `bench-off`, `bench-resolver`, `bench-state`, `bench-resolver+state` red in the summary of run A (by design, not a core bug).
3. `corpus` is skipped (section 5), `alpha` is skipped.

### 12.2 What the user says

Either of

* "Start run A" -- run A at once with the four bench steps and possibly the dual steps expected red (item 1 and 2 above), the other steps meaningful; or
* "Bench lane first, then run A" (the lane was run on 2026-11-05 and stopped at the classification, see the result box in section 4) -- one agent does section 4 (record at 35 with the string recorder, native verify, classify the differing games as Parley / Ixthian For,
  re-record natively, one commit), another looks at game 6 (item 1); run A starts when both are done.

### 12.3 What is run (the coordinator, on the go)

```sh
cd /root/TI4-Bot-Dev/.claude/worktrees/core-integration-24                     # clean tree at the tip
git rev-parse heavy-base-int24 && git diff --stat heavy-base-int24 -- web/ crates/ti4-model/src/state.rs   # tag exists, diff empty
TZ=Europe/Berlin date; df -h /root | tail -1                                  # not within the hour before 20:30; >= 60 GB free
for f in /root/.ti4-heavy.lock /root/.ti4-heavy.lock.2; do flock -n $f true && echo free $f || echo BUSY $f; done
export CARGO_TARGET_DIR=/root/deploytmp/eod CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
OUT=/root/end-of-d1-$(date -u +%Y%m%d)
ZERO_BAIL=enforce SHARDS=3 WORKERS=6 GAMES=20 SOAK_GAMES=40 \
  nohup tools/engine_core/end_of_d1.sh --skip ratchet --out $OUT > $OUT.nohup 2>&1 &     # via the harness with run_in_background
```

Then monitor with section 9. Run B (ratchet) per section 6 after A is understood. Delete `/root/deploytmp/eod` and the recorder dirs at the end (section 11).

