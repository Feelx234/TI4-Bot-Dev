#!/usr/bin/env bash
# Count calls of named functions with uprobes (needs root and tracefs), e.g.
#   tools/engine_profile/count_calls.sh BIN -- BIN run --config tools/engine_profile/profile-4p.json --workers 1 ...
# Functions are matched on their mangled names (FUNCS below, override with COUNT_FUNCS="a b").
# Offsets come from `nm` minus the text segment's (vaddr - file offset).
set -euo pipefail
bin="$1"; shift; [[ "${1:-}" == "--" ]] && shift
funcs=${COUNT_FUNCS:-"4game4Game4step17h 6timing8Resolver10checkpoint17h 13MovementRules9with_laws17h 8tactical12movable_into17h 7PlanCtx3new17h 8backward7verdict17h 8SelfPlay6decide17h 50_\$LT\$ti4_content..galaxy..Galaxy\$u20\$as\$u20\$core..clone..Clone\$GT\$5clone17h 55_\$LT\$ti4_model..state..GameState\$u20\$as\$u20\$core..clone..Clone\$GT\$5clone17h 11ti4_content5units9catalogue17h 4game4Game12legal_options17h 4game4Game12apply_choice17h"}
tr=/sys/kernel/tracing
read -r vaddr off < <(readelf -lW "$bin" | awk '$1=="LOAD" && / R E / {print strtonum($3), strtonum($2)}')
delta=$(( vaddr - off ))
events=()
i=0
for f in $funcs; do
  line=$(nm "$bin" | grep -E " [tT] " | grep -F -- "$f" | head -1 || true)
  [[ -z "$line" ]] && { echo "not found: $f" >&2; continue; }
  addr=$((16#${line%% *})); name="c$i"; i=$((i+1))
  echo "p:eop/$name $bin:0x$(printf %x $((addr - delta)))" >> $tr/uprobe_events
  events+=(-e "eop:$name"); echo "$name = $(echo "${line##* }" | c++filt 2>/dev/null || echo "${line##* }")" >&2
done
perf stat -x, "${events[@]}" -- "$@" 2>&1 >/dev/null | grep eop || true
for ((j=0; j<i; j++)); do echo "-:eop/c$j" >> $tr/uprobe_events; done
