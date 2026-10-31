#!/usr/bin/env bash
# Self-test of the two guards that keep the D4 covers registry usable for the d4gate coverage check (no cargo, no engine, seconds).
#   tools/engine_core/test_covers_hygiene.sh
# End-of-D1 run: the mutation ratchet (D4 tests once per mutant, EXTRA_TEST_CRATES) appended ~100 MB a minute to
# $CARGO_TARGET_DIR/d4-covers.txt (2 GB in 25 minutes, torn lines from the parallel jobs) -- the file the coverage check reads.
#   1. mutation_ratchet.sh must hand mutants.sh D4_COVERS_FILE=/dev/null (unless the caller names a file);
#   2. the `tests` step of end_of_d1.sh must delete the old registry before it runs the D4 crate (a --from rerun appended to it).
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
fail=0; ok() { echo "ok   $*"; }; bad() { echo "FAIL $*"; fail=1; }

# 1. a copy of the ratchet next to a fake mutants.sh and a one-module table
mkdir -p "$tmp/tools/engine_core/d4"
cp "$here/mutation_ratchet.sh" "$tmp/tools/engine_core/"
printf 'core/fake\tti4-engine-core\tcrates/ti4-engine-core/src/fake.rs\t\tti4-engine-core\t\n' > "$tmp/tools/engine_core/d4/mutation_modules.tsv"
cat > "$tmp/tools/engine_core/mutants.sh" <<'EOF'
#!/usr/bin/env bash
echo "${D4_COVERS_FILE-unset}" > "$SEEN"
exit 2
EOF
chmod +x "$tmp/tools/engine_core/mutants.sh"
run() { SEEN="$tmp/seen" ROOT="$tmp/root" OUT="$tmp/out" "$@" "$tmp/tools/engine_core/mutation_ratchet.sh" core/fake >/dev/null 2>&1; cat "$tmp/seen" 2>/dev/null; }
[[ "$(run env -u D4_COVERS_FILE)" == /dev/null ]] && ok "the ratchet sends the covers registry of a mutation run to /dev/null" || bad "the ratchet sends the covers registry of a mutation run to /dev/null"
[[ "$(run env D4_COVERS_FILE=/x/y.txt)" == /x/y.txt ]] && ok "a file named by the caller is kept" || bad "a file named by the caller is kept"

# 2. the tests step deletes the old registry in front of the D4 crate
if "$here/end_of_d1.sh" --dry-run 2>&1 | grep -E 'rm -f [^ ]*/d4-covers\.txt && cargo test --profile lockstep -p ti4-engine-core-tests' >/dev/null; then
  ok "the tests step deletes the old covers registry before the D4 crate runs"
else bad "the tests step deletes the old covers registry before the D4 crate runs"; fi
exit $fail
