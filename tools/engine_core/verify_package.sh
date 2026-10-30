#!/usr/bin/env bash
# The common gate of one engine-core package (docs/ENGINE_CORE_STATE.md 5; extends verify_stage.sh).
#
#   tools/engine_core/verify_package.sh AREA OUT_DIR BASE_SELFPLAY [WORKERS]
#
# AREA     an `Area` name of ti4-engine/src/dual.rs (movement, board, technology, combat, ...) or `none`
#          for a foundation package (no area switched).
# OUT_DIR  logs and summary.txt.
# BASE_SELFPLAY  a selfplay binary built from the previous merge commit (the CPU A/B baseline).
# Env: EXTRA_DUAL (more areas on Dual beside AREA in checks 3, e.g. `strategy=dual,objectives=dual`), CARGO_TARGET_DIR (required, private), BASE_REF (git ref the immutable surfaces are compared
# with; default engine-core-state-base), GAMES (dual games, default 20), DUAL_BENCH=1 (also run the
# bench with the area on Dual, ~2-3x slower), WASM_OPT (binary of wasm-opt), SKIP_AB=1, SHARDS (default 4:
# the 20-game dual run of check 3 as that many parallel `dual_run --shard i/N` processes whose files are
# merged into the single-run text, tools/engine_core/dual_sharded.sh; 1 = one process as before). The caller
# holds the heavy lane (/root/.ti4-heavy.lock) for the whole gate; the shards run concurrently inside it.
# Builds use the `gate` profile (docs/ENGINE_BUILD_SPEED.md): binaries are in $CARGO_TARGET_DIR/gate/.
# The CPU A/B (6/7) and the wasm size gate (8) are OFF by default (they need release/wasm-release
# builds): RUN_AB=1 / RUN_WASM=1 turn them on. The hash checks of the benches stay.
#
# Checks (numbers = gate table 5): 1 tests + gen-cards --check + wasm tests; 2 immutable surfaces;
# 3 dual run (engine suite with the area on Dual and fail-fast, 20 golden games, templates);
# 5 benches (bench + v1..v5 with the area on Core; bench with it on String; Dual optional);
# 6/7 golden hashes + CPU A/B (AREA on Core vs BASE_SELFPLAY); 8 wasm size; 9 hygiene (layers,
# lint_core, clippy); 10 state_size; 11 build times. The round trip (4): F2's tests inside 1, and since D2-1 the
# strict core_sync_gate over the golden positions, the marks census and the alpha-log round trip as their own steps.
# Exit 1 when anything failed.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
area="${1:?AREA}"; out="${2:?OUT_DIR}"; base="${3:?BASE_SELFPLAY}"; workers="${4:-6}"
target="${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR}"
base_ref="${BASE_REF:-engine-core-state-base}"; games="${GAMES:-20}"
mkdir -p "$out"; : > "$out/summary.txt"
fail=0
note() { echo "$*" | tee -a "$out/summary.txt"; }
# A step that runs longer than STEP_TIMEOUT seconds (default 3600; the position checks use POSITIONS_TIMEOUT, default
# 2400) fails as "timeout": one step must not hold the heavy lane for hours (the lane is the caller's, for the whole gate).
check() { local name="$1"; shift; local t0=$SECONDS cap="${STEP_TIMEOUT:-3600}"
  [[ "$name" == dual-positions ]] && cap="${POSITIONS_TIMEOUT:-2400}"
  timeout --foreground "$cap" "$@" > "$out/$name.log" 2>&1; local rc=$?
  if [[ $rc == 0 ]]; then note "ok   $name ($((SECONDS - t0)) s)"
  elif [[ $rc == 124 ]]; then note "FAIL $name (timeout after ${cap} s, see $out/$name.log)"; fail=1
  else note "FAIL $name (see $out/$name.log)"; fail=1; fi; }
cd "$repo"
if [[ "$area" == none ]]; then on=""; off=""; dual=""; else
  on="TI4_AREAS=$area=core"; off="TI4_AREAS=$area=string"; dual="TI4_AREAS=$area=dual${EXTRA_DUAL:+,$EXTRA_DUAL}"; fi

# 9 hygiene first: cheap.
check layers python3 -I tools/engine_core/check_layers.py core
note "     $(tail -1 "$out/layers.log")"
check lint-core tools/engine_core/lint_core.sh
note "     $(tail -1 "$out/lint-core.log")"
# D4-5: the covers gap of the typed core (windows no test drives); notes only, `--check` without --quick is the D4 gate
check d4-coverage python3 -I tools/engine_core/d4/coverage.py --check --quick
note "     $(head -1 "$out/d4-coverage.log")"
python3 -I tools/engine_core/check_layers.py engine > "$out/layers-engine.txt" 2>&1
note "     string engine: $(sed -n 2p "$out/layers-engine.txt")"
# Clippy: the string engine carries old warnings (counted, not gating); the core crates, the bridge
# and dual.rs must be clean, and nothing may be an error.
cargo clippy -p ti4-engine -p ti4-engine-core -p ti4-engine-core-factions -p ti4-engine-core-bridge --all-targets --message-format short > "$out/clippy.log" 2>&1
clippy_new="$(grep -E '^(crates/ti4-engine-core/|crates/ti4-engine-core-factions/|crates/ti4-engine-core-bridge/|crates/ti4-engine/src/(dual|core_sync/|render/))[^ ]*: (warning|error)' "$out/clippy.log" | wc -l)"
clippy_err="$(grep -c ': error' "$out/clippy.log")"
note "     clippy: $(grep -c ': warning' "$out/clippy.log") warnings in total (old string-engine ones included), $clippy_new in core/bridge/dual/core_sync/render, $clippy_err errors"
if [[ "$clippy_new" == 0 && "$clippy_err" == 0 ]]; then note "ok   clippy"; else note "FAIL clippy (see $out/clippy.log)"; fail=1; fi

# 2 immutable surfaces.
git diff --quiet "$base_ref" -- web/ docs/RULES_VERSION.md crates/ti4-model/src/state.rs \
  'crates/ti4-wasm/**/fixtures' ; rc1=$?
git diff --quiet "$base_ref" -- 'crates/ti4-wasm/**/*.ts' 'web-shared/**/generated' ; rc2=$?
if [[ $rc1 == 0 && $rc2 == 0 ]]; then note "ok   immutable surfaces (vs $base_ref)"; else
  note "FAIL immutable surfaces (git diff $base_ref -- web/ docs/RULES_VERSION.md crates/ti4-model/src/state.rs fixtures)"; fail=1; fi

# builds
t0=$SECONDS; check build-selfplay cargo build --profile gate -p ti4-selfplay; note "     build-selfplay $((SECONDS - t0)) s"
cp "$target/gate/selfplay" "$out/selfplay-new"
check build-bridge cargo build --profile gate -p ti4-engine-core-bridge --bins

# 4 round trip, strict (package D2-1, trap T16: this was never part of the script): store(load(g)) == g and
# load(store(x)) == x over the template openings, the fixtures and every STRIDE-th position of the golden games, with
# no refusal (`unmapped: 0`). The marks census lists the faction_marks key templates no typed family covers.
check roundtrip "$target/gate/core_sync_gate" tools/engine_profile/profile-20.json "$games" 10 --strict
note "     $(grep '^positions ' "$out/roundtrip.log" | tail -1)"
note "     $(grep '^unmapped (census' "$out/roundtrip.log" | tail -1)"
check marks-census python3 -I tools/engine_core/marks_census.py --strict
note "     $(tail -1 "$out/marks-census.log")"
# The alpha logs, on a COPY (the tool refuses the live directory). SKIP_ALPHA=1 or no /var/lib/ti4-public skips it.
if [[ -d /var/lib/ti4-public && "${SKIP_ALPHA:-0}" != 1 ]]; then
  alpha_copy="$(mktemp -d /root/deploytmp/alpha-copy.XXXXXX)"
  rsync -a --exclude feedback /var/lib/ti4-public/ "$alpha_copy/"
  check alpha-roundtrip "$target/gate/alpha_roundtrip" "$alpha_copy" --strict
  note "     $(grep '^alpha_roundtrip:' "$out/alpha-roundtrip.log" | tail -1)"
  rm -rf "$alpha_copy"
fi

# 1 tests
check tests cargo test -p ti4-model -p ti4-content -p ti4-engine -p ti4-host -p ti4-rule-bot \
  -p ti4-selfplay -p ti4-engine-core -p ti4-engine-core-factions -p ti4-engine-core-bridge
note "     $(grep -c '^test result: ok' "$out/tests.log") suites ok, $(grep -c 'FAILED\|test result: FAILED' "$out/tests.log") failed"
check gen-cards-check cargo run -q -p xtask -- gen-cards --check
check tests-xtask cargo test -p xtask
check tests-wasm cargo test -p ti4-wasm --features ts

# 3 dual run
if [[ -n "$dual" ]]; then
  check dual-suite env "$dual" TI4_DUAL_FAIL_FAST=1 cargo test -p ti4-engine
  note "     $(grep -c '^test result: ok' "$out/dual-suite.log") suites ok with $area on Dual (fail fast)"
  # SHARDS (default 4; 1 = one process) parallel shards, merged into the text of the single run.
  check dual-golden env TI4_DUAL_FAIL_FAST=1 tools/engine_core/dual_sharded.sh "$target/gate/dual_run" \
    tools/engine_profile/profile-20.json "$games" "$area=dual${EXTRA_DUAL:+,$EXTRA_DUAL}" "$out/dual-golden-shards"
  sed 's/^/     /' "$out/dual-golden.log" | head -12 | tee -a "$out/summary.txt"
  if [[ "$area" == board ]]; then
    check dual-templates "$target/gate/board_equivalence" templates
    note "     $(grep '^board queries' "$out/dual-templates.log" | tail -1)"
    check dual-positions "$target/gate/board_equivalence" golden tools/engine_profile/profile-20.json 20 10
    note "     $(grep '^board queries' "$out/dual-positions.log" | tail -1)"
  fi
  if [[ "$area" == agenda ]]; then
    # B9: the agenda reads next to the string functions (the same bin prints all summaries).
    check dual-templates "$target/gate/board_equivalence" templates
    note "     $(grep '^agenda reads' "$out/dual-templates.log" | tail -1)"
    check dual-positions "$target/gate/board_equivalence" golden tools/engine_profile/profile-20.json 20 10
    note "     $(grep '^agenda reads' "$out/dual-positions.log" | tail -1)"
  fi
  if [[ "$area" == strategy ]]; then
    # B8: the strategy reads next to the string functions (the same bin prints both summaries).
    check dual-templates "$target/gate/board_equivalence" templates
    note "     $(grep '^strategy reads' "$out/dual-templates.log" | tail -1)"
    check dual-positions "$target/gate/board_equivalence" golden tools/engine_profile/profile-20.json 20 10
    note "     $(grep '^strategy reads' "$out/dual-positions.log" | tail -1)"
  fi
  if [[ "$area" == exploration ]]; then
    check dual-templates "$target/gate/board_equivalence" cards templates
    note "     $(grep '^exploration queries' "$out/dual-templates.log" | tail -1)"
    check dual-positions "$target/gate/board_equivalence" cards golden tools/engine_profile/profile-20.json 20 10 6
    note "     $(grep '^exploration queries' "$out/dual-positions.log" | tail -1)"
  fi
  if [[ "$area" == action_cards ]]; then
    # B6: the action-card reads next to the string functions (the same bin prints all summaries).
    check dual-templates "$target/gate/board_equivalence" templates
    note "     $(grep '^action card reads' "$out/dual-templates.log" | tail -1)"
    check dual-positions "$target/gate/board_equivalence" golden tools/engine_profile/profile-20.json 20 10
    note "     $(grep '^action card reads' "$out/dual-positions.log" | tail -1)"
  fi
  if [[ "$area" == movement ]]; then
    check dual-templates "$target/gate/core_equivalence" templates --routes
    note "     $(tail -1 "$out/dual-templates.log")"
    check dual-positions tools/engine_core/positions_sharded.sh "$target/gate/core_equivalence" tools/engine_profile/profile-20.json 20 "${POSITIONS_STRIDE:-40}" "$out/dual-positions-shards"
    note "     $(grep '^shard' "$out/dual-positions.log" | tr '\n' ' ' | cut -c1-400)"
  fi
fi

# 5 benches
bench() { # NAME ENVASSIGN
  local name="$1" envv="$2"
  env ${envv:+"$envv"} VERIFY_OUT="$out/$name" tools/engine_profile/verify_benches.sh "$out/selfplay-new" "$workers" > "$out/$name.txt" 2>&1
  sed "s/^/     $name /" "$out/$name.txt" | tee -a "$out/summary.txt"
  [[ $(grep -c '200 identical, 0 different' "$out/$name.txt") == 6 ]] || { note "FAIL $name"; fail=1; }
}
bench benches-core "$on"
export SELFPLAY_RESULTS=off
if [[ -n "$off" ]]; then
  env "$off" "$out/selfplay-new" run --config tools/rulebot/golden.json --verify tools/rulebot/baseline.json \
    --no-cache --workers "$workers" --out "$out/bench-string.json" --quiet --no-table > "$out/bench-string.log" 2>&1
  note "     bench with $area on String: $(grep -o '[0-9]* identical, [0-9]* different' "$out/bench-string.log")"
  grep -q '200 identical, 0 different' "$out/bench-string.log" || { note "FAIL bench-string"; fail=1; }
fi
if [[ -n "$dual" && "${DUAL_BENCH:-0}" == 1 ]]; then
  env "$dual" "$out/selfplay-new" run --config tools/rulebot/golden.json --verify tools/rulebot/baseline.json \
    --no-cache --workers "$workers" --out "$out/bench-dual.json" --quiet --no-table > "$out/bench-dual.log" 2>&1
  note "     bench with $area on Dual: $(grep -o '[0-9]* identical, [0-9]* different' "$out/bench-dual.log")"
  grep -q '200 identical, 0 different' "$out/bench-dual.log" || { note "FAIL bench-dual"; fail=1; }
fi

# 6/7 hashes + CPU A/B
if [[ "${RUN_AB:-0}" == 1 && "${SKIP_AB:-0}" != 1 ]]; then
  env ${on:+"$on"} AB_OUT="$out/ab" tools/engine_profile/ab_time.sh "$base" "$out/selfplay-new" tools/engine_profile/profile-20.json 3 4 > "$out/ab.txt" 2>&1
  tail -2 "$out/ab.txt" | sed 's/^/     ab  /' | tee -a "$out/summary.txt"
  grep -q 'same decision hash: 20/20' "$out/ab.txt" || { note "FAIL ab hashes (profile-20 vs base)"; fail=1; }
  ratio="$(grep -o 'B/A = [0-9.]*' "$out/ab.txt" | grep -o '[0-9.]*$')"
  [[ -n "$ratio" ]] && python3 -I -c "import sys; sys.exit(0 if float('$ratio') <= 1.015 else 1)" || { note "FAIL cpu A/B ratio ${ratio:-?} > 1.015"; fail=1; }
fi

# 8 wasm size
if [[ "${RUN_WASM:-0}" == 1 ]]; then
check wasm cargo build -p ti4-wasm --target wasm32-unknown-unknown --profile wasm-release
wasm="$target/wasm32-unknown-unknown/wasm-release/ti4_wasm.wasm"
note "     wasm raw (cargo) $(stat -c %s "$wasm" 2>/dev/null) B"
if [[ -n "${WASM_OPT:-}" && -f "$wasm" ]]; then
  "$WASM_OPT" --enable-bulk-memory --enable-bulk-memory-opt --enable-call-indirect-overlong \
    --enable-multivalue --enable-mutable-globals --enable-nontrapping-float-to-int \
    --enable-reference-types --enable-sign-ext -Oz --always-inline-max-function-size=0 \
    --flexible-inline-max-function-size=0 --one-caller-inline-max-function-size=0 \
    --partial-inlining-ifs=0 "$wasm" -o "$out/ti4_wasm.opt.wasm"
  node scripts/wasm-size-gate.mjs "$out/ti4_wasm.opt.wasm" > "$out/wasm-gate.log" 2>&1
  node -e 'const fs=require("fs"),z=require("zlib");const b=fs.readFileSync(process.argv[1]);console.log("     wasm-opt raw "+b.length+" gzip "+z.gzipSync(b,{level:9}).length+" brotli "+z.brotliCompressSync(b,{params:{[z.constants.BROTLI_PARAM_QUALITY]:11}}).length)' "$out/ti4_wasm.opt.wasm" | tee -a "$out/summary.txt"
  sed 's/^/     gate /' "$out/wasm-gate.log" | tee -a "$out/summary.txt"
fi
fi

# 10 state size
if [[ -x "$target/release/state_size" ]]; then
  check state-size tools/engine_core/state_size.sh
  sed 's/^/     /' "$out/state-size.log" | head -8 | tee -a "$out/summary.txt"
else
  note "skip state-size (no release state_size binary in $target; build it with cargo build --release -p ti4-engine-core-bridge --bins)"
fi

[[ $fail == 0 ]] && note "PACKAGE OK ($area)" || note "PACKAGE FAILED ($area)"
exit $fail
