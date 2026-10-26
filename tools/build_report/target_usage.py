#!/usr/bin/env python3
"""Where the disk of a cargo target directory goes.

usage: target_usage.py <target-dir> [top-N]

Prints, per profile directory (debug, release, <triple>/...): the size of deps/, incremental/,
build/, examples/ and the loose files; then deps/ by file kind (rlib, rmeta, executables, .so,
.dwp, .d, .o) and by crate (the top N, with the number of hashed copies of the crate) and
incremental/ by crate.
"""
import os, re, sys, collections

root = sys.argv[1]
topn = int(sys.argv[2]) if len(sys.argv) > 2 else 15

def du(path):
    total = 0
    for dp, dn, fn in os.walk(path):
        for f in fn:
            try:
                total += os.lstat(os.path.join(dp, f)).st_size
            except OSError:
                pass
    return total

def mb(n):
    return f"{n/1e6:9.0f} MB"

def profile_dirs(base):
    for name in sorted(os.listdir(base)):
        p = os.path.join(base, name)
        if not os.path.isdir(p):
            continue
        if os.path.isdir(os.path.join(p, "deps")) or os.path.isdir(os.path.join(p, "incremental")) or os.path.isdir(os.path.join(p, "build")):
            yield name, p
        elif name not in ("cargo-timings", "doc") and any(os.path.isdir(os.path.join(p, s, "deps")) for s in os.listdir(p) if os.path.isdir(os.path.join(p, s))):
            for sub, sp in profile_dirs(p):
                yield f"{name}/{sub}", sp

print(f"total {mb(du(root))}  {root}\n")
for name, p in profile_dirs(root):
    print(f"== {name}   {mb(du(p))}")
    for sub in ("deps", "incremental", "build", "examples", ".fingerprint"):
        sp = os.path.join(p, sub)
        if os.path.isdir(sp):
            print(f"   {sub:<14}{mb(du(sp))}")
    loose = sum(os.lstat(os.path.join(p, f)).st_size for f in os.listdir(p) if os.path.isfile(os.path.join(p, f)))
    print(f"   {'(loose files)':<14}{mb(loose)}")
    deps = os.path.join(p, "deps")
    if os.path.isdir(deps):
        kind = collections.Counter(); kcount = collections.Counter()
        crate = collections.Counter(); copies = collections.defaultdict(set)
        for f in os.listdir(deps):
            fp = os.path.join(deps, f)
            if not os.path.isfile(fp):
                continue
            size = os.lstat(fp).st_size
            m = re.match(r"^(?:lib)?(.+?)-([0-9a-f]{16})(\..*)?$", f)
            ext = (m.group(3) or "") if m else os.path.splitext(f)[1]
            if ext == ".rlib": k = "rlib"
            elif ext == ".rmeta": k = "rmeta"
            elif ext in (".so", ".dylib"): k = "shared lib (.so)"
            elif ext == ".dwp": k = ".dwp (split debuginfo)"
            elif ext == ".d": k = ".d (dep-info)"
            elif ext == ".o": k = ".o"
            elif ext == "" : k = "executable (test/bin)"
            else: k = ext
            kind[k] += size; kcount[k] += 1
            if m:
                crate[m.group(1)] += size; copies[m.group(1)].add(m.group(2))
        print("   deps/ by kind:")
        for k, v in kind.most_common():
            print(f"      {k:<26}{mb(v)}  {kcount[k]:>5} files")
        print(f"   deps/ by crate (top {topn}):")
        for c, v in crate.most_common(topn):
            print(f"      {c:<26}{mb(v)}  {len(copies[c])} hash(es)")
    inc = os.path.join(p, "incremental")
    if os.path.isdir(inc):
        ic = collections.Counter()
        for d in os.listdir(inc):
            ic[re.sub(r"-[0-9a-z]{13}$", "", d)] += du(os.path.join(inc, d))
        print(f"   incremental/ by crate (top {topn}):")
        for c, v in ic.most_common(topn):
            print(f"      {c:<26}{mb(v)}")
    print()
