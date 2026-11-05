# Compiler flags: speed, memory, size and build time of the engine

Lane `core-perf-flags`, cut from `core-integration-25` (3e7a099ab), 2026-11-05. Toolchain 1.94.1. Nothing in the committed profiles changed; this is a measurement and a
recommendation. Machine: 8 cores (AMD EPYC Genoa), shared with heavy run A (`end_of_d1.sh`, slot 1) and other lanes; every measurement held heavy lock slot 2.

## Answer in five lines

1. **The `gate` profile makes the engine 1.5x as many instructions and 1.86x as many cycles as `release`** (same opt-level 3). The cause is the 16 codegen units without LTO
   (no cross-unit inlining): `gate` + `codegen-units = 1` already recovers 3/4 of it, `gate` + thin LTO all of it. The opt-level inside `gate` hardly matters (2 = 3).
2. **The shipped `release` profile (thin LTO, 1 cgu) is already within 3 percent of the best static configuration** (fat LTO: -1.5 percent instructions, +23 percent build time).
   Free flags are worth nothing here: `target-cpu=native` -2 percent instructions, `x86-64-v3` -0.4 percent, overflow checks ON +0.2 percent. `panic = "abort"` gives -3.7 percent
   instructions and -11 percent binary size but is a behaviour change (see Risks).
3. **The only large lever left is PGO: -8 percent instructions and -18 percent cycles over `release`** on engine games (trained on other seeds than measured), with a LOWER
   build CPU time; it costs a build-pipeline step (instrumented build, training run, `llvm-profdata`).
4. **Every variant plays the identical games**: all 7 chains (`perf_ab` x6, `native_play` fingerprint) are byte-identical across all 15 native variants and the 6 wasm variants (node `sha256` of the final state).
   No profile changes a decision.
5. **Memory is flat**: peak RSS of a 4-player game is 28 to 34 MB in every variant (the spread is allocator noise of 1 to 3 MB); it is not a lever.

## Method

* Binaries: `perf_ab` and `native_play` of `ti4-engine-core-bridge` (the `core-perf-kernel` bin `perf_kernel` landed during this lane in a21a6a240 together with code changes; it is NOT
  in the matrix, because the base commit here must stay unchanged. `core-replay-4-1` below is the closest stand-in: the core replaying a recorded smart game, no bot work).
* Variants are custom profiles in `docs/perf_flags_data/profiles.toml` (`cargo build --config docs/perf_flags_data/profiles.toml --profile <name>`) over `gate` (g-*), `release` (r-*),
  `wasm-release` (w-*); RUSTFLAGS only for target-cpu and PGO. All have `debug-assertions = false`, `overflow-checks = false` (the release defaults) unless the name says otherwise.
* Cold build: private `CARGO_TARGET_DIR` deleted before each variant, `CARGO_INCREMENTAL=0`, `CARGO_BUILD_JOBS=4`, `nice -n 15`, `cargo build -p ti4-engine-core-bridge --bin perf_ab --bin native_play`
  (this also builds the dependency tree and, because the bridge links `ti4-wasm`, its cdylib). Rebuild: a new `pub fn` appended to `crates/ti4-engine-core/src/lib.rs`
  (the lowest crate: everything above it recompiles), no incremental; the "incr" column is the second rebuild with `CARGO_INCREMENTAL=1` after a first one populated the cache (measured for gate, g-o1, g-o2, g-thin, release only).
* Runtime: `perf stat -e instructions:u,cycles:u`, 3 repetitions per workload, medians. Workloads: `perf_ab core random 4 {1,2,3}` (rule-bot random pick; sum shown), `perf_ab legacy random 4 1`,
  `perf_ab core record:F 4 1` (the shipped rule bot with the view, 33 G instructions in gate) and `replay:F 4 1`, `native_play --quick`. Peak RSS from `/usr/bin/time` (`%M`).
* Script: `docs/perf_flags_data/run_variant.sh` (one variant), `run_all.sh`, `run_wasm.sh`, `run_server.sh`, `run_pgo.sh`, `aggregate.py` (tables). Raw data: `docs/perf_flags_data/results.jsonl`
  (one line per measurement, with the load average at the start), `tables.md` (all tables), `chains.txt` (the chain check).

### How noisy is it (honest)

* **Instruction counts are exact**: the 3 repetitions differ by at most 0.008 percent, load does not move them. Conclusions below rest on them.
* Cycles move with the neighbours (SMT siblings, cache): median spread of the 3 repetitions 5 percent, worst 20 percent. Only cycle differences above about 5 percent mean something
  (so "g-o2 has 5 percent fewer cycles than gate" is NOT a finding; the 1.86x gate/release is).
* Wall times of builds are noisy by tens of percent: the load average at the start of the cold builds was 1.1 to 14.8 (run A, other lanes). **CPU seconds (user+sys) of builds are the
  comparable figure** (a few percent). Build wall at 4 jobs is given for orientation only.

## Results: native (perf_ab + native_play, 4 jobs)

"instr" and "cycles" are the sum over the three `core random 4p` games (seeds 1 to 3), relative to `gate`. "user s" is the summed user time of those three games. Size is the stripped `perf_ab`.

| variant | what | cold wall s | cold CPU s | rebuild wall s (no incr) | rebuild wall s (incr) | size MB | instr G | instr rel | cycles rel | user s | peak RSS MB |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| **gate** | opt3, no LTO, 16 cgu (today) | 170 | 664 | 147 | **16** | 39.8 | 14.60 | 1.000 | 1.000 | 2.06 | 33.1 |
| g-o1 | gate, opt-level 1 | 488* | 576 | 260 | 15 | 42.6 | 17.37 | 1.190 | 1.229 | 2.56 | 34.3 |
| g-o2 | gate, opt-level 2 | 167 | 652 | 142 | 17 | 39.5 | 14.82 | 1.015 | 0.953 | 1.99 | 33.0 |
| g-os | gate, opt-level s | 134 | 520 | 110 | | 35.3 | 16.50 | 1.130 | 1.140 | 2.37 | 32.2 |
| g-oz | gate, opt-level z | 121 | 469 | 100 | | 33.7 | 20.57 | 1.409 | 1.483 | 3.12 | 32.1 |
| g-cgu1 | gate, 1 cgu | 393 | 950 | 324 | | 36.4 | 11.08 | 0.759 | 0.638 | 1.35 | 31.2 |
| g-thin | gate, thin LTO (16 cgu) | 347 | 1377 | 325 | 206 | 40.3 | 10.07 | 0.690 | 0.591 | 1.25 | 33.3 |
| g-fat | gate, fat LTO (16 cgu) | 433 | 1189 | 410 | | 35.7 | 9.82 | 0.673 | 0.556 | 1.17 | 34.6 |
| **release** | thin LTO, 1 cgu (shipped) | 446 | 1319 | 423 | 334 | 36.7 | 9.81 | 0.672 | 0.537 | 1.14 | 31.8 |
| r-fat | release, fat LTO | 548 | 1233 | 468 | | 34.8 | 9.65 | 0.661 | 0.525 | 1.11 | 34.5 |
| r-abort | release, panic = abort | 442 | 1217 | 340 | | 32.6 | 9.43 | 0.646 | 0.535 | 1.17 | 31.0 |
| r-ovf | release, overflow-checks ON | 432 | 1318 | 368 | | 36.8 | 9.83 | 0.673 | 0.526 | 1.13 | 31.6 |
| r-v3 | release, target-cpu=x86-64-v3 | 442 | 1340 | 386 | | 37.4 | 9.76 | 0.669 | 0.528 | 1.13 | 31.8 |
| r-native | release, target-cpu=native | 461 | 1396 | 400 | | 38.2 | 9.56 | 0.655 | 0.524 | 1.11 | 32.1 |
| **r-pgo** | release + PGO (train: other seeds) | 327 | 1041 | 275 | | 32.0 | 9.00 | 0.616 | 0.440 | 0.96 | 32.7 |

`*` g-o1 started at a load of about 10; its CPU seconds (576) are the figure to read. `debug-assertions` ON was not run as a variant: the engine has 25 `debug_assert!` in the core and a debug build is what `dev` /
`lockstep0` already are; `r-ovf` is the overflow part of that cost.

Instructions per workload, relative to gate (full tables with RSS and wall: `docs/perf_flags_data/tables.md`):

| variant | core random 4p s1 | s2 | s3 | legacy random 4p | core record (smart bot) | core replay | native_play --quick |
|---|---:|---:|---:|---:|---:|---:|---:|
| gate (absolute, G) | 5.14 | 4.73 | 4.73 | 2.10 | 32.89 | 9.94 | 4.14 |
| g-o1 | 1.180 | 1.192 | 1.198 | 1.134 | 1.217 | 1.227 | 1.165 |
| g-o2 | 1.015 | 1.014 | 1.016 | 1.013 | 1.024 | 1.021 | 1.013 |
| g-cgu1 | 0.731 | 0.775 | 0.774 | 0.739 | 0.757 | 0.776 | 0.723 |
| g-thin | 0.675 | 0.685 | 0.711 | 0.732 | 0.741 | 0.745 | 0.657 |
| release | 0.657 | 0.667 | 0.694 | 0.712 | 0.723 | 0.712 | 0.647 |
| r-fat | 0.647 | 0.655 | 0.683 | 0.700 | 0.715 | 0.700 | 0.643 |
| r-abort | 0.633 | 0.639 | 0.668 | 0.696 | 0.703 | 0.685 | 0.638 |
| r-native | 0.642 | 0.648 | 0.677 | 0.709 | 0.711 | 0.698 | 0.629 |
| r-pgo | 0.602 | 0.615 | 0.633 | 0.637 | 0.635 | 0.628 | 0.607 |

Wall per game (median of 3, load 1 to 10 so read as +-10 percent): core random 4p seed 1 `gate` 0.75 s, `release` 0.41 s, `r-pgo` 0.35 s; the smart-bot game (`record`) 5.1 s, 3.05 s, 2.6 s.
Instructions per cycle: gate 1.99, release 2.48, r-pgo 2.78.

### What the numbers say

* **opt-level** (on gate): 3 and 2 are the same (+1.5 percent instructions for 2, cycles within noise); 1 is +19 percent instructions / +23 percent cycles, `s` +13 / +14, `z` +41 / +48.
  Build CPU: O1 is 13 percent cheaper than O3, `s` 22 percent, `z` 29 percent. Size: `s` and `z` shrink the native binary by 11 and 15 percent only. Nothing here beats O3.
* **Codegen units and LTO are what separates gate from release.** The ladder (instructions): gate 1.00, cgu1 0.76, thin(16) 0.69, fat(16) 0.67, release 0.67, release+fat 0.66.
  Build CPU: 664, 950, 1377, 1189, 1319, 1233. So `gate` is cheap exactly because it does not do the work that makes the code fast. `g-fat` (fat LTO over 16 cgu) is as fast as `release`
  with 10 percent less CPU, but it is not incremental either, so it is no gate replacement.
* **Incremental rebuild is the point of gate**: 16 s after touching the lowest core file (vs 147 s without incremental). With thin LTO (g-thin) incremental is 206 s, with `release` 334 s.
* **panic = abort**: -3.7 percent instructions, cycles unchanged within noise, native binary -11 percent, `server` -11 percent (22.6 vs 25.3 MB), build -8 percent CPU.
* **overflow-checks ON** costs +0.2 percent instructions: free in this engine (the hot loops index and count with small types the optimizer can bound).
* **target-cpu**: `x86-64-v3` -0.4 percent, `native` (here AVX-512 Genoa) -2.0 percent instructions, cycles unchanged within noise: the engine is branchy pointer-and-table code with nothing to vectorize.
  And the deploy host is not guaranteed to match, so no.
* **PGO**: -8 percent instructions, -18 percent cycles (IPC 2.48 to 2.78: fewer mispredicted branches and better layout), -6 to -12 percent instructions on each of the 7 workloads, the string engine included (0.712 to 0.637 of gate; it saw one training game). Trained on 4p seeds 4 to 6, 5p seed 1, legacy 4p seed 4, smart 4p seed 5 (record + replay); measured on 4p seeds 1 to 3 and the others above.
  The build is cheaper (1041 vs 1319 CPU s) because cold code is optimized for size, but a PGO release needs the instrumented build (about as long as a release build), a training run (about 1 minute) and `llvm-profdata`.
* **Allocator** (note only, mimalloc stays parked): `perf_ab`, `native_play`, `dual_run`, `selfplay` and the other bridge bins all install mimalloc; `ti4-server` has no `#[global_allocator]` and runs on glibc malloc.
  So none of the numbers above say anything about the allocator, and the server's RSS/CPU cannot be read from them.

## The server binary and the wasm build

`cargo build -p ti4-server --bin server`, cold, 4 jobs (wall noisy, CPU comparable):

| profile | cold wall s | cold CPU s | rebuild after core change, wall s | rebuild CPU s | size MB (stripped) |
|---|---:|---:|---:|---:|---:|
| gate | 96 | 361 | 82 | 264 | 29.0 |
| release (shipped) | 206 | 532 | 191 | 436 | 25.3 |
| r-abort | 187 | 485 | 173 | 399 | 22.6 |

Browser build (`-p ti4-wasm --target wasm32-unknown-unknown`, profiles over `wasm-release`). **No `wasm-opt` here (binaryen is not installed), so these are the files cargo makes,
before the step that `build-wasm.mjs` adds (-19 percent raw, -3 percent brotli).** The shipped, optimized file is 6.43 MB raw / 1.47 MB brotli; budget 6.63 / 2.11 / 1.52 MB (`crates/ti4-wasm/size-budget.json`).
Speed: `node crates/ti4-wasm/js/run.mjs <wasm> 3 2000 4` (2000 steps of a 4-seat random game; module compile + run, 3 repetitions; same `decisions=1955`, same state sha256 in every variant).

| profile | cold wall s | cold CPU s | raw MB | gzip MB | brotli MB | run ms (3 reps) |
|---|---:|---:|---:|---:|---:|---|
| wasm-release (z, fat LTO, abort; shipped) | 83 | 145 | 7.85 | 2.17 | 1.516 | 3074, 3162, 3115 |
| w-thin (z, thin LTO) | 59 | 147 | 7.85 | 2.17 | 1.516 | 2874, 2902, 2981 |
| w-unwind (`panic = "unwind"`) | 59 | 147 | 7.85 | 2.17 | 1.516 | 2914, 2925, 3028 |
| w-os (opt-level s) | 63 | 165 | 8.65 (+10 %) | 2.44 | 1.680 (+11 %) | 2066, 2090, 2022 (**-33 %**) |
| w-o2 | 135 | 283 | 10.66 (+36 %) | 3.31 | 2.145 (+41 %) | 1648, 1717, 1595 (-47 %) |
| w-o3 | 136 | 293 | 11.11 (+41 %) | 3.43 | 2.220 (+46 %) | 1667, 1560, 1613 (-48 %) |

`w-thin` and `w-unwind` are the same file as the shipped one (thin vs fat does nothing at 1 cgu; wasm32-unknown-unknown has no unwinding, so `panic` is moot). The shipped `z` is the smallest and the slowest:
**`s` is 33 percent faster in the browser for +11 percent download (brotli, before wasm-opt: 1.68 MB, over the 1.52 MB budget by about 6 percent after wasm-opt)**; `2`/`3` double the size. That is a product decision (budget vs offline-play speed), not a build detail.

## Risks of each change

| change | gain | risk |
|---|---|---|
| gate opt-level 2 | none | none, but nothing to gain |
| gate -> thin LTO or cgu1 | gate speed x1.4 to 1.5 | incremental rebuild 16 s -> 206 s (thin) or no incremental (cgu1); that is the inner loop. Not recommended for gate. |
| heavy sweeps on `release` instead of `gate` | 1.5x instructions, 1.86x cycles | build 3 to 7 min longer per binary set; hashes identical (checked) |
| release fat LTO | -1.5 % instr, -2 % cycles | +23 % build wall, no incremental either way; not worth it |
| release `panic = "abort"` | -3.7 % instr, -11 % size | **behaviour change**: today a panic in a request task (tokio/axum catches it) drops one connection and the server lives; with abort the process dies and every table with it (the server has no `catch_unwind`, no panic middleware). `ti4-selfplay` (`play.rs:389`) uses `catch_unwind` to report a panicking game, with abort the whole sweep dies. Do not. |
| release `overflow-checks = true` | free in speed (+0.2 %) | makes release differ from gate in the case of an overflow (panic vs wrap); a latent overflow would turn into a server task panic. Only with a soak. Not recommended. |
| `target-cpu=native / v3` | 0 to 2 % | binary may SIGILL on another host. Not recommended. |
| PGO | -8 % instr, -18 % cycles | build pipeline (instrumented build + training + `llvm-profdata` from `llvm-tools-preview`, which is not in `rust-toolchain.toml` components); a stale profile after big code changes silently loses the gain (rustc warns with `-pgo-warn-missing-function`); profile files are build inputs that must be regenerated per release. Decisions unchanged (checked). |
| wasm `s` instead of `z` | browser engine -33 % time | +11 % download, size budget must be raised |

## Recommendation

**(a) `gate` (tests, inner loop, short verification): leave it as it is.** It is the cheapest cold CPU of the optimizing profiles (664 s vs 1319), has a 16 s incremental rebuild, and opt-level 3 vs 2 is a wash.
Do not give it LTO or 1 cgu. What to change is the HABIT, not the profile: **a long run on one commit should use a `release` build** (the HEAVY plan's sweeps, `dual_run` over many games, perf timing -- the docs already say
timing claims must use release). Break-even from the numbers above: release costs about +650 CPU s per set of two bins (+3 to 7 min wall at 4 jobs) and saves 46 percent of the run cycles,
so it pays as soon as a binary will run more than roughly 25 to 30 CPU-minutes on gate (spread over any number of workers); more bins multiply the build cost (each bin repeats the LTO), more runs divide it.
If the build count is the problem, `g-fat` (fat LTO over 16 cgu: release speed, 10 percent less build CPU) is an equal alternative for one-off builds.

**(b) `release` (server, selfplay, wasm-release's native sibling): already near the best.** Everything free is exhausted (fat LTO -1.5 percent, native CPU -2 percent, abort -3.7 percent: none worth its cost or risk).
Do not change the shipped profile. The only change with a real return is **PGO (-18 percent cycles)**; adopt it only if engine CPU is measured to matter on the alpha server (a bot game is 0.4 s, the server's time is
likely I/O, serde and the bot, and the server runs on glibc malloc, which this lane did not measure). If it is wanted: add `llvm-tools-preview` to `rust-toolchain.toml`, a `tools/pgo.sh` doing the three steps of
`docs/perf_flags_data/run_pgo.sh` over a fixed training set of games, and build `server` with `RUSTFLAGS=-Cprofile-use=...` in `deploy.sh`; keep `release` as the fallback when the profile file is missing.
For the browser, the one open question is `opt-level = "s"` vs `"z"` (+11 percent download, -33 percent engine time).

Not done: wasm speed in a real browser (node/V8 only), `perf_kernel` (landed in another lane with code changes), server runtime (no load generator; allocator not varied), PGO with a profile from the server's real mix of games.
