# Engine compute: tools that spend the machine's cores instead of the agent's tokens

Branch `core-compute` (from `core-dual-shards`). Tooling, build configuration and test harness only: no engine rule,
no `RULES_VERSION`, serde, fixture, fingerprint or bench baseline changed. Machine: 8 cores, 15 GB RAM, **a disk that
is 97-100 % full** (several agents keep 20 GB target directories), no reflinks (ext4), no sccache, no nextest, no
cargo-mutants, no mold/lld. All numbers below were taken on this machine while other agents were running, so
treat them as orders of magnitude, not as benchmarks; load averages are stated where they mattered.

| # | Tool | Use |
|---|------|-----|
| 1 | `tools/engine_core/mutants.sh` | mutation testing of a module with ONE build |
| 2 | `tools/engine_core/target_dir.sh` | private target dir seeded from a warm shared one |
| 3 | `[profile.lockstep]`, `[profile.lockstep0]` | lighter profiles for test loops |
| 4 | `tools/engine_profile/verify_benches.sh` (JOBS), `tools/engine_core/test_parallel.sh` | concurrent benches and test executables |

## 1. Mutation switches

Before: an agent that wants to know whether its tests would notice a changed `<` edits the source by hand and
rebuilds, once per mutant. Now one build holds all mutants behind a runtime switch and the targeted tests run once
per mutant without any rebuild.

```
tools/engine_core/mutants.sh query::tech query::tech::                       # core module, its own inline tests
tools/engine_core/mutants.sh crates/ti4-engine-core/src/rules/                # a directory (generated/ and tests/ are skipped)
TEST_CRATE=ti4-engine-core-bridge TEST_ARGS="--test tech" \
  tools/engine_core/mutants.sh query::tech                                    # mutate the core, run the ADAPTER's tests
FN=can_research,researchable tools/engine_core/mutants.sh query::tech query::tech::   # only these functions
LIST=1 tools/engine_core/mutants.sh query::tech                               # just list the mutation points
```

Arguments: `TARGET` (file, directory or module path; the crate is taken from the path, so files of
`crates/ti4-engine/src/...` work the same) and the libtest filter words. Output: one line per mutant

```
SURVIVED #20  crates/ti4-engine-core/src/query/tech.rs:122:14  [active] replace & with |:  scope.0 & source_bit(Source::ThundersEdge)  =>  scope.0 | source_bit(Source::ThundersEdge)
killed   #19  ...tech.rs:122:14  [active] replace != with ==:  ...  by printed_technology_data_matches_the_string_engine
```

then `N mutants run: K killed, M survived` and the list of survivors again. A survivor is a behaviour change no
test of your filter notices: write a test that fails for it, or, if it is equivalent (cannot change any result), put
`// mutants:skip` on that line (or on / above the `fn` line to skip the whole function; comments pass
`lint_core.sh`). Report: `$MUT_DIR/report.json`; the rewritten copy: `$MUT_DIR/tree` (read it when a mutant looks
odd). Env: `JOBS` (mutants in parallel, 4), `KINDS` (`body,arith,cmp,logic,cond,lit,assign`), `FN`, `LINES=S-E`,
`TIMEOUT` (per mutant; default 6x the baseline run + 10 s; a timeout counts as killed), `QUIET=1` (survivors only),
`STRICT=1` (exit 1 on survivors), `CARGO_BUILD_JOBS` (2).

### Options evaluated

* cargo-mutants: not installed and not in the offline registry cache; it also rebuilds once per mutant, which is
  the cost to remove.
* (a) a hand-written `mutant!` macro / `cfg(feature = "mutants")` in the core: needs every mutation site written
  by hand into committed source and touches the lint-checked tree. Rejected.
* (b) **chosen:** a source rewriter (`tools/mutants`, a small `syn` program, not a workspace member, own
  `Cargo.lock`, builds offline in 3 s with the `syn`/`proc-macro2` already in the registry cache). It reads the
  pristine files, writes the wrapped text into a COPY of the tree (`$MUT_DIR/tree`, rsync-ed from the checkout on
  every run) and adds `src/__mutant.rs` (an `AtomicU32` read once from `TI4_MUTANT`) plus `mod __mutant;` to the
  crate root. Nothing in the checkout changes, so `lint_core.sh` and `check_layers.py` see the committed text
  only (re-run unchanged: 357 files, 0 hits; 355 files, 0 violations) and the mutated variant can never be
  committed. The wrapped code is exact when `TI4_MUTANT` is unset: tests of the copy pass as the original does
  (the runner refuses to start when the baseline run fails).

Mutation points (all only in non-test, non-`const` code): function body replaced by `()`, `true`/`false`,
`0`/`1`, `None`, `Ok(())`, `Vec::new()`, `String::new()` or `Default::default()` by return type; `+ - * / % & | ^
<< >>` swapped; `== != < <= > >=` changed (`<` to `<=`, `==` to `!=`, ...); `&&` and `||` swapped; `if` and match
guard conditions negated (not `if let` and let chains); integer literals +1; bool literals flipped; compound
assignments (`+=` to `-=`, ...). Not mutated: macro bodies (`assert!`, `matches!`, `format!` arguments are token
streams), patterns, types, `const`/`static`/enum discriminants, `const fn`, `#[cfg(test)]` code.

A rewrite that does not compile (type inference, moves, `T: Default` missing, ...) is found without a human: the
build runs with `--message-format=json`, the failing points are mapped from the error positions back to the
mutation points and disabled, the tree is regenerated and built again (usually 1-2 extra rounds, each an
incremental build of the mutated crate). The count of such points is printed (`N points not compilable ...
skipped`); on the `tech.rs` pilot it was 0 of 192, on a stress file of generics / let chains / closures / `?`, 3 of 95.

Adapter files work the same (`mutants.sh crates/ti4-engine/src/technology_core.rs <filter>`, the crate is taken from
the path; `TEST_ARGS` defaults to `--lib`). Checked with `CHECK_ONLY=1` (rewrite + `cargo check --tests`, no test
run) on `technology_core.rs` (484 lines): 86 mutation points, 4 compile errors in the first build, 2 points dropped
after one prune round, 84 live. That check took 5 min 27 s at 2 jobs under `nice` (it is a first, cold dev check of
the 335k-line `ti4-engine` on a loaded machine); the test run of such a mutation needs a dev-profile test build of
`ti4-engine` once (minutes), then seconds per mutant. Pick the filter words so the run is the tests of that module.

### Pilot: `crates/ti4-engine-core/src/query/tech.rs` (696 lines, 192 mutation points)

| | build | run of the mutants | killed / survived | wall |
|---|---|---|---|---|
| inline tests only (`query::tech::`, 4 tests) | 28 s cold (dev profile, 2 jobs), 4 s when warm | 0.5 s | 50 / 142 (26 %) | 29 s cold, 16 s warm |
| all 137 lib tests of the core | same build | 11 s | 50 / 142 | 16 s warm |
| adapter tests (`TEST_CRATE=ti4-engine-core-bridge TEST_ARGS="--test tech"`, compares with the string engine) | 124 s cold (the bridge pulls in `ti4-engine`) | 83 s (1.6 s per run, 4 in parallel, machine loaded) | 99 / 93 (52 %) | 3 min 28 s |
| **manual, rebuild per mutant** (same bridge test, 3 hand-made mutants #17, #19, #20) | 162 s cold | 35 s, 45 s, 44 s per mutant (rebuild of the core and `ti4-engine` + the test), mean 42 s | all three killed, as in the table above | 192 mutants = about 2 h 15 min |

So with the adapter tests: **3.5 min for all 192 mutants instead of about 2 h 15 min (about 39x)**, and a repeat
run after changing only a test costs the build-free part (about 1.5 min); with the core's own tests it is seconds
(the manual per-mutant cost of those, a rebuild of the 59k-line core only, was not measured). The numbers are from
a machine at load average 3-10 (other agents' gates), so absolute times are pessimistic.

What the pilot says about the module: the 4 inline tests of `tech.rs` kill 26 % of its mutants; the bridge
equivalence test kills 52 %; the rest (for example `ready`, `owned`, `colours_of`, the `+= 1` counters) is covered
by neither, which is the information the tool exists to give.

### Caveats

* Survivors need judgement: some mutants are equivalent (`<` against `<=` where the operands can never be equal).
  Mark them `// mutants:skip` with a reason.
* The switch is a global per process: the runner starts one process per mutant (4 at a time). Tests that need a
  mutant switched inside a running process are not supported (`__mutant::active()` reads the environment once).
* Mutants that loop forever are killed by the timeout (counted as killed, printed `(timeout)`).
* Mutated code is compiled with debug assertions of the dev profile: an arithmetic overflow caused by a mutant
  panics and counts as killed. The build is `cargo test` in the dev profile of
  the copy with `CARGO_PROFILE_DEV_DEBUG=0` (the bridge tests with the default dev debug info took 3.6 GB of
  a nearly full disk). `$MUT_DIR/target` is kept for the next run; delete it when done: `rm -rf /root/deploytmp/mutants-*/target`.
* The scratch copy excludes `web*`, `.git`, `target`. A test that reads those fails in the baseline and the run stops.
* Only the files named are mutated; their callers' behaviour is observed through the tests you select.

## 2. Shared build cache

Situation: every agent builds in a private `CARGO_TARGET_DIR` (they must not share one: two worktrees in one
target dir invalidate each other's workspace crates all the time, and cargo serialises them on the dir lock).
So every new worktree starts with a cold build of all registry dependencies and of the 335k-line `ti4-engine`.

Evaluated:

* **sccache**: not installed, not in the offline registry cache, cannot be installed here. (It also does not cache
  incremental or proc-macro/build-script outputs, and the workspace crates are what is slow.)
* One shared target dir per profile behind a lock: rejected (see above; serialises all agents).
* **Chosen: seed a private dir from a shared warm one.** Cargo's unit hashes do NOT contain the checkout path
  (verified: a different worktree finds the existing fingerprints), so a warm dir built at some commit is reusable
  by any worktree; the only things that make cargo rebuild are (1) source mtimes newer than the warm build and
  (2) changed content. A fresh worktree has all-new mtimes, so `target_dir.sh seed` sets the mtime of every tracked
  file and directory that is identical to the warm commit to just before the warm build (a changed file keeps its
  new time, so exactly it and its dependents rebuild). The disk has no reflinks and is nearly full, so the seeded
  dir hard-links the big artifacts (>= 64 KB: rlib, rmeta, executables) of the warm dir and copies the small
  metadata (cargo rewrites `.fingerprint/` and `.d` files in place; a first version that linked them modified the
  warm dir, the final one leaves it byte-identical: checked, 0 files changed).

```
tools/engine_core/target_dir.sh warm                       # once per merge, by whoever holds the live checkout (or the coordinator):
                                                           #   builds `cargo test --profile gate -p ti4-engine --lib --no-run` into /root/deploytmp/warm-target
tools/engine_core/target_dir.sh warm /path/dir test --profile gate -p ti4-engine-core-bridge --bins    # any other set of cargo arguments
export CARGO_TARGET_DIR=/root/deploytmp/<agent>-target
tools/engine_core/target_dir.sh seed $CARGO_TARGET_DIR      # seconds; DEST must be empty or new
cargo test --profile gate -p ti4-engine --lib --no-run      # only what differs from the warm commit is compiled
tools/engine_core/target_dir.sh rm $CARGO_TARGET_DIR        # when done (refuses paths outside /root/deploytmp)
```

Rules of the recipe: the warm dir must have been built with the same toolchain, profile, features, `RUSTFLAGS` and
`CARGO_BUILD_JOBS` is irrelevant. A seed from a warm commit far behind the agent's branch is still correct (cargo
rebuilds what differs), it just saves less; refresh the warm dir after big merges. `seed` touches mtimes only in the
checkout it is run in (never content). A warm dir for each profile you use (`gate`, `lockstep`, dev) can live in
the same SEED dir: run `warm SEED test --profile lockstep ...` after the first. Keep the hard-linked artifacts in
mind when deleting: removing a seeded dir frees only what was rebuilt; removing the warm dir frees the rest only once all seeded dirs are gone.

Measured (`tools/engine_core/compute_bench.sh seed WORK`, `gate` profile, 4 jobs):

| `cargo test --profile gate -p ti4-engine-core --lib --no-run` | wall | crates compiled |
|---|---|---|
| cold, private empty dir (= the warm build) | 35 s | all dependencies + 3 workspace crates |
| seeded, same commit, other worktree | **0.2 s** | none |
| seeded, 1 changed file in `ti4-engine-core` | **11.5 s** | `ti4-engine-core` only |

**Engine-scale numbers: PENDING.** The `ti4-engine` measurements (cold gate build vs seeded, and gate vs lockstep vs
lockstep0: build, run, incremental) need more than 5 minutes each, so they must hold `/root/.ti4-heavy.lock`, which
other agents' gates held for hours during this work, and they need about 3 GB of a disk that was at 98-100 %. They are
queued as one lock hold (`/root/deploytmp/core-compute/engine_meas.sh`, output `/root/deploytmp/core-compute/engine.out`,
`RESULT` lines in the format above; it aborts with a `WATCHDOG` line when less than 600 MB are free). To rerun by
hand from a scratch worktree: `flock /root/.ti4-heavy.lock env LOCK=0 JOBS=4 tools/engine_core/compute_bench.sh
profiles WORK` and `... seed WORK`. Until they exist, the small-crate table is the evidence for the mechanism; the
expected effect at engine scale is: a seeded build compiles only the changed crates (the `ti4-engine` rebuild
remains, it is the crate that changed), so the saving is the registry dependencies plus every unchanged workspace
crate, not the engine itself.

The seed itself takes under a second here and adds 136 MB (the copied metadata) next to the 309 MB warm dir.

## 3. Lighter test profile

`[profile.lockstep]` (inherits `gate`, `opt-level = 1`, 64 codegen units) and `[profile.lockstep0]` (inherits `gate`,
`opt-level = 0` for the workspace crates, dependencies stay at `opt-level 3`, 256 codegen units) were added to
`Cargo.toml`. Both keep `debug-assertions` and `overflow-checks` OFF like `gate` and `release`, so the tests see the
same arithmetic as the gate binaries; only the code generation differs. The existing `gate` profile and
`verify_package.sh` are untouched. Use: `cargo test --profile lockstep -p ti4-engine --lib` (binaries land in
`$CARGO_TARGET_DIR/lockstep/`; a profile has its own artifacts, a warm dir per profile). Decision hashes and bench
results do not depend on the profile (docs/ENGINE_BUILD_SPEED.md), but the gate stays the one that produces the
hashes and the CPU numbers.

Measured with `tools/engine_core/compute_bench.sh profiles WORK` (cold build, test run, incremental rebuild after
touching one file):

| `cargo test --profile P -p ti4-engine-core --lib` (137 tests, 59k lines) | cold build | test run | incremental | result |
|---|---|---|---|---|
| gate | 29.8 s | 0.2 s | 1.9 s | 137 passed |
| lockstep | 27.1 s | 0.2 s | 2.4 s | 137 passed |
| lockstep0 | 40.0 s | 0.3 s | 1.9 s | 137 passed |

At this size the build is dominated by the dependencies and the crate is too small to show a difference: the
interesting numbers are those of the 335k-line `ti4-engine`.

**Engine-scale numbers: PENDING** (same queued job as in section 2: `tools/engine_core/compute_bench.sh profiles WORK`
with `PKG=ti4-engine`; results in `/root/deploytmp/core-compute/engine.out`). Choose by those numbers; until then
use `lockstep` only for loops where a failure would be a test failure, never for a gate or a timing claim.

## 4. Concurrent benches and tests

**Benches.** `tools/engine_profile/verify_benches.sh BIN [WORKERS]` ran `bench`, `bench-v5`, `bench-v4`, `bench-v1`,
`bench-v2`, `bench-v3` one after the other. It now runs them concurrently, `JOBS` at a time (default 3; `JOBS=1` is
the old behaviour), splits the `WORKERS` budget (default 6) over the running benches (`ceil(WORKERS / JOBS)` each,
`BENCH_WORKERS` overrides), and prints the SAME six lines in the SAME order when all have finished, so
`verify_package.sh` / `verify_stage.sh` (which count `200 identical, 0 different`) need no change. `HEAVY_LOCK=1` makes
the script take `/root/.ti4-heavy.lock` itself (default off: the gates that call it already hold the lock and a
child would wait for its parent). `ONLY="bench bench-v5"` runs a subset; `RB_DIR` points at a directory with other
configs (used for the test below).

Identical-output check (small configs: seeds 1..3, 4 games per bench, same binary, the machine at load 6-10):

| | JOBS=1 | JOBS=3 | JOBS=6 |
|---|---|---|---|
| wall for the six benches | 43.8 s | 27.1 s | 26.2 s |
| the six result lines up to `\|` | `4 identical, 0 different, 0 not in the baseline` x6, md5 393ec45f... | same md5 | same md5 |

Full-size (200 games per bench) JOBS=1 against JOBS=3 with the real configs: **PENDING**, queued behind the heavy lock
(`/root/deploytmp/core-compute/bench_full.sh`, output `benchfull.out`; sequential is about 3.5 min per bench at
6 workers). The output lines are produced by the same code as in the small check, so only the wall time is open.

**Test suites.** `cargo-nextest` is not installed and not in the registry cache, so there is no nextest partitioning.
`tools/engine_core/test_parallel.sh [cargo test args]` is the replacement: one `cargo test --no-run` build, then all
test executables (lib and integration tests, not doc tests) run concurrently (`JOBS`, default 3, `THREADS` per
executable, default 4), one result line each and the names of failed tests with the tail of their output. Checked on
`-p ti4-model -p ti4-content -p ti4-engine-core` (5 executables, 0 failures). It is not wired into
`verify_package.sh` (the gate's check 1 uses `cargo test -p ...`; swap it in there if the gate's test phase is
the bottleneck). Within one executable libtest already uses all cores; the benefit is across the many small
executables and for the tail of a single slow one.

## Paragraph for agent briefs

Paste into porting-agent briefs:

> Compute tools (docs/ENGINE_COMPUTE.md). (1) After you port or change a core module, run
> `tools/engine_core/mutants.sh <module e.g. query::tech> <test filter words>` (add `TEST_CRATE=ti4-engine-core-bridge
> TEST_ARGS="--test <name>"` to run the adapter's equivalence tests against the mutated core; `FN=a,b` limits it to
> functions). One build, every mutant run against your tests in seconds; it prints `SURVIVED`/`killed` per mutant with
> file:line and the mutated expression. Add a test for each survivor or mark a truly equivalent one `// mutants:skip`
> with a reason; put the survivor count in your report. It works on a copy, your checkout is never modified; delete
> `/root/deploytmp/mutants-*/target` when done (disk is almost full). (2) Do not cold-build: `export
> CARGO_TARGET_DIR=<private dir>; tools/engine_core/target_dir.sh seed $CARGO_TARGET_DIR` first (the coordinator keeps
> the warm dir `/root/deploytmp/warm-target` fresh with `target_dir.sh warm`); delete your dir with `target_dir.sh rm`
> when done. (3) For test loops use `cargo test --profile lockstep ...` instead of the gate profile (same results,
> faster to build); verify_package.sh stays on `gate`. (4) `tools/engine_profile/verify_benches.sh BIN [WORKERS]` runs the six
> benches concurrently (`JOBS`, default 3); `tools/engine_core/test_parallel.sh -p a -p b` runs test executables
> concurrently. Take `/root/.ti4-heavy.lock` for anything longer than 5 minutes and keep `CARGO_BUILD_JOBS=2`.
