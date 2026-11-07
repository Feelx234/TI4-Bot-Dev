#!/usr/bin/env bash
# Clippy of the core and everything around it, as a gate (end_of_d1.sh step `clippy`).
#
#   tools/engine_core/clippy_core.sh [OUTFILE]      (OUTFILE default: ${CARGO_TARGET_DIR:-target}/clippy-core.txt)
#
# Fails (exit 1) when
#   * cargo itself exits non-zero (an error in ANY of the crates, test targets included), or
#   * a warning is reported in a file of ti4-chance, ti4-engine-chance, ti4-engine-core, -factions, -setup, -bridge, -tests, ti4-rule-bot-typed, ti4-engine-search or ti4-mcts (ANY file), or
#   * ti4-engine-legacy (directory crates/ti4-engine) reports a warning that is not in tools/engine_core/clippy_engine_baseline.txt (lines `file<TAB>message`,
#     the line number left out). The baseline is the engine's old warnings; it may only shrink. `UPDATE_BASELINE=1` rewrites it.
# Engine warnings that are gone from the output are listed (to shrink the baseline) but do not fail.
set -uo pipefail
export LC_ALL=C
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo" || exit 2
out="${1:-${CARGO_TARGET_DIR:-target}/clippy-core.txt}"
base="tools/engine_core/clippy_engine_baseline.txt"
cargo clippy -p ti4-chance -p ti4-engine-chance -p ti4-engine-core -p ti4-engine-core-factions -p ti4-engine-core-setup -p ti4-engine-legacy -p ti4-engine -p ti4-engine-core-bridge -p ti4-engine-core-tests \
  -p ti4-rule-bot-typed -p ti4-engine-search -p ti4-mcts --all-targets --message-format short > "$out" 2>&1
rc=$?
fail=0
if [[ $rc != 0 ]]; then echo "clippy_core: cargo clippy exited $rc"; grep -E '^(error|[^ ]+: error)' "$out" | head -20; fail=1; fi
core_re='^crates/(ti4-chance|ti4-engine-chance|ti4-engine-next|ti4-engine-core|ti4-engine-core-factions|ti4-engine-core-setup|ti4-engine-core-bridge|ti4-engine-core-tests|ti4-rule-bot-typed|ti4-engine-search|ti4-mcts)/[^ ]*: (warning|error)'
if grep -E "$core_re" "$out"; then echo "clippy_core: warnings/errors in the core crates (above)"; fail=1; fi
# engine: "crates/ti4-engine/src/x.rs:12:3: warning: message" -> "crates/ti4-engine/src/x.rs<TAB>message"
now="$(grep -E '^crates/ti4-engine/[^ ]*: warning' "$out" | sed -E 's/^([^:]+):[0-9]+:[0-9]+: warning: /\1\t/' | sort)"
if [[ "${UPDATE_BASELINE:-0}" == 1 ]]; then printf '%s\n' "$now" > "$base"; echo "clippy_core: baseline rewritten ($(wc -l < "$base") lines)"; fi
new="$(comm -13 <(sort "$base" 2>/dev/null) <(printf '%s\n' "$now"))"
gone="$(comm -23 <(sort "$base" 2>/dev/null) <(printf '%s\n' "$now"))"
if [[ -n "$(printf '%s' "$new" | tr -d '[:space:]')" ]]; then echo "clippy_core: NEW warnings in ti4-engine-legacy (not in $base):"; printf '%s\n' "$new"; fail=1; fi
if [[ -n "$(printf '%s' "$gone" | tr -d '[:space:]')" ]]; then echo "clippy_core: baseline entries that are gone (shrink the baseline):"; printf '%s\n' "$gone"; fi
[[ $fail == 0 ]] && echo "clippy_core: ok (cargo rc $rc, engine warnings $(printf '%s\n' "$now" | grep -c .), baseline $(grep -c . "$base" 2>/dev/null))"
exit $fail
