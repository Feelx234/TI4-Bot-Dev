#!/usr/bin/env bash
# The combined heavy run for the end of D1: ONE script, ONE hold of the heavy lane (docs/INTEGRATION6.md 6).
#
#   tools/engine_core/end_of_d1.sh --dry-run                  print the steps (and what each runs); touches nothing, takes no lock
#   CARGO_TARGET_DIR=/root/deploytmp/eod tools/engine_core/end_of_d1.sh [OPTIONS]
#
# Options:  --out DIR       logs and the summary (default /root/end-of-d1-<UTC date>)
#           --only ID..     run only these steps (ids below, comma or space separated)
#           --skip ID..     run all but these steps
#           --from ID       start at this step (a resumed run)
#           --base-ref REF  the ref the `lint` step compares the immutable surfaces against (also env BASE_REF)
#           --dry-run       as above
# Env:      ZERO_BAIL (report|enforce, the zero-bail step), CARGO_TARGET_DIR (required except for --dry-run: private, gate/lockstep/dev profiles are built into it),
#           SHARDS (default 3: two gates fit on 8 cores), WORKERS (benches, default 6), GAMES (golden dual games, default 20),
#           ALPHA_SRC (default /var/lib/ti4-public), SOAK_GAMES (default 40), BASE_REF (immutable-surface check; default core-integration-6, else origin/core-integration-6, else the step FAILS and says so)
#
# The lane: the script re-executes itself under /root/heavy2.sh (the two-slot lock) and holds it for the whole run. Never start it
# outside a quiet window and never together with another gate that holds both slots. Nothing here edits the checkout; the
# only writes are $CARGO_TARGET_DIR, $OUT and, for the alpha round trip, a temporary COPY of the alpha data (the live
# directory is refused by the tool).
#
# A step that fails does not stop the run (the later steps are independent; `tests` fails first when the build is broken): the
# summary names every failure and the exit status is 1. Order (as decided): layers/lint, tests, sharded golden dual
# (driver=dual, the wide mask, every area), board_equivalence, core_sync_gate --strict, alpha_roundtrip, benches 200x6 under
# TI4_CHURN off / resolver / state / resolver,state, the golden lent census, the soak corpus, the mutation ratchet LAST (it is
# the longest and its scratch builds are the biggest).
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"

dry=0; only=(); skip=(); from=""; out=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) dry=1 ;;
    --out) out="${2:?--out DIR}"; shift ;;
    --base-ref) BASE_REF="${2:?--base-ref REF}"; shift ;;
    --from) from="${2:?--from ID}"; shift ;;
    --only) shift; while [[ $# -gt 0 && "$1" != --* ]]; do IFS=, read -ra p <<< "$1"; only+=("${p[@]}"); shift; done; continue ;;
    --skip) shift; while [[ $# -gt 0 && "$1" != --* ]]; do IFS=, read -ra p <<< "$1"; skip+=("${p[@]}"); shift; done; continue ;;
    -h|--help) sed -n '2,25p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "unknown argument $1 (see --help)" >&2; exit 2 ;;
  esac
  shift
done

out="${out:-/root/end-of-d1-$(date -u +%Y%m%d)}"
if [[ $dry == 0 && -z "${END_OF_D1_LOCKED:-}" ]]; then
  : "${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR to a private target dir}"
  export END_OF_D1_LOCKED=1
  re=(--out "$out")
  [[ -n "$from" ]] && re+=(--from "$from")
  [[ -n "${BASE_REF:-}" ]] && re+=(--base-ref "$BASE_REF")
  [[ ${#only[@]} -gt 0 ]] && re+=(--only "${only[@]}")
  [[ ${#skip[@]} -gt 0 ]] && re+=(--skip "${skip[@]}")
  exec /root/heavy2.sh "${BASH_SOURCE[0]}" "${re[@]}"
fi

: "${SHARDS:=3}" "${WORKERS:=6}" "${GAMES:=20}" "${ALPHA_SRC:=/var/lib/ti4-public}" "${SOAK_GAMES:=40}" "${BASE_REF:=}"
T="${CARGO_TARGET_DIR:-<CARGO_TARGET_DIR>}"
CFG="tools/engine_profile/profile-20.json"
WIDE="driver=dual,invasion=dual,production=dual,leaders=dual,exploration=dual,agenda=dual"
D4FLAGS=""
for p in ti4-engine-core ti4-engine-core-factions ti4-engine-core-tests; do
  D4FLAGS+=" --config profile.lockstep.package.$p.debug-assertions=true --config profile.lockstep.package.$p.overflow-checks=true"
done

# ---- the steps: id | title | command (run with `bash -c` in the repo root, output to $out/<id>.log) ---------------------------
ids=(); titles=(); cmds=()
step() { ids+=("$1"); titles+=("$2"); cmds+=("$3"); }

step layers   "layers of the core (check_layers.py core, engine report)" \
  "python3 -I tools/engine_core/check_layers.py core && { python3 -I tools/engine_core/check_layers.py engine > $out/layers-engine.txt 2>&1 || true; }"
# BASE_REF: the flag/env value, else core-integration-6 (local), else origin/core-integration-6; none of them = the step fails.
step lint     "core hygiene (lint_core.sh) and the immutable surfaces against ${BASE_REF:-core-integration-6}" \
  "base='$BASE_REF'; if [[ -z \$base ]]; then for r in core-integration-6 origin/core-integration-6; do git rev-parse -q --verify \$r^{commit} >/dev/null && { base=\$r; break; }; done; fi; \
   [[ -n \$base ]] && git rev-parse -q --verify \$base^{commit} >/dev/null || { echo \"lint: no usable base ref (\$base; tried core-integration-6 and origin/core-integration-6): pass --base-ref REF\"; exit 1; }; \
   echo \"lint: immutable surfaces against \$base\"; \
   tools/engine_core/lint_core.sh && git diff --exit-code \$base -- web/ docs/RULES_VERSION.md crates/ti4-model/src/state.rs"
step build    "gate-profile binaries (selfplay, every bridge bin)" \
  "cargo build --profile gate -p ti4-selfplay -p ti4-engine-core-bridge --bins && cp $T/gate/selfplay $out/selfplay-new"
step clippy   "clippy gate (clippy_core.sh): cargo's exit status, no warning anywhere in the core, factions, bridge, test and typed-bot crates, no new one in the engine" \
  "tools/engine_core/clippy_core.sh $out/clippy.txt"
step tests    "tests: packages, the engine suite with every area on Dual (fail-fast), the D4 crate with its profile, gen-cards, xtask, wasm" \
  "cargo test -p ti4-model -p ti4-content -p ti4-engine -p ti4-host -p ti4-rule-bot -p ti4-selfplay -p ti4-engine-core -p ti4-engine-core-factions -p ti4-engine-core-bridge -p ti4-rule-bot-typed \
   && TI4_AREAS=all=dual TI4_DUAL_FAIL_FAST=1 cargo test --profile lockstep -p ti4-engine --lib \
   && rm -f $T/d4-covers.txt && cargo test --profile lockstep -p ti4-engine-core-tests --no-fail-fast $D4FLAGS \
   && cargo run -q -p xtask -- gen-cards --check && cargo test -p xtask && cargo test -p ti4-wasm --features ts"
step dual-all "sharded golden dual, every area on Dual (driver included), bail probe on" \
  "rm -f $out/bail-probe.tsv; TI4_BAIL_PROBE=$out/bail-probe.tsv TI4_DUAL_FAIL_FAST=1 SHARDS=$SHARDS tools/engine_core/dual_sharded.sh $T/gate/dual_run $CFG $GAMES all=dual $out/dual-all-shards"
step dual-drv "sharded golden dual, driver=dual alone" \
  "TI4_DUAL_FAIL_FAST=1 SHARDS=$SHARDS tools/engine_core/dual_sharded.sh $T/gate/dual_run $CFG $GAMES driver=dual $out/dual-driver-shards"
step dual-wide "sharded golden dual, the wide mask" \
  "TI4_DUAL_FAIL_FAST=1 SHARDS=$SHARDS tools/engine_core/dual_sharded.sh $T/gate/dual_run $CFG $GAMES $WIDE $out/dual-wide-shards"
# The lent set (tools/rulebot/golden-lent.json: the factions with most guards) with the probe on, so the table of lent sites is built
# from 39 games and not only the 20 golden ones (docs/todo/D2-remaining.md 3.1 R6). It counts what the probe sees (the bails of
# `run_dual_window`/`run_dual_ask`); gate rows, nested windows and the `dual_typed!`/`dual!`/`dual::write` None fallbacks are NOT
# in the probe yet (that is the lent_sites.py inventory of D2-5r R3): this step widens the corpus, not the kinds of site.
step dual-lent "sharded dual of the lent set (golden-lent.json), every area on Dual, bail probe on (a second corpus for the census)" \
  "rm -f $out/bail-probe-lent.tsv; TI4_BAIL_PROBE=$out/bail-probe-lent.tsv TI4_DUAL_FAIL_FAST=1 SHARDS=$SHARDS tools/engine_core/dual_sharded.sh $T/gate/dual_run tools/rulebot/golden-lent.json $GAMES all=dual $out/dual-lent-shards"
step board-t  "board_equivalence: map templates" \
  "$T/gate/board_equivalence templates"
step board-g  "board_equivalence: golden positions" \
  "$T/gate/board_equivalence golden $CFG $GAMES 10"
step sync     "core_sync_gate --strict (round trip of >= 3000 positions) and the marks census" \
  "$T/gate/core_sync_gate $CFG $GAMES 10 --strict && python3 -I tools/engine_core/marks_census.py --strict"
step alpha    "alpha_roundtrip --strict on a fresh COPY of the alpha data" \
  "d=\$(mktemp -d) && rsync -a --exclude feedback $ALPHA_SRC/ \$d/ && $T/gate/alpha_roundtrip \$d --strict; rc=\$?; rm -rf \$d; exit \$rc"
for mode in off resolver state resolver,state; do
  id="bench-${mode//,/+}"
  if [[ $mode == off ]]; then env="env -u TI4_CHURN"; else env="env TI4_CHURN=$mode"; fi
  step "$id" "benches bench, bench-v1..v5 (200/200 identical x6) with TI4_CHURN=$mode" \
    "$env VERIFY_OUT=$out/$id tools/engine_profile/verify_benches.sh $out/selfplay-new $WORKERS > $out/$id.txt 2>&1; cat $out/$id.txt; [[ \$(grep -c '200 identical, 0 different' $out/$id.txt) == 6 ]]"
done
step census   "golden lent census: the labels the core bails at (bail probe merged into the table) and the golden games on core / string" \
  "if [[ ! -s $out/bail-probe.tsv ]]; then echo \"census: $out/bail-probe.tsv is missing or empty: it is written by the step dual-all (TI4_BAIL_PROBE); run dual-all first (same --out), or --from dual-all\"; exit 1; fi \
   && $T/gate/bail_table $out/bail-probe.tsv \$( [[ -s $out/bail-probe-lent.tsv ]] && echo $out/bail-probe-lent.tsv ) > $out/bails.rs.txt \
   && $T/gate/golden check --mask all=core && $T/gate/golden check --mask all=string"
# The zero-bail acceptance (user rule: a whole game plays in the new engine without a bail, on the agreed content: everything except
# Thunder's Edge and the dropped factions). Counts every bail and gate by cause in the reports of dual-all and dual-lent (every area on
# Dual, gate binaries under unsupported::Policy::Panic and strict unmapped mode). ZERO_BAIL=report (default for now: the starting list of
# the lanes still porting, exit 0) or ZERO_BAIL=enforce (the acceptance run after lanes 9/10: any cause left fails the step).
# Corpus today: the 20 golden games and the lent set; widening it to every lobby-seatable faction is the soak lane `factions`.
step zero-bail "zero-bail acceptance: bails and gates by cause over the all=dual reports (ZERO_BAIL=${ZERO_BAIL:-report})" \
  "python3 -I tools/engine_core/zero_bail.py \$( [[ \"${ZERO_BAIL:-report}\" == enforce ]] && echo --enforce ) $out/dual-all.log \$( [[ -s $out/dual-lent.log ]] && echo $out/dual-lent.log ) | tee $out/zero-bail.txt"
# Strict product mode (TI4_CORE_STRICT=1: the W17 product assertions, docs/todo/D2-remaining.md finding 5) is exercised by a gate here and
# only here: dual-all itself must stay un-strict (a strict assertion would turn a counted bail into a panic and empty the probe).
step strict   "TI4_CORE_STRICT=1: the golden replays on core and the driver dual over the golden games (0 product assertions)" \
  "TI4_CORE_STRICT=1 $T/gate/golden check --mask all=core && TI4_CORE_STRICT=1 TI4_DUAL_FAIL_FAST=1 SHARDS=$SHARDS tools/engine_core/dual_sharded.sh $T/gate/dual_run $CFG $GAMES driver=dual $out/strict-shards"
step soak     "soak corpus: one round through the dual (every lane, masks all/driver/wide)" \
  "SOAK_HOLDS_LOCK=1 DUAL_RUN=$T/gate/dual_run SHARDS=$SHARDS SLICE_GAMES=$SOAK_GAMES ROUNDS=1 SOAK_DIR=$out/soak SOAK_WORK=$out/soak-work tools/engine_core/soak.sh run > $out/soak-run.txt 2>&1; rc=\$?; cat $out/soak-run.txt; [[ \$rc == 0 ]] && ! grep -q INCOMPLETE $out/soak-run.txt"
step d4gate   "the cheap steps of the D4 gate (no engine in the test crate's default build, coverage check, golden check)" \
  "tools/engine_core/d4_gate.sh 3 4 5"
step ratchet  "mutation ratchet of the core modules (floors in tools/engine_core/mutation_floor.json) -- LAST" \
  "EXTRA_TEST_CRATES=ti4-engine-core-tests ROOT=$out/mut OUT=$out/mutation JOBS=4 tools/engine_core/mutation_ratchet.sh"

# ---- selection ----------------------------------------------------------------------------------------------------------------
want() { # id
  local id="$1" s
  if [[ ${#only[@]} -gt 0 ]]; then for s in "${only[@]}"; do [[ "$s" == "$id" ]] && return 0; done; return 1; fi
  for s in "${skip[@]:-}"; do [[ -n "$s" && "$s" == "$id" ]] && return 1; done
  return 0
}
selected=()
started=0; [[ -z "$from" ]] && started=1
for i in "${!ids[@]}"; do
  [[ "${ids[$i]}" == "$from" ]] && started=1
  [[ $started == 1 ]] && want "${ids[$i]}" && selected+=("$i")
done
if [[ -n "$from" && $started == 0 ]]; then echo "unknown step id for --from: $from" >&2; exit 2; fi
sel() { local i; for i in "${selected[@]}"; do [[ "${ids[$i]}" == "$1" ]] && return 0; done; return 1; }
if [[ $dry == 0 ]] && sel census && ! sel dual-all && [[ ! -s "$out/bail-probe.tsv" ]]; then
  echo "step census needs $out/bail-probe.tsv, written by the step dual-all, which is not selected and has not left one in this --out: run dual-all first (--from dual-all) or point --out at the run that did" >&2; exit 2
fi
if [[ ${#selected[@]} == 0 ]]; then echo "no step selected (ids: ${ids[*]})" >&2; exit 2; fi

if [[ $dry == 1 ]]; then
  echo "end_of_d1.sh --dry-run: ${#selected[@]} of ${#ids[@]} steps; lane: /root/heavy2.sh; SHARDS=$SHARDS GAMES=$GAMES WORKERS=$WORKERS; out=$out; target=$T"
  n=0
  for i in "${selected[@]}"; do
    n=$((n + 1))
    printf '\n%02d. [%s] %s\n    log: %s/%s.log\n    run: %s\n' "$n" "${ids[$i]}" "${titles[$i]}" "$out" "${ids[$i]}" "${cmds[$i]}"
  done
  echo; echo "(dry run: nothing was run, no lock was taken)"
  exit 0
fi

# ---- the run ------------------------------------------------------------------------------------------------------------------
mkdir -p "$out"; cd "$repo" || exit 2
summary="$out/summary.txt"; : > "$summary"
say() { echo "$(date -u +%H:%M:%S) $*" | tee -a "$summary"; }
say "end_of_d1: $(git rev-parse --short HEAD) on $(git rev-parse --abbrev-ref HEAD), target $T, out $out, SHARDS=$SHARDS"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}" CARGO_PROFILE_DEV_DEBUG="${CARGO_PROFILE_DEV_DEBUG:-0}" CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
fail=()
for i in "${selected[@]}"; do
  id="${ids[$i]}"; t0=$SECONDS
  say "start $id: ${titles[$i]}"
  bash -c "set -o pipefail; ${cmds[$i]}" > "$out/$id.log" 2>&1; rc=$?
  if [[ $rc == 0 ]]; then say "ok   $id ($((SECONDS - t0)) s)"; else say "FAIL $id (rc $rc, $((SECONDS - t0)) s, see $out/$id.log)"; fail+=("$id"); fi
done
if [[ ${#fail[@]} == 0 ]]; then say "END OF D1 RUN OK (${#selected[@]} steps)"; exit 0; fi
say "END OF D1 RUN FAILED: ${fail[*]}"
exit 1
