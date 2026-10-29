#!/usr/bin/env bash
# Reproduce one soak difference as a single game with full detail (docs/ENGINE_SOAK.md, "Repro").
#
#   tools/engine_core/soak_repro.sh --line 'DUAL_RUN CONFIG GAMES "MASK" --seed S' [options]
#   tools/engine_core/soak_repro.sh --record SLICE_DIR/result.json N [options]     (N-th difference, 0-based; hash changes: --hash N)
#   tools/engine_core/soak_repro.sh --config CONFIG --games G --mask MASK (--seed S | --game I) [options]
#
# Options: --bin DUAL_RUN (default: the binary of the record/line, or $DUAL_RUN), --failfast (TI4_DUAL_FAIL_FAST=1 and a
# backtrace: the first difference panics at its call site), --nested (print the nested-activity census too).
# Needs no lock: it plays ONE game twice (dual, then string), well under a minute. It prints the game (seed, map, factions,
# the `selfplay replay` flags) and then exactly what `dual_run` prints for it: every difference with both sides.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
bin="${DUAL_RUN:-}"; line=""; record=""; which=0; kind="diff"; config=""; games=""; mask=""; sel=(); failfast=0; nested=0
while (($#)); do
  case "$1" in
    --line) line="$2"; shift 2 ;;
    --record) record="$2"; which="$3"; shift 3 ;;
    --hash) kind="hash"; shift ;;
    --config) config="$2"; shift 2 ;;
    --games) games="$2"; shift 2 ;;
    --mask) mask="$2"; shift 2 ;;
    --seed) sel=(--seed "$2"); shift 2 ;;
    --game) sel=(--game "$2"); shift 2 ;;
    --bin) bin="$2"; shift 2 ;;
    --failfast) failfast=1; shift ;;
    --nested) nested=1; shift ;;
    *) echo "unknown argument $1" >&2; exit 2 ;;
  esac
done
if [[ -n "$record" ]]; then
  line="$(python3 -I - "$record" "$which" "$kind" <<'PY'
import json, sys
r = json.load(open(sys.argv[1]))
rows = r["hash_changes"] if sys.argv[3] == "hash" else r["differences"]
print(rows[int(sys.argv[2])]["repro"])
PY
)" || { echo "no such record" >&2; exit 2; }
fi
if [[ -n "$line" ]]; then
  mapfile -d '' parts < <(python3 -I -c 'import shlex,sys; sys.stdout.write("\0".join(shlex.split(sys.argv[1])))' "$line")
  bin_in_line="${parts[0]}"; config="${parts[1]}"; games="${parts[2]}"; mask="${parts[3]}"; sel=("${parts[@]:4}")
  [[ -z "$bin" ]] && bin="$bin_in_line"
fi
[[ -n "$bin" && -n "$config" && -n "$games" && -n "$mask" && ${#sel[@]} -eq 2 ]] || { sed -n 2,12p "$0" >&2; exit 2; }
[[ -x "$bin" ]] || { echo "dual_run binary not found: $bin (set DUAL_RUN or --bin)" >&2; exit 2; }
[[ -f "$config" ]] || { echo "config not found: $config" >&2; exit 2; }
echo "== game =="
"$bin" --specs "$config" "$games" | python3 -I -c '
import json, sys
flag, val = sys.argv[1], sys.argv[2]
for l in sys.stdin:
    g = json.loads(l)
    if (flag == "--seed" and str(g["seed"]) == val) or (flag == "--game" and str(g["index"]) == val):
        print(json.dumps({k: v for k, v in g.items() if k != "selfplay_flags"}))
        print("replay flags:", g["selfplay_flags"], "(bot overrides:", g["bot"], ")")
' "${sel[0]}" "${sel[1]}"
echo "== dual run: $bin $config $games \"$mask\" ${sel[*]} =="
env_extra=()
((failfast)) && env_extra+=(TI4_DUAL_FAIL_FAST=1 RUST_BACKTRACE=1)
((nested)) && env_extra+=(TI4_NESTED_LOG=1)
env "${env_extra[@]}" "$bin" "$config" "$games" "$mask" "${sel[@]}"
rc=$?
echo "== exit $rc (0: no difference; 1: a difference or hash change; 2: usage) =="
exit $rc
