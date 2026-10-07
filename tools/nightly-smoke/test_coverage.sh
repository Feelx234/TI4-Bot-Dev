#!/usr/bin/env bash
# Checks coverage.py on a tiny fake repo and fake reports.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }

mkdir -p "$tmp/repo/crates/demo/src" "$tmp/reports/a" "$tmp/reports/b"
cat > "$tmp/repo/crates/demo/src/lib.rs" <<'EOF'
fn one() { DecisionContext::new(player.clone(), DecisionSource::Content("x".to_owned()), "seen_always", p, r); }
fn two() {
    DecisionContext::new(
        a,
        DecisionSource::Rule(format!("8.{}", 1)),
        "seen_once",
        state.phase, state.round)
}
fn three() { DecisionContext::new(a, b, "never_offered", c, d) }
fn four() { DecisionContext::new(a, b, subtype_var, c, d) }
EOF
echo '{"subtypes": {"seen_always": 3, "seen_once": 1, "not_in_source": 2}}' > "$tmp/reports/a/report.json"
echo '{"subtypes": {"seen_always": 1}}' > "$tmp/reports/b/report.json"
echo 'not json' > "$tmp/reports/b/other.json"

out=$(python3 "$here/coverage.py" --repo "$tmp/repo" "$tmp/reports")
echo "$out" | grep -q "over 2 runs" || fail "should count two reports: $out"
echo "$out" | grep -q "3 literal engine subtypes; 1 never offered, 1 offered in fewer than 2 runs" || fail "counts: $out"
echo "$out" | grep -A2 "## Never offered" | grep -q '`never_offered`' || fail "never_offered should be listed: $out"
echo "$out" | grep -A2 "fewer than 2 runs$" | grep -q '`seen_once` (1)' || fail "seen_once should be rare: $out"
echo "$out" | grep -q 'subtype_var' && fail "a variable subtype is not a literal"
echo "ok: coverage"
