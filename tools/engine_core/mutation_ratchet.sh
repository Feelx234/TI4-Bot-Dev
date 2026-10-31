#!/usr/bin/env bash
# Mutation baseline and ratchet for the typed core (docs/todo/D4-core-tests.md 6.3/6.4, docs/D4_MUTATION_BASELINE.md).
#
#   tools/engine_core/mutation_ratchet.sh [MODULE...]       run the modules, compare with tools/engine_core/mutation_floor.json
#   MODE=baseline tools/engine_core/mutation_ratchet.sh ... run them and write the summaries, never fail below a floor
#   UPDATE=1 tools/engine_core/mutation_ratchet.sh ...      afterwards RAISE the floors to the measured scores (never lowers)
#   LIST=1 tools/engine_core/mutation_ratchet.sh            list the module names
#
# MODULE is a name of tools/engine_core/d4/mutation_modules.tsv (`core/query`, `factions/cb_economy`, ...); none = all.
# Each module = one `mutants.sh` run per test crate of its row (unioned: a mutant is killed when any run kills it).
#   EXTRA_TEST_CRATES="ti4-engine-core-tests"   run those crates' tests as well (the D4 kit; the floors of a package are
#                                                measured with the tests it wrote, so a package passes its own list)
#   JOBS (4)   CARGO_BUILD_JOBS (2)   KINDS   STRICT   passed to mutants.sh;  OUT (summaries; default $ROOT/results)
#   ROOT (default /root/deploytmp/core-d4b): scratch dir of mutants.sh ($ROOT/mut) and the results.
# Exits 1 when a module is below its floor (MODE=check, the default). Prints the survivors of a module below its floor;
# every module's full survivor list is written to $OUT/<name>.survivors.txt. The scratch build ($ROOT/mut/target) is
# deleted at the end of the call (disk is tight); the summaries stay.
# An equivalent mutant is marked `// mutants:skip <reason>` in the source (not counted).
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
mods="$here/d4/mutation_modules.tsv"
floors="${FLOORS:-$here/mutation_floor.json}"
root="${ROOT:-/root/deploytmp/core-d4b}"
out="${OUT:-$root/results}"
mode="${MODE:-check}"
mkdir -p "$out"
# The D4 kit appends one line per covers! to $D4_COVERS_FILE, else $CARGO_TARGET_DIR/d4-covers.txt (which a caller such as
# end_of_d1.sh also reads for the coverage check). A mutation run executes the D4 tests once per mutant, so without this the file
# grew by gigabytes per hour (2 GB in 25 minutes of the end-of-D1 ratchet, torn lines from the parallel jobs included) and
# drowned the real registry of the `tests` step. The registry of a mutation run is of no use: send it to /dev/null.
export D4_COVERS_FILE="${D4_COVERS_FILE:-/dev/null}"

if [[ "${LIST:-0}" == 1 ]]; then grep -v '^#' "$mods" | cut -f1; exit 0; fi
names=("$@")
if [[ ${#names[@]} == 0 ]]; then mapfile -t names < <(grep -v '^#' "$mods" | cut -f1); fi

summaries=()
for name in "${names[@]}"; do
  row="$(grep -v '^#' "$mods" | awk -F'\t' -v n="$name" '$1==n')"
  [[ -n "$row" ]] || { echo "unknown module $name (LIST=1 lists them)"; exit 2; }
  # awk, not `read`: tab is IFS whitespace and would swallow an empty exclude field
  f() { awk -F'\t' -v n="$name" -v i="$1" '$1==n {print $i}' "$mods"; }
  crate="$(f 2)"; target="$(f 3)"; exclude="$(f 4)"; tests="$(f 5)"; targs="$(f 6)"
  fname="${name//\//_}"
  reports=()
  t0=$SECONDS
  IFS=',' read -ra tcs <<< "$tests${EXTRA_TEST_CRATES:+,${EXTRA_TEST_CRATES// /,}}"
  for tc in "${tcs[@]}"; do
    rep="$out/$fname.$tc.report.json"
    rm -f "$rep" "$root/mut/report.json"
    echo "== $name: mutants of $target (exclude: ${exclude:-none}) against the tests of $tc"
    MUT_DIR="$root/mut" CRATE="$crate" TEST_CRATE="$tc" TEST_ARGS="$targs" EXCLUDE="$exclude" QUIET=1 STRICT=0 \
      JOBS="${JOBS:-4}" CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}" ${KINDS:+KINDS="$KINDS"} \
      nice -n 10 "$here/mutants.sh" "$target" > "$out/$fname.$tc.log" 2>&1
    rc=$?
    tail -3 "$out/$fname.$tc.log" | cut -c1-300
    if [[ $rc -ne 0 || ! -f "$root/mut/report.json" ]]; then
      echo "FAILED $name against $tc (rc $rc): see $out/$fname.$tc.log"; tail -12 "$out/$fname.$tc.log" | cut -c1-300
      continue
    fi
    cp "$root/mut/report.json" "$rep"
    reports+=(--report "$rep")
  done
  if [[ ${#reports[@]} -eq 0 ]]; then echo "no report for $name"; continue; fi
  python3 -I "$here/d4/mutation_report.py" summarize --module "$name" "${reports[@]}" --wall $((SECONDS - t0)) \
    --out "$out/$fname.summary.json" --survivors "$out/$fname.survivors.txt"
  summaries+=(--summary "$out/$fname.summary.json")
done
rm -rf "$root/mut/target"

[[ ${#summaries[@]} -gt 0 ]] || exit 2
rc=0
if [[ "$mode" != baseline ]]; then
  python3 -I "$here/d4/mutation_report.py" check --floors "$floors" "${summaries[@]}" || rc=1
fi
if [[ "${UPDATE:-0}" == 1 ]]; then
  python3 -I "$here/d4/mutation_report.py" raise --floors "$floors" "${summaries[@]}"
fi
exit $rc
