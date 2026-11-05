#!/bin/bash
# PGO for docs/PERF_FLAGS.md: instrumented release build, train on games NOT used for the measurements
# (4p seeds 4 to 6 random, 5p seed 1 random, 4p seed 5 smart, replay of it, 4p legacy seed 4), merge, then
# measure `release` + profile-use through run_variant.sh as variant r-pgo.
set -eu
WT=/root/TI4-Bot-Dev/.claude/worktrees/core-perf-flags
PT=/root/deploytmp/llvmtools/llvm-tools-1.94.1-x86_64-unknown-linux-gnu/llvm-tools-preview/lib/rustlib/x86_64-unknown-linux-gnu/bin
P=/root/deploytmp/pgo
OUTD=/root/deploytmp/flags-data
rm -rf $P; mkdir -p $P/raw
(
  flock 8
  cd $WT
  export CARGO_TARGET_DIR=$P/target CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0
  RUSTFLAGS="-C profile-generate=$P/raw" /usr/bin/time -f "%e %U %S %M" -o $OUTD/logs/pgo-instr.time nice -n 15 cargo build --profile release -p ti4-engine-core-bridge --bin perf_ab --bin native_play
  B=$P/target/release
  for s in 4 5 6; do $B/perf_ab core random 4 $s > /dev/null; done
  $B/perf_ab core random 5 1 > /dev/null
  $B/perf_ab legacy random 4 4 > /dev/null
  $B/perf_ab core record:$P/rec-4-5.txt 4 5 > /dev/null
  $B/perf_ab core replay:$P/rec-4-5.txt 4 5 > /dev/null
  $PT/llvm-profdata merge -o $P/merged.profdata $P/raw
  ls -la $P/merged.profdata
  rm -rf $P/target
) 8>/root/.ti4-heavy.lock.2
cd $WT
exec docs/perf_flags_data/run_variant.sh r-pgo release "-C profile-use=$P/merged.profdata -C llvm-args=-pgo-warn-missing-function"
