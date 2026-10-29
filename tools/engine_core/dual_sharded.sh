#!/usr/bin/env bash
# The golden dual run as parallel shards (docs/ENGINE_CORE_STATE.md 5, "Sharded dual run").
#
#   tools/engine_core/dual_sharded.sh DUAL_RUN CONFIG GAMES AREAS SHARD_DIR
#
# Env: SHARDS (default 4; 1 = the old single process `dual_run CONFIG GAMES AREAS`, no files).
#
# Starts `dual_run CONFIG GAMES AREAS --shard i/N --out SHARD_DIR/shard-i.json` for i in 0..N-1 at the same
# time (the caller holds the heavy lane, /root/.ti4-heavy.lock; the shards run inside it), waits for all of
# them, then `dual_run --merge` prints exactly the text the single process prints (games / areas on dual /
# hashes equal / `dual: N calls, ...` / per-area and windows lines / differences). Games go to shards by
# index % N, so the merge is the unsharded report (counters, differences in game order, per-game hash verdict).
# A shard that fails (exit status other than 0, e.g. a fail-fast panic or a difference) is named first in the
# output with the tail of its log (SHARD_DIR/shard-i.log); the merge then refuses if its file is missing.
# Exit 1 when any shard or the merge failed, 0 otherwise.
set -uo pipefail
bin="${1:?DUAL_RUN}"; config="${2:?CONFIG}"; games="${3:?GAMES}"; areas="${4:?AREAS}"; dir="${5:?SHARD_DIR}"
n="${SHARDS:-4}"
if ! [[ "$n" =~ ^[0-9]+$ ]] || (( n < 1 )); then echo "SHARDS must be a positive integer (got '$n')" >&2; exit 2; fi
if (( n == 1 )); then exec "$bin" "$config" "$games" "$areas"; fi
mkdir -p "$dir"; rm -f "$dir"/shard-*.json "$dir"/shard-*.log
pids=()
for ((i = 0; i < n; i++)); do
  "$bin" "$config" "$games" "$areas" --shard "$i/$n" --out "$dir/shard-$i.json" > "$dir/shard-$i.log" 2>&1 &
  pids+=("$!")
done
fail=0
for ((i = 0; i < n; i++)); do
  wait "${pids[$i]}"; rc=$?
  if (( rc != 0 )); then
    fail=1
    echo "SHARD $i/$n FAILED (exit $rc; log $dir/shard-$i.log):"
    tail -n 5 "$dir/shard-$i.log" | cut -c1-300 | sed 's/^/    /'
  fi
done
files=()
for ((i = 0; i < n; i++)); do [[ -f "$dir/shard-$i.json" ]] && files+=("$dir/shard-$i.json"); done
if (( ${#files[@]} == 0 )); then echo "MERGE REFUSED: no shard file was written"; fail=1
else "$bin" --merge "${files[@]}" || fail=1; fi
exit "$fail"
