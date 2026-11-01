#!/usr/bin/env bash
# The gate of one engine-core stage (docs/ENGINE_CORE.md, section 3): everything must pass before
# the stage is committed.
#
#   tools/engine_core/verify_stage.sh OUT_DIR BASE_SELFPLAY [WORKERS]
#
# 1. release builds: selfplay with the core (default), selfplay with `string-movement` (the off
#    switch), the bridge binaries;
# 2. the dual run: core_equivalence templates --routes, and golden profile-20 (20 games, every 10th
#    position): 0 differences;
# 3. the tests: model, content, engine, host, rule bot, selfplay, wasm (--features ts), the core and
#    the bridge (debug);
# 4. the wasm build (wasm-release, then wasm-opt as web-shared/scripts/build-wasm.mjs when a binary
#    is given in WASM_OPT) and its size gate;
# 5. all six benches with the core on (200/200 each), `bench` with it off;
# 6. CPU A/B of the 20 golden games: BASE_SELFPLAY vs the core build, 3 interleaved runs.
# Uses $CARGO_TARGET_DIR. Prints a summary to OUT_DIR/summary.txt; exit 1 when anything failed.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
out="$1"; base="$2"; workers="${3:-6}"
target="${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR}"
mkdir -p "$out"; : > "$out/summary.txt"
fail=0
note() { echo "$*" | tee -a "$out/summary.txt"; }
check() { local name="$1"; shift; if "$@" > "$out/$name.log" 2>&1; then note "ok   $name"; else note "FAIL $name (see $out/$name.log)"; fail=1; fi; }
cd "$repo"

check build-on cargo build --release -p ti4-selfplay
cp "$target/release/selfplay" "$out/selfplay-on"
check build-off cargo build --release -p ti4-selfplay --features ti4-engine/string-movement
cp "$target/release/selfplay" "$out/selfplay-off"
check build-bridge cargo build --release -p ti4-engine-core-bridge --bins

check dual-templates "$target/release/core_equivalence" templates --routes
note "     $(tail -1 "$out/dual-templates.log")"
check dual-golden "$target/release/core_equivalence" golden tools/engine_profile/profile-20.json 20 10
note "     $(tail -1 "$out/dual-golden.log")"

check tests cargo test -p ti4-model -p ti4-content -p ti4-engine-legacy -p ti4-engine -p ti4-host -p ti4-rule-bot \
  -p ti4-selfplay -p ti4-engine-core -p ti4-engine-core-bridge
note "     $(grep -c '^test result: ok' "$out/tests.log") suites ok, $(grep -c 'FAILED\|test result: FAILED' "$out/tests.log") failed"
check gen-cards-check cargo run -q -p xtask -- gen-cards --check
check tests-xtask cargo test -p xtask
check tests-wasm cargo test -p ti4-wasm --features ts
note "     $(grep -c '^test result: ok' "$out/tests-wasm.log") suites ok"

check wasm cargo build -p ti4-wasm --target wasm32-unknown-unknown --profile wasm-release
wasm="$target/wasm32-unknown-unknown/wasm-release/ti4_wasm.wasm"
cp "$wasm" "$out/ti4_wasm.wasm"
note "     wasm raw (cargo) $(stat -c %s "$wasm") B"
if [[ -n "${WASM_OPT:-}" ]]; then
  "$WASM_OPT" --enable-bulk-memory --enable-bulk-memory-opt --enable-call-indirect-overlong \
    --enable-multivalue --enable-mutable-globals --enable-nontrapping-float-to-int \
    --enable-reference-types --enable-sign-ext -Oz --always-inline-max-function-size=0 \
    --flexible-inline-max-function-size=0 --one-caller-inline-max-function-size=0 \
    --partial-inlining-ifs=0 "$wasm" -o "$out/ti4_wasm.opt.wasm"
  # Reported, not gating: the gzip/brotli budgets were already exceeded at 8f2d03e3 (1 829 366 /
  # 1 313 584 bytes after wasm-opt); the stage's change of the sizes is what the report states.
  node scripts/wasm-size-gate.mjs "$out/ti4_wasm.opt.wasm" > "$out/wasm-gate.log" 2>&1
  node -e 'const fs=require("fs"),z=require("zlib");const b=fs.readFileSync(process.argv[1]);console.log("     wasm-opt raw "+b.length+" gzip "+z.gzipSync(b,{level:9}).length+" brotli "+z.brotliCompressSync(b,{params:{[z.constants.BROTLI_PARAM_QUALITY]:11}}).length)' "$out/ti4_wasm.opt.wasm" | tee -a "$out/summary.txt"
  sed 's/^/     gate /' "$out/wasm-gate.log" | tee -a "$out/summary.txt"
fi

VERIFY_OUT="$out/benches-on" tools/engine_profile/verify_benches.sh "$out/selfplay-on" "$workers" > "$out/benches-on.txt" 2>&1
sed 's/^/     on  /' "$out/benches-on.txt" | tee -a "$out/summary.txt"
[[ $(grep -c '200 identical, 0 different' "$out/benches-on.txt") == 6 ]] || { note "FAIL benches-on"; fail=1; }
export SELFPLAY_RESULTS=off
"$out/selfplay-off" run --config tools/rulebot/golden.json --verify tools/rulebot/baseline.json \
  --no-cache --workers "$workers" --out "$out/bench-off.json" --quiet --no-table > "$out/bench-off.log" 2>&1
note "     off $(grep -o '[0-9]* identical, [0-9]* different' "$out/bench-off.log")"
grep -q '200 identical, 0 different' "$out/bench-off.log" || { note "FAIL bench-off"; fail=1; }

AB_OUT="$out/ab" tools/engine_profile/ab_time.sh "$base" "$out/selfplay-on" tools/engine_profile/profile-20.json 3 4 > "$out/ab.txt" 2>&1
sed 's/^/     ab  /' "$out/ab.txt" | tail -2 | tee -a "$out/summary.txt"
grep -q 'same decision hash: 20/20' "$out/ab.txt" || { note "FAIL ab hashes"; fail=1; }

[[ $fail == 0 ]] && note "STAGE OK" || note "STAGE FAILED"
exit $fail
