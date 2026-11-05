#!/bin/bash
# One variant of docs/PERF_FLAGS.md: cold build, runtime benches, rebuild after a one-file change.
#   run_variant.sh <name> <cargo-profile> [RUSTFLAGS-string]    (holds heavy lock slot 2 for the whole run)
# Results (one JSON line per measurement) are appended to $OUTD/results.jsonl, raw logs go to $OUTD/logs/.
set -u
NAME=$1; PROF=$2; RF=${3:-}
WT=${WT:-/root/TI4-Bot-Dev/.claude/worktrees/core-perf-flags}
OUTD=${OUTD:-/root/deploytmp/flags-data}
TD=/root/deploytmp/flags/$NAME
JOBS=${JOBS:-4}
case " gate release g-o1 g-o2 g-thin " in *" $NAME "*) INCR=${INCR:-1};; *) INCR=${INCR:-0};; esac   # the incremental pair only for these (it costs two more workspace builds)
mkdir -p "$OUTD/logs"
exec 8>/root/.ti4-heavy.lock.2
flock 8
cd "$WT" || exit 1
export CARGO_TARGET_DIR=$TD CARGO_BUILD_JOBS=$JOBS
[ -n "$RF" ] && export RUSTFLAGS="$RF"
rm -rf "$TD"
CFG=(--config docs/perf_flags_data/profiles.toml)
log() { echo "$1" >> "$OUTD/results.jsonl"; }
tm() { # tm <label> <cmd...> -> prints "wall user sys maxrss_kb" in $T
  /usr/bin/time -f "%e %U %S %M" -o "$OUTD/logs/$NAME.$1.time" "${@:2}" > "$OUTD/logs/$NAME.$1.out" 2>&1
  local rc=$?
  T=$(tail -1 "$OUTD/logs/$NAME.$1.time")
  return $rc
}
loadavg() { cut -d' ' -f1 /proc/loadavg; }

# 1. cold build
L0=$(loadavg)
CARGO_INCREMENTAL=0 tm cold nice -n 15 cargo build --profile "$PROF" "${CFG[@]}" -p ti4-engine-core-bridge --bin perf_ab --bin native_play
RC=$?
set -- $T
log "{\"variant\":\"$NAME\",\"kind\":\"cold_build\",\"rc\":$RC,\"wall\":$1,\"user\":$2,\"sys\":$3,\"load\":$L0,\"jobs\":$JOBS}"
[ $RC -ne 0 ] && { rm -rf "$TD"; exit 1; }
BIN=$TD/$PROF
PA=$BIN/perf_ab; NP=$BIN/native_play
log "{\"variant\":\"$NAME\",\"kind\":\"size\",\"perf_ab\":$(stat -c %s $PA),\"native_play\":$(stat -c %s $NP),\"text_perf_ab\":$(size $PA | awk 'NR==2{print $1}')}"

# 2. runtime
REC=$OUTD/rec-$NAME-4-1.txt
bench() { # bench <label> <cmd...>   3 repetitions under perf stat
  local label=$1; shift
  local cmd=("$@")
  for rep in 1 2 3; do
    local L=$(loadavg)
    /usr/bin/time -f "%e %U %S %M" -o "$OUTD/logs/$NAME.$label.$rep.time" \
      perf stat -x, -e instructions:u,cycles:u -o "$OUTD/logs/$NAME.$label.$rep.perf" "${cmd[@]}" > "$OUTD/logs/$NAME.$label.$rep.out" 2>&1
    local ins=$(grep instructions "$OUTD/logs/$NAME.$label.$rep.perf" | cut -d, -f1)
    local cyc=$(grep cycles "$OUTD/logs/$NAME.$label.$rep.perf" | cut -d, -f1)
    local tt=($(tail -1 "$OUTD/logs/$NAME.$label.$rep.time"))
    local out=$(grep -a '^{' "$OUTD/logs/$NAME.$label.$rep.out" | tail -1 | tr -d '\n')
    [ -z "$out" ] && out=$(tail -c 300 "$OUTD/logs/$NAME.$label.$rep.out" | tr '\n"' '| ')
    echo "{\"variant\":\"$NAME\",\"kind\":\"run\",\"work\":\"$label\",\"rep\":$rep,\"instructions\":${ins:-0},\"cycles\":${cyc:-0},\"wall\":${tt[0]},\"user\":${tt[1]},\"sys\":${tt[2]},\"rss_kb\":${tt[3]},\"load\":$L,\"out\":$(python3 -c 'import json,sys;print(json.dumps(sys.argv[1]))' "$out")}" >> "$OUTD/results.jsonl"
  done
}
bench core-random-4-1 $PA core random 4 1
bench core-random-4-2 $PA core random 4 2
bench core-random-4-3 $PA core random 4 3
bench legacy-random-4-1 $PA legacy random 4 1
bench core-record-4-1 $PA core record:$REC 4 1
bench core-replay-4-1 $PA core replay:$REC 4 1
bench native-quick $NP --quick

# 3. rebuild after a change in ti4-engine-core (a new item appended), no incremental
F=crates/ti4-engine-core/src/lib.rs
cp $F "$OUTD/lib.rs.orig.$NAME"
edit() { echo "pub fn __flags_probe_$1() -> u32 { $1 }" >> $F; }
edit 1
L0=$(loadavg)
CARGO_INCREMENTAL=0 tm rebuild nice -n 15 cargo build --profile "$PROF" "${CFG[@]}" -p ti4-engine-core-bridge --bin perf_ab --bin native_play
RC=$?; set -- $T
log "{\"variant\":\"$NAME\",\"kind\":\"rebuild_noincr\",\"rc\":$RC,\"wall\":$1,\"user\":$2,\"sys\":$3,\"load\":$L0}"
if [ "$INCR" = 1 ]; then
  edit 2
  CARGO_INCREMENTAL=1 tm incr0 nice -n 15 cargo build --profile "$PROF" "${CFG[@]}" -p ti4-engine-core-bridge --bin perf_ab --bin native_play
  RC=$?; set -- $T
  log "{\"variant\":\"$NAME\",\"kind\":\"rebuild_incr_first\",\"rc\":$RC,\"wall\":$1,\"user\":$2,\"sys\":$3}"
  edit 3
  L0=$(loadavg)
  CARGO_INCREMENTAL=1 tm incr1 nice -n 15 cargo build --profile "$PROF" "${CFG[@]}" -p ti4-engine-core-bridge --bin perf_ab --bin native_play
  RC=$?; set -- $T
  log "{\"variant\":\"$NAME\",\"kind\":\"rebuild_incr\",\"rc\":$RC,\"wall\":$1,\"user\":$2,\"sys\":$3,\"load\":$L0}"
fi
cp "$OUTD/lib.rs.orig.$NAME" $F
git -C "$WT" diff --quiet -- $F || echo "WARN: core lib.rs not restored" >&2
rm -rf "$TD"
