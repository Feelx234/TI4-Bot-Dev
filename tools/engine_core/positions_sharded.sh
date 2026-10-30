#!/usr/bin/env bash
# The golden position check of the movement reads as parallel shards (docs/ENGINE_CORE_STATE.md 5).
#
#   tools/engine_core/positions_sharded.sh CORE_EQUIVALENCE CONFIG GAMES STRIDE SHARD_DIR
#
# Env: SHARDS (default 4; 1 = one process). Starts `core_equivalence golden CONFIG GAMES STRIDE --shard i/N` for
# i in 0..N-1 at the same time (the caller holds the heavy lane), waits for all, prints the game hashes of shard 0
# and every shard's summary line. Exit 1 when any shard failed or reported a mismatch.
#
# Why: the string side of the check builds `MovementRules::with_laws` for every (seat, system) of every position, and
# since the faction hooks read the typed state (mark codecs) that costs about 5 s per position, so the old 20 games at
# stride 10 (about 3000 positions) ran for three hours in one process. The gate samples every 40th position (about
# 750) in SHARDS processes and `verify_package.sh` bounds the step (POSITIONS_TIMEOUT).
set -uo pipefail
bin="${1:?CORE_EQUIVALENCE}"; config="${2:?CONFIG}"; games="${3:?GAMES}"; stride="${4:?STRIDE}"; dir="${5:?SHARD_DIR}"
n="${SHARDS:-4}"
if ! [[ "$n" =~ ^[0-9]+$ ]] || (( n < 1 )); then echo "SHARDS must be a positive integer (got '$n')" >&2; exit 2; fi
if (( n == 1 )); then exec "$bin" golden "$config" "$games" "$stride"; fi
mkdir -p "$dir"; rm -f "$dir"/shard-*.log
pids=()
for ((i = 0; i < n; i++)); do
  "$bin" golden "$config" "$games" "$stride" --shard "$i/$n" > "$dir/shard-$i.log" 2>&1 &
  pids+=("$!")
done
fail=0
for ((i = 0; i < n; i++)); do
  wait "${pids[$i]}" || { echo "shard $i failed (exit $?), tail of $dir/shard-$i.log:"; tail -5 "$dir/shard-$i.log"; fail=1; }
done
grep '^game hash' "$dir/shard-0.log"
for ((i = 0; i < n; i++)); do echo "shard $i/$n: $(grep -v '^game hash' "$dir/shard-$i.log" | tail -1)"; done
if grep -q 'mismatches [1-9]' "$dir"/shard-*.log; then fail=1; fi
exit "$fail"
