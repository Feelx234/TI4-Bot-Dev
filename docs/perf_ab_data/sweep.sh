#!/bin/bash
B=/root/deploytmp/core-perf/gate/perf_ab
D=/root/deploytmp/core-perf-data
OUT=$D/results.jsonl
run() { # args to perf_ab
  local tries=0 load
  while :; do
    load=$(cut -d' ' -f1 /proc/loadavg)
    if awk "BEGIN{exit !($load < 1.5)}"; then break; fi
    tries=$((tries+1)); [ $tries -ge 60 ] && break; sleep 10
  done
  local l1=$(cut -d' ' -f1 /proc/loadavg)
  local line=$($B "$@")
  local l2=$(cut -d' ' -f1 /proc/loadavg)
  echo "${line%\}},\"load_before\":$l1,\"load_after\":$l2,\"quiet_wait\":$tries}" >> $OUT
}
for p in 3 4 5 6; do for s in 1 2 3 4 5 6; do
  for e in legacy core; do
    run $e random $p $s
    run $e record:$D/$e-$p-$s.rec $p $s
    run $e replay:$D/$e-$p-$s.rec $p $s
  done
done; done
echo done > $D/DONE
