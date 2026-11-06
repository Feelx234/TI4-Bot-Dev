#!/usr/bin/env bash
# D2-9 lane 9a: no production crate may reach `ti4-engine-legacy` or the `lend-string` feature
# (docs/todo/D2-9-strip.md 2.1/2.2, docs/D29A_RESUME.md). Uses `cargo tree` only: no build, seconds.
#
#   tools/engine_core/check_production_graph.sh              strict: exit 1 on any violation (the end state)
#   tools/engine_core/check_production_graph.sh --report     print the current violations, exit 0 (until lane 9b lands)
#   tools/engine_core/check_production_graph.sh --self-test   prove on a synthetic workspace that the checker fails when it must
#   tools/engine_core/check_production_graph.sh --root DIR   check another checkout (default: this repo)
#
# Checks, per production crate P (PROD_CRATES, default the list below):
#   1 `cargo tree -p P -e normal,build,features` and the same with `dev` added (a dev-dependency must not smuggle
#     the feature or the crate in): no `ti4-engine-legacy`, no `lend-string` anywhere in the tree;
#   2 the thin crate `ti4-engine` as resolved for P carries none of the dev features (typed-core, native-subject,
#     observed-count, string-*, lend-string);
#   3 `ti4-wasm` again for --target wasm32-unknown-unknown;
#   4 `ti4-engine-core-bridge` / `ti4-engine-core-tests` / `ti4-perf-alloc` (counting allocator, unsafe, perf tools only) and the MCTS research
#     crates `ti4-mcts-features` / `ti4-mcts-lab` are not in the tree of P (normal, build, dev);
# and statically: 5 the production alias sets of .cargo/config.toml list no dev-tool crate and no legacy;
# 6 `lend-string` appears in Cargo.toml files only of the allow-list; 7 the thin crate has no default feature that
# reaches lend-string/legacy; 8 deploy/, web-shared/scripts, scripts/ never use `--workspace` or `lend-string`.
# Negative controls (always run, also in --report): the bridge MUST reach legacy, and `ti4-engine --features lend-string`
# MUST (the second only once that feature exists); otherwise the checker has rotted to always-green and the run fails.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
mode=strict
while [[ $# -gt 0 ]]; do
  case "$1" in
    --report) mode=report ;;
    --self-test) mode=selftest ;;
    --root) root="$(cd "${2:?--root DIR}" && pwd)"; shift ;;
    -h|--help) sed -n '2,23p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done

PROD_CRATES="${PROD_CRATES:-ti4-host ti4-server ti4-wasm ti4-rule-bot ti4-model ti4-content ti4-calc ti4-engine ti4-chance ti4-engine-chance}"
WASM_CRATE="${WASM_CRATE:-ti4-wasm}"
DEV_TOOLS="${DEV_TOOLS:-ti4-engine-core-bridge ti4-engine-core-tests ti4-perf-alloc ti4-training ti4-sim ti4-policy ti4-mlp ti4-review ti4-replayer ti4-cli ti4-bot-agent ti4-selfplay ti4-bridge ti4-legacy ti4-advisor ti4-rule-bot-typed ti4-tensor ti4-mcts-features ti4-mcts-lab xtask}"
ALLOW_LEND="${ALLOW_LEND:-ti4-engine ti4-engine-core-bridge ti4-engine-core-tests ti4-training ti4-review ti4-replayer ti4-sim ti4-selfplay ti4-cli ti4-mlp ti4-bridge ti4-legacy ti4-advisor}"
DEV_FEATURES='typed-core|native-subject|observed-count|string-[a-z]+|lend-string'

# ---- tree helpers (all take the workspace dir first) ----
tree() { local dir="$1"; shift; (cd "$dir" && cargo tree --offline "$@" 2>&1); }

# violations of one crate; prints one line per violation (and the first inverted path for a reach).
crate_violations() {
  local dir="$1" p="$2" extra="${3:-}" k out f t label
  label="$p${3:+ ($3)}"
  for k in "normal,build" "normal,build,dev"; do
    out="$(tree "$dir" -p "$p" $extra -e "$k,features" --prefix none)"
    if [[ "$out" == error:* || "$out" == *"error: "* && "$out" != *"$p"* ]]; then echo "ERROR $label [$k]: cargo tree failed: $(head -1 <<<"$out")"; continue; fi
    local reach=0 who
    if [[ "$k" == "normal,build" ]]; then
      grep -qE '^ti4-engine-legacy |lend-string|lend_string' <<<"$out" && reach=1
    else
      # dev tree: the feature never; the legacy crate only as a DIRECT dev-dependency of P (tests and examples that drive a string window;
      # it is not linked into any product binary). Any other package that depends on legacy in the tree of P (a depth-1 dependent of legacy other than P) is a violation.
      grep -qE 'lend-string|lend_string' <<<"$out" && reach=1
      who="$(tree "$dir" -p "$p" $extra -e "$k" -i ti4-engine-legacy --prefix depth | grep -E '^1[^0-9]' | sed -E 's/^1//; s/ v[0-9].*//' | sort -u | grep -v -x -e "$p" | head -1)"
      [[ -n "$who" && "$who" != error* && "$who" != *"did not match"* && "$who" != *"nothing to print"* ]] && reach=1
    fi
    if (( reach )); then
      echo "VIOLATION $label [$k]: reaches legacy / lend-string"
      tree "$dir" -p "$p" $extra -e "$k" -i ti4-engine-legacy --prefix depth | head -6 | sed 's/^/    via /'
      grep -E 'lend-string' <<<"$out" | sort -u | head -3 | sed 's/^/    feature /'
    fi
  done
  # the thin crate's dev features as resolved for P
  out="$(tree "$dir" -p "$p" $extra -e normal,build,features -i ti4-engine --prefix none 2>&1)"
  f="$(grep -oE "feature \"($DEV_FEATURES)\"" <<<"$out" | sort -u | tr '\n' ' ')"
  [[ -n "$f" ]] && echo "VIOLATION $label [normal,build]: thin crate ti4-engine carries $f"
  for t in ti4-engine-core-bridge ti4-engine-core-tests ti4-perf-alloc ti4-mcts-features ti4-mcts-lab; do
    out="$(tree "$dir" -p "$p" $extra -e normal,build,dev --prefix none)"
    grep -q "^$t " <<<"$out" && echo "VIOLATION $label [normal,build,dev]: tree contains $t"
  done
  return 0
}

all_graph_violations() {
  local dir="$1" p
  for p in $PROD_CRATES; do crate_violations "$dir" "$p"; done
  if grep -q "^$WASM_CRATE\$" <<<"$(cd "$dir" && cargo metadata --offline --no-deps --format-version 1 2>/dev/null | python3 -I -c 'import json,sys;print("\n".join(p["name"] for p in json.load(sys.stdin)["packages"]))')"; then
    crate_violations "$dir" "$WASM_CRATE" "--target wasm32-unknown-unknown"
  fi
}

static_violations() {
  local dir="$1"
  python3 -I - "$dir" "$DEV_TOOLS" "$ALLOW_LEND" <<'PY'
import sys, os, re, glob, tomllib
root, dev, allow = sys.argv[1], sys.argv[2].split(), set(sys.argv[3].split())
cfg = os.path.join(root, '.cargo', 'config.toml')
if os.path.exists(cfg):
    aliases = tomllib.load(open(cfg, 'rb')).get('alias', {})
    for name, val in aliases.items():
        if not name.endswith('-web') and not name.endswith('-web-slow'):
            continue
        toks = val.split() if isinstance(val, str) else [t for s in val for t in s.split()]
        pk = {toks[i + 1] for i, t in enumerate(toks[:-1]) if t == '-p'}
        bad = sorted(pk & set(dev))
        if bad: print(f'VIOLATION alias {name}: lists dev-tool crate(s) {" ".join(bad)}')
        if 'ti4-engine-legacy' in pk: print(f'VIOLATION alias {name}: lists ti4-engine-legacy (lane 9d drops it)')
        for i, t in enumerate(toks):
            if re.search(r'ti4-engine-legacy\.opt-level', t): print(f'VIOLATION alias {name}: profile key ti4-engine-legacy (lane 9d renames it)'); break
        if '--workspace' in toks or '--all' in toks: print(f'VIOLATION alias {name}: --workspace')
for f in sorted(glob.glob(os.path.join(root, 'crates', '*', 'Cargo.toml'))):
    d = os.path.basename(os.path.dirname(f))
    try: t = tomllib.load(open(f, 'rb'))
    except Exception as e: print(f'ERROR {f}: {e}'); continue
    name = t.get('package', {}).get('name', d)
    txt = open(f).read()
    code = '\n'.join(l.split('#', 1)[0] for l in txt.splitlines())
    if 'lend-string' in code and name not in allow:
        print(f'VIOLATION crates/{d}/Cargo.toml ({name}): mentions lend-string but is not on the allow-list')
    if name == 'ti4-engine':
        dflt = t.get('features', {}).get('default', [])
        feats = t.get('features', {})
        def reach(x, seen=()):
            if x in seen: return False
            if 'lend-string' in x or x == 'dep:ti4-engine-legacy' or x.startswith('ti4-engine-legacy'): return True
            return any(reach(y, seen + (x,)) for y in feats.get(x, []))
        if any(reach(x) for x in dflt):
            print(f'VIOLATION crates/{d}/Cargo.toml: default features {dflt} reach legacy / lend-string')
PY
  local h
  h="$(cd "$dir" && grep -rInE -e '--workspace|lend-string' deploy web-shared/scripts scripts --include='*.sh' --include='*.mjs' --include='*.js' --include='*.ts' --exclude-dir=node_modules 2>/dev/null | grep -v 'check_production_graph' | head -10)"
  [[ -n "$h" ]] && echo "VIOLATION deploy/build scripts use --workspace or lend-string:" && sed 's/^/    /' <<<"$h"
  return 0
}

# ---- negative controls on the real workspace ----
negative_controls() {
  local dir="$1" bad=0 out
  if (cd "$dir" && cargo metadata --offline --no-deps --format-version 1 2>/dev/null | grep -q '"name":"ti4-engine-core-bridge"'); then
    out="$(tree "$dir" -p ti4-engine-core-bridge -e normal -i ti4-engine-legacy --prefix none)"
    grep -q '^ti4-engine-legacy ' <<<"$out" || { echo "CONTROL FAILED: the bridge does not reach ti4-engine-legacy (checker or graph rotted)"; bad=1; }
  fi
  if grep -qE '^lend-string *=' "$dir/crates/ti4-engine-next/Cargo.toml" 2>/dev/null; then
    out="$(tree "$dir" -p ti4-engine --features lend-string -e normal -i ti4-engine-legacy --prefix none)"
    grep -q '^ti4-engine-legacy ' <<<"$out" || { echo "CONTROL FAILED: ti4-engine --features lend-string does not reach legacy"; bad=1; }
  fi
  # a failed -i must be an error text, never an empty success we could mistake for "clean"
  out="$(tree "$dir" -p ti4-model -e normal -i this-package-does-not-exist-xyz --prefix none)"
  grep -q 'did not match any packages' <<<"$out" || { echo "CONTROL FAILED: cargo tree -i on a missing package no longer says 'did not match any packages': $(head -1 <<<"$out")"; bad=1; }
  return $bad
}

# ---- self-test: a synthetic workspace ----
self_test() {
  local tmp rc=0 v
  export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-$(cd "$root" && rustup show active-toolchain 2>/dev/null | awk '{print $1; exit}')}"
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/d29a-graph.XXXXXX")" || return 1
  trap 'rm -rf "$tmp"' RETURN
  mkdir -p "$tmp/legacy/src" "$tmp/thin/src" "$tmp/good/src" "$tmp/bad/src" "$tmp/devbad/src" "$tmp/direct/src" "$tmp/tool/src" "$tmp/devok/src" "$tmp/devtrans/src"
  for d in legacy thin good bad devbad direct tool devok devtrans; do : > "$tmp/$d/src/lib.rs"; done
  printf '[workspace]\nresolver="2"\nmembers=["legacy","thin","good","bad","devbad","direct","tool","devok","devtrans"]\n' > "$tmp/Cargo.toml"
  mk() { printf '[package]\nname="%s"\nversion="0.1.0"\nedition="2021"\n%s\n' "$1" "$2" > "$tmp/$3/Cargo.toml"; }
  mk ti4-engine-legacy '' legacy
  mk ti4-engine '[dependencies]
ti4-engine-legacy={path="../legacy",optional=true}
[features]
lend-string=["dep:ti4-engine-legacy"]' thin
  mk good '[dependencies]
ti4-engine={path="../thin"}' good
  mk bad '[dependencies]
ti4-engine={path="../thin",features=["lend-string"]}' bad
  mk devbad '[dependencies]
ti4-engine={path="../thin"}
[dev-dependencies]
ti4-engine={path="../thin",features=["lend-string"]}' devbad
  mk direct '[dependencies]
ti4-engine-legacy={path="../legacy"}' direct
  mk tool '[dependencies]
ti4-engine={path="../thin",features=["lend-string"]}' tool
  mk devok '[dependencies]
ti4-engine={path="../thin"}
[dev-dependencies]
ti4-engine-legacy={path="../legacy"}' devok
  mk devtrans '[dependencies]
ti4-engine={path="../thin"}
[dev-dependencies]
direct={path="../direct"}' devtrans
  fail() { echo "SELF-TEST FAILED: $*"; rc=1; }
  v="$(crate_violations "$tmp" good)";   [[ -z "$v" ]] || fail "clean crate flagged: $v"
  v="$(crate_violations "$tmp" bad)";    grep -q VIOLATION <<<"$v" || fail "normal feature smuggle not caught"
  v="$(crate_violations "$tmp" devbad)"; grep -q 'normal,build,dev' <<<"$v" || fail "dev-dependency smuggle not caught"
  grep -q 'VIOLATION devbad \[normal,build\]' <<<"$v" && fail "dev smuggle wrongly flagged in the normal tree"
  v="$(crate_violations "$tmp" direct)"; grep -q VIOLATION <<<"$v" || fail "direct legacy dependency not caught"
  v="$(crate_violations "$tmp" devok)";  [[ -z "$v" ]] || fail "a direct legacy dev-dependency (tests only) wrongly flagged: $v"
  v="$(crate_violations "$tmp" devtrans)"; grep -q 'normal,build,dev' <<<"$v" || fail "legacy reached through a dev-dependency's own dependency not caught"
  # a dev tool may use the feature: it is a violation only for a production crate (the list decides, not the tool)
  PROD_CRATES="good tool" v="$(all_graph_violations "$tmp")"
  grep -q 'VIOLATION tool' <<<"$v" || fail "tool listed as production not caught"
  mkdir -p "$tmp/.cargo"; printf '[alias]\nb-web = "build -p good -p tool"\n' > "$tmp/.cargo/config.toml"
  v="$(DEV_TOOLS=tool static_violations "$tmp")"; grep -q 'alias b-web: lists dev-tool' <<<"$v" || fail "dev-tool crate in an alias set not caught"
  mkdir -p "$tmp/deploy"; echo 'cargo build --workspace' > "$tmp/deploy/x.sh"
  v="$(static_violations "$tmp")"; grep -q -- '--workspace' <<<"$v" || fail "--workspace in deploy not caught"
  v="$(negative_controls "$tmp")" || true  # no bridge in the synthetic workspace: only the missing-package control runs
  [[ -z "$v" ]] || fail "controls on synthetic workspace: $v"
  (( rc == 0 )) && echo "self-test ok: 7 violating shapes caught, the clean crate and the direct legacy dev-dependency pass, controls pass"
  return $rc
}

if [[ "$mode" == selftest ]]; then self_test; exit $?; fi

cd "$root" || exit 2
out="$( { negative_controls "$root"; all_graph_violations "$root"; static_violations "$root"; } 2>&1)"
n_ctl=$(grep -c '^CONTROL FAILED' <<<"$out")
n=$(grep -cE '^(VIOLATION|ERROR)' <<<"$out")
[[ -n "$out" ]] && echo "$out"
echo "production graph: $n violation(s), $n_ctl failed control(s) [mode $mode]"
if (( n_ctl > 0 )); then exit 1; fi        # a rotted checker fails in every mode
if [[ "$mode" == report ]]; then exit 0; fi
(( n == 0 )) || exit 1
