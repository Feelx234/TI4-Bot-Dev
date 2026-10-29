#!/usr/bin/env python3 -I
"""Runs the test executable(s) once per mutant (env TI4_MUTANT=<id>) and prints killed/survived with
file:line and the mutated expression. Used by tools/engine_core/mutants.sh (docs/ENGINE_COMPUTE.md).

  run_mutants.py --points P.json --disable D.txt --cwd DIR --exe EXE... [--jobs N] [--timeout S]
                 [--out report.json] -- FILTER...
"""
import argparse, json, os, re, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor

ap = argparse.ArgumentParser()
ap.add_argument("--points", required=True)
ap.add_argument("--disable", required=True)
ap.add_argument("--cwd", required=True)
ap.add_argument("--exe", action="append", required=True)
ap.add_argument("--jobs", type=int, default=4)
ap.add_argument("--threads", type=int, default=2, help="--test-threads of each run")
ap.add_argument("--timeout", type=float, default=0)
ap.add_argument("--out", default=None)
ap.add_argument("--quiet", action="store_true", help="only survivors and the summary")
ap.add_argument("filter", nargs="*")
a = ap.parse_args()

pts = json.load(open(a.points))["points"]
try:
    dis = {int(x) for x in open(a.disable).read().split()}
except OSError:
    dis = set()
live = [p for p in pts if not p["disabled"] and p["id"] not in dis]
unviable = [p for p in pts if p["id"] in dis or p["disabled"]]

def run(mid, timeout=None):
    """-> (status, detail, seconds) status: ok | fail | timeout"""
    env = dict(os.environ, TI4_MUTANT=str(mid), CARGO_MANIFEST_DIR=a.cwd, RUST_BACKTRACE="0")
    t0 = time.time()
    failed = []
    for exe in a.exe:
        try:
            r = subprocess.run([exe, *a.filter, f"--test-threads={a.threads}", "-q"], cwd=a.cwd, env=env,
                               capture_output=True, text=True, timeout=timeout)
        except subprocess.TimeoutExpired:
            return "timeout", "", time.time() - t0
        out = r.stdout + r.stderr
        if r.returncode != 0:
            failed += re.findall(r"^\s+(\S+)$", out.split("failures:\n")[-1], re.M) if "failures:" in out else []
            if not failed:
                m = re.findall(r"^---- (\S+) stdout ----", out, re.M)
                failed = m or ["(process failed: rc %d)" % r.returncode]
            return "fail", ",".join(failed[:3]), time.time() - t0
    return "ok", "", time.time() - t0

t0 = time.time()
st, detail, base_s = run(0)
if st != "ok":
    print(f"BASELINE FAILS without a mutant ({st}: {detail}); the filter/tests must pass first.", file=sys.stderr)
    sys.exit(2)
ran = sum(1 for _ in [0])
timeout = a.timeout or max(20.0, base_s * 6 + 10)
print(f"baseline (no mutant) passes in {base_s:.1f} s; {len(live)} mutants, {a.jobs} parallel, timeout {timeout:.0f} s each", flush=True)

results = {}
def one(p):
    st, detail, s = run(p["id"], timeout)
    results[p["id"]] = (st, detail, s)
    return p, st, detail, s

def label(p):
    return f'{p["file"]}:{p["line"]}:{p["col"]}'

with ThreadPoolExecutor(a.jobs) as ex:
    for p, st, detail, s in ex.map(one, live):
        verdict = {"ok": "SURVIVED", "fail": "killed  ", "timeout": "killed  "}[st]
        extra = {"ok": "", "fail": f"  by {detail}", "timeout": "  (timeout)"}[st]
        if a.quiet and st != "ok":
            continue
        print(f'{verdict} #{p["id"]:<4} {label(p)}  [{p["fn"] or "-"}] {p["desc"]}:  {p["orig"]}  =>  {p["mutated"]}{extra}', flush=True)

killed = sum(1 for v in results.values() if v[0] != "ok")
surv = [p for p in live if results[p["id"]][0] == "ok"]
print()
print(f"{len(live)} mutants run: {killed} killed, {len(surv)} survived"
      + (f" ({100 * killed / len(live):.0f}% killed)" if live else "")
      + f"; {len(unviable)} points not compilable under the switch (skipped); {time.time() - t0:.1f} s")
if surv:
    print("Survivors (write a test that fails for each, or mark an equivalent one `// mutants:skip` in the source):")
    for p in surv:
        print(f'  #{p["id"]:<4} {label(p)}  [{p["fn"] or "-"}] {p["desc"]}:  {p["orig"]}  =>  {p["mutated"]}')
if a.out:
    json.dump({"baseline_s": base_s, "mutants": [dict(p, status=results[p["id"]][0], by=results[p["id"]][1]) for p in live],
               "unviable": [p["id"] for p in unviable]}, open(a.out, "w"), indent=1)
sys.exit(0)
