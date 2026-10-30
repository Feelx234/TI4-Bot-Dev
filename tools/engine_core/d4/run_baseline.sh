#!/usr/bin/env bash
# Run the mutation baseline module by module (each module under the heavy lock, so a gate can slip in between), then
# print the markdown table. Meant to be started in the background by one runner:
#   nohup tools/engine_core/d4/run_baseline.sh [MODULE...] > /root/deploytmp/core-d4b/results/batch.log 2>&1 &
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ratchet="$here/../mutation_ratchet.sh"
root="${ROOT:-/root/deploytmp/core-d4b}"; out="${OUT:-$root/results}"
mkdir -p "$out"
names=("$@"); [[ ${#names[@]} == 0 ]] && mapfile -t names < <(LIST=1 "$ratchet")
for n in "${names[@]}"; do
  echo "### $n $(date +%H:%M:%S)"
  flock /root/.ti4-heavy.lock env MODE=baseline ROOT="$root" OUT="$out" "$ratchet" "$n"
done
sums=(); for n in "${names[@]}"; do f="$out/${n//\//_}.summary.json"; [[ -f "$f" ]] && sums+=(--summary "$f"); done
[[ ${#sums[@]} -gt 0 ]] && python3 -I "$here/mutation_report.py" table "${sums[@]}"
echo "### done $(date +%H:%M:%S)"
