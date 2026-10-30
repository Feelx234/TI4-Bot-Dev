#!/usr/bin/env bash
# Mutation testing with ONE build (docs/ENGINE_COMPUTE.md 1).
#
#   tools/engine_core/mutants.sh TARGET [TEST_FILTER...]
#
# TARGET   a source file or directory (path relative to the repo, e.g. crates/ti4-engine-core/src/query/tech.rs)
#          or a module path of the core (query::tech, rules::combat; CRATE picks another crate).
# TEST_FILTER  words passed to the libtest binary (substring match on the test names, like `cargo test FILTER`);
#          none = every test of the crate target. Name the tests of the module you changed: faster, and a
#          survivor then means "this module's own tests do not see it".
# Env: CRATE (default: from the path, else ti4-engine-core), TEST_CRATE (run the tests of ANOTHER crate that
#      depends on the mutated one, e.g. ti4-engine-core-bridge with TEST_ARGS="--test tech"), JOBS (mutants run in parallel, default 4),
#      CARGO_BUILD_JOBS (default 2), KINDS (default body,arith,cmp,logic,cond,lit,assign), FN (only functions
#      whose name contains one of a,b,c), LINES (S-E), TEST_ARGS (default --lib; e.g. "--lib --test adapter"),
#      CHECK_ONLY=1 (only check that the rewrite compiles, with `cargo check --tests`; no test run),
#      EXCLUDE (regex; repo-relative files it matches are left out of a directory TARGET),
#      TIMEOUT (seconds per mutant), LIST=1 (only list the mutation points), QUIET=1 (survivors only),
#      MUT_DIR (scratch, default /root/deploytmp/mutants-<checkout>), STRICT=1 (exit 1 when mutants survive).
#
# Nothing in the checkout is changed: the source is rewritten in a COPY (MUT_DIR/tree), where tools/mutants
# wraps operators, comparisons, conditions, integer/bool literals and function bodies in a runtime switch
# (env TI4_MUTANT=<id>); the test binary is built once and run once per mutant. A line containing
# `// mutants:skip` (or the line above a fn) is not mutated.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
target="${1:?TARGET (file, directory or module path)}"; shift
filter=("$@")
name="$(basename "$repo")"
mut="${MUT_DIR:-/root/deploytmp/mutants-$name}"; tree="$mut/tree"; tgt="$mut/target"
mkdir -p "$mut"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"

# 1. files
crate="${CRATE:-}"
mapfile -t files < <(python3 -I - "$repo" "$target" "$crate" <<'PY'
import os, sys
repo, t, crate = sys.argv[1:4]
EXCLUDE = os.environ.get("EXCLUDE")  # regex over repo-relative paths: files it matches are not mutated (D4: baseline of a directory minus B12-owned files)
def rs(d):
    out = []
    for r, ds, fs in os.walk(os.path.join(repo, d)):
        ds[:] = sorted(x for x in ds if x not in ("generated", "tests", "benches", "target"))
        out += [os.path.relpath(os.path.join(r, f), repo) for f in sorted(fs) if f.endswith(".rs") and not f.startswith("__")]
    if EXCLUDE:
        import re
        out = [f for f in out if not re.search(EXCLUDE, f)]
    return out
if "/" in t or t.endswith(".rs"):
    p = t if not os.path.isabs(t) else os.path.relpath(t, repo)
    print("\n".join(rs(p) if os.path.isdir(os.path.join(repo, p)) else [p]))
else:
    c = crate or "ti4-engine-core"
    base = os.path.join("crates", c, "src", *t.split("::"))
    for cand in (base + ".rs", os.path.join(base, "mod.rs")):
        if os.path.exists(os.path.join(repo, cand)):
            print(cand); break
    else:
        sys.exit("no module " + t + " in crate " + c)
PY
)
[[ ${#files[@]} -gt 0 && -n "${files[0]}" ]] || { echo "no files for $target"; exit 2; }
for f in "${files[@]}"; do [[ -f "$repo/$f" ]] || { echo "not a file: $f"; exit 2; }; done
if [[ -z "$crate" ]]; then
  d="$repo/${files[0]}"; while [[ "$d" != "$repo" && ! -f "$d/Cargo.toml" ]]; do d="$(dirname "$d")"; done
  crate="$(sed -n 's/^name *= *"\(.*\)"/\1/p' "$d/Cargo.toml" | head -1)"; cratedir="${d#$repo/}"
else cratedir="crates/$crate"; fi
tcrate="${TEST_CRATE:-$crate}"; tcratedir="$cratedir"; [[ -n "${TEST_CRATE:-}" ]] && tcratedir="crates/$TEST_CRATE"
echo "crate $crate (tests of $tcrate), ${#files[@]} file(s): ${files[*]}" | cut -c1-200

# 2. the tool (cheap; its own target dir)
t0=$SECONDS
CARGO_TARGET_DIR="${MUT_TOOL_TARGET:-/root/deploytmp/mutants-tool-target}" cargo build -q --offline --manifest-path "$repo/tools/mutants/Cargo.toml" || exit 2
tool="${MUT_TOOL_TARGET:-/root/deploytmp/mutants-tool-target}/debug/ti4-mutants"

# 3. the scratch copy: content-compared, so only files that really differ get a new mtime (cargo then
#    rebuilds only those crates); mtimes of unchanged files are NOT preserved on purpose.
mkdir -p "$tree"
rsync -rlpgo --checksum --delete \
  --exclude=/.git --exclude=target/ --exclude=node_modules/ --exclude=/web/ --exclude=/web2/ --exclude=/web-shared/ \
  --exclude=/.claude/ --exclude=/tools/mutants/ "$repo/" "$tree/"
disable="$mut/disabled.txt"; : > "$disable"
gen_args=(gen --orig "$repo" --dest "$tree" --points "$mut/points.json" --disable "$disable")
[[ -n "${KINDS:-}" ]] && gen_args+=(--kinds "$KINDS")
[[ -n "${FN:-}" ]] && gen_args+=(--fn "$FN")
[[ -n "${LINES:-}" ]] && gen_args+=(--lines "$LINES")
"$tool" "${gen_args[@]}" "${files[@]}" || exit 2
if [[ "${LIST:-0}" == 1 ]]; then
  python3 -I - "$mut/points.json" <<'PY'
import json, sys
for p in json.load(open(sys.argv[1]))["points"]:
    print(f'#{p["id"]:<4} {p["file"]}:{p["line"]}:{p["col"]}  [{p["fn"]}] {p["desc"]}:  {p["orig"]}  =>  {p["mutated"]}')
PY
  exit 0
fi

# 4. build once; points whose rewrite does not compile (type inference, moves, const contexts) are disabled
#    from the compile errors and the tree is regenerated, a few rounds at most.
read -ra targs <<< "${TEST_ARGS:---lib}"
build_cmd=(test --no-run); [[ "${CHECK_ONLY:-0}" == 1 ]] && build_cmd=(check --tests)
# No debug info in the scratch build (the dev profile has line tables + packed dwp: about 3.5 GB for the bridge tests).
build() { ( cd "$tree" && CARGO_TARGET_DIR="$tgt" CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_SPLIT_DEBUGINFO=off cargo "${build_cmd[@]}" --offline --locked "${targs[@]}" -p "$tcrate" --message-format=json 2> "$mut/build.err" > "$mut/build.json" ); }
round=0
until build; do
  round=$((round + 1))
  if [[ $round -gt 8 ]]; then echo "build still fails after 8 prune rounds; see $mut/build.err and $mut/build.json"; tail -20 "$mut/build.err"; exit 2; fi
  extra=(); [[ $round -gt 2 ]] && extra=(--all)
  "$tool" prune --points "$mut/points.json" --dest "$tree" --disable "$disable" "${extra[@]}" < "$mut/build.json"
  rc=$?
  if [[ $rc -ne 0 ]]; then echo "build fails and no mutation point is to blame (see $mut/build.err):"; grep -E '^error' -A4 "$mut/build.err" | head -30; exit 2; fi
  "$tool" "${gen_args[@]}" "${files[@]}" > /dev/null || exit 2
done
if [[ "${CHECK_ONLY:-0}" == 1 ]]; then echo "CHECK_ONLY: the rewrite compiles ($round prune round(s), $(python3 -I -c 'import json,sys;print(sum(1 for p in json.load(open(sys.argv[1]))["points"] if p["disabled"] or p["id"] in {int(x) for x in open(sys.argv[2]).read().split()}))' "$mut/points.json" "$disable") of $(python3 -I -c 'import json,sys;print(len(json.load(open(sys.argv[1]))["points"]))' "$mut/points.json") points dropped)"; exit 0; fi
mapfile -t exes < <(python3 -I -c '
import json,sys
for l in open(sys.argv[1]):
    try: m=json.loads(l)
    except Exception: continue
    if m.get("reason")=="compiler-artifact" and m.get("executable") and m["profile"]["test"]:
        print(m["executable"])' "$mut/build.json")
[[ ${#exes[@]} -gt 0 ]] || { echo "no test executable produced"; exit 2; }
echo "built once in $((SECONDS - t0)) s ($round prune round(s)); test executables: ${#exes[@]}"

# 5. run
exe_args=(); for e in "${exes[@]}"; do exe_args+=(--exe "$e"); done
q=(); [[ "${QUIET:-0}" == 1 ]] && q=(--quiet)
python3 -I "$repo/tools/mutants/run_mutants.py" --points "$mut/points.json" --disable "$disable" --cwd "$tree/$tcratedir" \
  "${exe_args[@]}" --jobs "${JOBS:-4}" --timeout "${TIMEOUT:-0}" --out "$mut/report.json" "${q[@]}" -- "${filter[@]}"
rc=$?
[[ $rc -ne 0 ]] && exit $rc
if [[ "${STRICT:-0}" == 1 ]]; then
  python3 -I -c 'import json,sys; sys.exit(1 if any(m["status"] == "ok" for m in json.load(open(sys.argv[1]))["mutants"]) else 0)' "$mut/report.json" || exit 1
fi
exit 0
