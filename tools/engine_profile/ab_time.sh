#!/usr/bin/env bash
# CPU time of two selfplay binaries on the same games, interleaved (A B A B ...), and whether
# they played the same games (every game's decision hash).
#
#   tools/engine_profile/ab_time.sh BIN_A BIN_B [CONFIG] [REPEATS] [WORKERS]
#
# CONFIG defaults to tools/engine_profile/profile-20.json (10 4p + 10 6p golden games). CPU time
# (user + sys of the whole process tree, /usr/bin/time) is what is compared: the machine is
# shared, so wall time is noisy. Prints one line per run and the medians.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
a="$1"; b="$2"; config="${3:-$here/profile-20.json}"; reps="${4:-3}"; workers="${5:-4}"
out="${AB_OUT:-$(mktemp -d)}"; mkdir -p "$out"
export SELFPLAY_RESULTS=off
for ((i=1; i<=reps; i++)); do
  for side in A B; do
    bin=$a; [[ $side == B ]] && bin=$b
    /usr/bin/time -f "%U %S %e" -o "$out/t$side$i" "$bin" run --config "$config" --no-cache \
      --workers "$workers" --out "$out/$side$i.json" --quiet --no-table >/dev/null 2>&1
    read -r u s e < "$out/t$side$i"
    echo "$side run $i: cpu $(echo "$u + $s" | bc) s (user $u sys $s) wall $e s"
  done
done
python3 -I - "$out" "$reps" <<'PY'
import json, statistics, sys
out, reps = sys.argv[1], int(sys.argv[2])
def cpu(side):
    v = []
    for i in range(1, reps + 1):
        u, s, e = open(f"{out}/t{side}{i}").read().split()
        v.append(float(u) + float(s))
    return v
def hashes(side):
    g = json.load(open(f"{out}/{side}1.json"))["games"]
    return {(x["spec"]["setup"], x["spec"]["seed"], x["spec"]["map"]): x["hash"] for x in g}
a, b = cpu("A"), cpu("B")
ha, hb = hashes("A"), hashes("B")
same = sum(1 for k in ha if hb.get(k) == ha[k])
ma, mb = statistics.median(a), statistics.median(b)
print(f"median cpu A {ma:.1f} s, B {mb:.1f} s: B/A = {mb/ma:.3f} ({100*(1-mb/ma):+.1f}% less CPU)")
print(f"games with the same decision hash: {same}/{len(ha)}")
PY
