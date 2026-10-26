#!/usr/bin/env bash
# Interleaved CPU-time A/B of the movement slice (docs/ENGINE_CORE.md, "Measurements"):
# the string engine vs the typed core on the same questions, as tools/engine_profile/ab_time.sh
# does for whole games.
#
#   tools/engine_core/ab_movement.sh BENCH_BIN [MODE_A] [MODE_B] [CONFIG] [GAMES] [STRIDE] [REPEATS] [INNER]
#
# BENCH_BIN is target/release/core_movement_bench (cargo build --release -p ti4-engine-core-bridge).
# Defaults: legacy vs core, tools/engine_profile/profile-4p.json, 4 games, every 10th position,
# 5 interleaved runs (A B A B ...), each answering every question INNER=3 times. Compares the CPU
# time of the timed phase (thread CPU, from the binary) and of the whole process (/usr/bin/time,
# which includes playing the games, the same in both), and that every answer digest is the same.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bin="$1"; a="${2:-legacy}"; b="${3:-core}"
config="${4:-$here/../engine_profile/profile-4p.json}"; games="${5:-4}"; stride="${6:-10}"
reps="${7:-5}"; inner="${8:-3}"
out="${AB_OUT:-$(mktemp -d)}"; mkdir -p "$out"
for ((i=1; i<=reps; i++)); do
  for side in A B; do
    mode=$a; [[ $side == B ]] && mode=$b
    /usr/bin/time -f "%U %S" -o "$out/t$side$i" "$bin" "$mode" "$config" "$games" "$stride" "$inner" > "$out/r$side$i"
    read -r u s < "$out/t$side$i"
    echo "$side ($mode) run $i: $(cat "$out/r$side$i") | process cpu $(echo "$u + $s" | bc) s"
  done
done
python3 -I - "$out" "$reps" "$a" "$b" <<'PY'
import re, statistics, sys
out, reps, a, b = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
def runs(side):
    phase, digests, queries = [], set(), set()
    for i in range(1, reps + 1):
        line = open(f"{out}/r{side}{i}").read()
        phase.append(float(re.search(r"cpu_ms ([0-9.]+)", line).group(1)))
        digests.add(re.search(r"digest (\w+)", line).group(1))
        queries.add(re.search(r"queries (\d+)", line).group(1))
    return phase, digests, queries
pa, da, qa = runs("A"); pb, db, qb = runs("B")
ma, mb = statistics.median(pa), statistics.median(pb)
print(f"timed phase, median of {reps}: A ({a}) {ma:.0f} ms, B ({b}) {mb:.0f} ms: B/A = {mb/ma:.3f} "
      f"({ma/mb:.1f}x), queries {sorted(qa)}")
print(f"same answers (digest): {'yes' if len(da | db) == 1 else 'NO: ' + str(sorted(da | db))}")
PY
