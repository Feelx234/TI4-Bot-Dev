#!/usr/bin/env bash
# `cargo test` with the test executables run CONCURRENTLY (docs/ENGINE_COMPUTE.md 4; cargo-nextest is not installed).
#
#   tools/engine_core/test_parallel.sh [CARGO_TEST_ARGS...]     e.g. -p ti4-model -p ti4-engine-legacy -p ti4-host --profile gate
#
# Env: JOBS (executables running at once, default 3), THREADS (--test-threads of each, default 4),
#      FILTER (words after `--`, e.g. a test name filter), OUT (log directory, default a temp dir).
#
# Builds with `cargo test --no-run` (the usual one build), then runs every test executable (lib, integration
# tests; doc tests are NOT run, run `cargo test --doc` for them) from its package directory with
# CARGO_MANIFEST_DIR set, JOBS at a time. Prints one `ok/FAILED  package::target  N passed ... in S s` line per
# executable in the build order, then the names of the failed tests with the tail of their output.
# Exit 1 when any executable failed. Use it where `cargo test -p a -p b ...` runs the executables one after
# the other (the many small suites of verify_package.sh check 1 leave the cores idle while one runs).
set -uo pipefail
out="${OUT:-$(mktemp -d)}"; mkdir -p "$out"
jobs="${JOBS:-3}"; threads="${THREADS:-4}"
cargo test "$@" --no-run --message-format=json > "$out/build.json" 2> "$out/build.err" || { cat "$out/build.err" | tail -30; exit 2; }
python3 -I - "$out" "$jobs" "$threads" "${FILTER:-}" <<'PY'
import json, os, re, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor
out, jobs, threads, flt = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4].split()
exes = []
for line in open(os.path.join(out, "build.json")):
    try: m = json.loads(line)
    except ValueError: continue
    if m.get("reason") == "compiler-artifact" and m.get("executable") and m["profile"]["test"]:
        mf = os.path.dirname(m["manifest_path"]) if "manifest_path" in m else os.getcwd()
        exes.append((f'{m["target"]["name"]}', m["executable"], mf, m["target"]["kind"]))
def run(e):
    name, exe, mf, kind = e
    t0 = time.time()
    r = subprocess.run([exe, *flt, f"--test-threads={threads}"], cwd=mf, env=dict(os.environ, CARGO_MANIFEST_DIR=mf),
                       capture_output=True, text=True)
    txt = r.stdout + r.stderr
    open(os.path.join(out, f"{name}.{os.path.basename(exe)}.log"), "w").write(txt)
    res = re.findall(r"^test result: .*$", txt, re.M)
    return e, r.returncode, (res[-1] if res else "(no result line)"), time.time() - t0, txt
bad = 0
t0 = time.time()
with ThreadPoolExecutor(jobs) as ex:
    for e, rc, res, s, txt in ex.map(run, exes):
        print(f'{"ok    " if rc == 0 else "FAILED"} {e[0]:<28} {res.replace("test result: ", "")[:90]} [{s:.0f} s]', flush=True)
        if rc != 0:
            bad += 1
            fails = re.findall(r"^---- (\S+) stdout ----", txt, re.M)
            print("       failed tests:", ", ".join(fails[:10]) or "(none named; see log)")
            print("       " + "\n       ".join(txt.strip().splitlines()[-8:]))
print(f"{len(exes)} test executables, {bad} failed, {time.time() - t0:.0f} s (logs in {out})")
sys.exit(1 if bad else 0)
PY
