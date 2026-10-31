#!/usr/bin/env bash
# Self-test of tools/engine_profile/verify_benches.sh with a fake `selfplay` (no engine, no lock, a few seconds).
#   tools/engine_core/test_verify_benches.sh
# A bench that finds a difference exits non-zero, and GNU time then writes "Command exited with non-zero status 2" in front of the
# numbers of its -o file. The script must still print its line with real cpu and wall numbers (end-of-D1 run, bench-state: the
# line read "cpu  s wall with non-zero status 2 s" after a `bc` syntax error and hid the figures of the failing bench).
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
fail=0; ok() { echo "ok   $*"; }; bad() { echo "FAIL $*"; fail=1; }
cat > "$tmp/selfplay" <<'EOF'
#!/usr/bin/env bash
# fake: `selfplay run ... --verify BASELINE ...`; the baseline file name tells what to report
for a in "$@"; do case "$a" in *baseline-v5.json) diff=1 ;; esac; done
if [[ -n "${diff:-}" ]]; then
  echo "verify against $a: 199 identical, 1 different, 0 not in the baseline"; echo "  DIFFERENT: selfplay replay --seed 85"; exit 2
fi
echo "verify against $a: 200 identical, 0 different, 0 not in the baseline"; exit 0
EOF
chmod +x "$tmp/selfplay"
out="$tmp/out"
txt="$(VERIFY_OUT="$out" "$here/../engine_profile/verify_benches.sh" "$tmp/selfplay" 3 2>"$tmp/err")"
echo "$txt" | sed 's/^/     /'
[[ "$(echo "$txt" | wc -l)" == 6 ]] && ok "six lines" || bad "six lines"
[[ "$(echo "$txt" | grep -c '200 identical, 0 different')" == 5 ]] && ok "five clean benches" || bad "five clean benches"
echo "$txt" | grep -q '^bench-v5: 199 identical, 1 different, 0 not in the baseline | cpu [0-9.]* s wall [0-9.]* s$' \
  && ok "the failing bench keeps its cpu and wall figures" || bad "the failing bench keeps its cpu and wall figures"
[[ ! -s "$tmp/err" ]] && ok "no error output (bc syntax error)" || { bad "error output"; cat "$tmp/err"; }
exit $fail
