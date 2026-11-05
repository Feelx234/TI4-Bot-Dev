#!/bin/bash
# Cold build of the server bin (what deploy.sh ships) for docs/PERF_FLAGS.md, plus size and the rebuild after a one-file change in ti4-engine-core.
#   run_server.sh <name> <cargo-profile> [RUSTFLAGS]
set -u
NAME=$1; PROF=$2; RF=${3:-}
WT=/root/TI4-Bot-Dev/.claude/worktrees/core-perf-flags
OUTD=/root/deploytmp/flags-data; TD=/root/deploytmp/flags/srv-$NAME
mkdir -p $OUTD/logs
exec 8>/root/.ti4-heavy.lock.2; flock 8
cd $WT; export CARGO_TARGET_DIR=$TD CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0
[ -n "$RF" ] && export RUSTFLAGS="$RF"
rm -rf $TD
build() { /usr/bin/time -f "%e %U %S %M" -o $OUTD/logs/srv-$NAME.$1.time nice -n 15 cargo build -p ti4-server --bin server --profile $PROF --config docs/perf_flags_data/profiles.toml > $OUTD/logs/srv-$NAME.$1.out 2>&1; }
build cold; RC=$?; t=($(tail -1 $OUTD/logs/srv-$NAME.cold.time))
echo "{\"variant\":\"$NAME\",\"kind\":\"server_cold\",\"rc\":$RC,\"wall\":${t[0]},\"user\":${t[1]},\"sys\":${t[2]},\"size\":$(stat -c %s $TD/$PROF/server)}" >> $OUTD/results.jsonl
F=crates/ti4-engine-core/src/lib.rs; cp $F $OUTD/lib.rs.srv.$NAME
echo "pub fn __flags_probe_srv() -> u32 { 1 }" >> $F
build re; t=($(tail -1 $OUTD/logs/srv-$NAME.re.time))
echo "{\"variant\":\"$NAME\",\"kind\":\"server_rebuild\",\"wall\":${t[0]},\"user\":${t[1]},\"sys\":${t[2]}}" >> $OUTD/results.jsonl
cp $OUTD/lib.rs.srv.$NAME $F
rm -rf $TD
