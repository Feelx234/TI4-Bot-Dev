#!/usr/bin/env bash
# Seeded private CARGO_TARGET_DIRs (docs/ENGINE_COMPUTE.md 2).
#
#   tools/engine_core/target_dir.sh warm  [SEED] [CARGO_ARGS...]   build/refresh the shared warm dir
#   tools/engine_core/target_dir.sh seed  DEST [SEED]               make DEST a private dir seeded from SEED
#   tools/engine_core/target_dir.sh path                            print the default SEED
#   tools/engine_core/target_dir.sh rm    DEST                      delete a seeded dir
#
# SEED defaults to $TI4_WARM_TARGET or /root/deploytmp/warm-target. Cargo cannot share artifacts between
# checkouts through its cache alone (no sccache here) and the disk has no reflinks, so a seed is made by
# HARD-LINKING the big finished artifacts (deps/, build/ of every profile; not incremental/) of the warm dir and
# copying the small metadata (.fingerprint/, .d files): a seeded dir costs seconds and little disk. Cargo
# replaces (never edits) the big artifacts it rebuilds, but rewrites the small files in place, hence the copies.
#
# Why the mtimes are touched: cargo calls a workspace crate fresh only when its sources are not newer than
# the previous build. A new worktree has all-new mtimes. `seed` therefore sets the mtime of every tracked
# file that is IDENTICAL to the warm commit (recorded in SEED/.warm-commit by `warm`) to just before the
# START of the warm build (SEED/.warm-time); files that differ keep their time, so cargo rebuilds exactly them and their dependents.
# This changes only mtimes of files in the checkout the script lives in.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
cmd="${1:-}"; shift || true
default_seed="${TI4_WARM_TARGET:-/root/deploytmp/warm-target}"
case "$cmd" in
  path) echo "$default_seed" ;;
  warm)
    seed="${1:-$default_seed}"; [[ $# -gt 0 ]] && shift
    args=("$@"); [[ ${#args[@]} == 0 ]] && args=(test --profile gate -p ti4-engine-legacy --lib --no-run)
    mkdir -p "$seed"
    # One warm build at a time on the dir; cargo takes its own lock too.
    exec 9> "$seed/.warm.lock"; flock 9
    t_start="$(date +%s)"   # BEFORE the build: cargo compares sources with the START of each compile
    ( cd "$repo" && CARGO_TARGET_DIR="$seed" cargo "${args[@]}" ) || exit 1
    git -C "$repo" rev-parse HEAD > "$seed/.warm-commit"
    echo "$t_start" > "$seed/.warm-time"
    echo "warm dir $seed at $(cat "$seed/.warm-commit")" ;;
  seed)
    dest="${1:?DEST}"; seed="${2:-$default_seed}"
    [[ -f "$seed/.warm-commit" ]] || { echo "no warm dir at $seed (run: $0 warm)"; exit 1; }
    [[ -e "$dest" ]] && [[ -n "$(ls -A "$dest" 2>/dev/null)" ]] && { echo "$dest is not empty"; exit 1; }
    mkdir -p "$dest"
    t0=$SECONDS
    # Big artifacts (rlib, rmeta, so, executables, objects; >= 64 KB) are hard-linked; everything small
    # (.fingerprint, .d files, build-script outputs) is COPIED, because cargo rewrites those in place when it
    # rebuilds a crate and that must not reach the warm dir.
    python3 -I - "$seed" "$dest" <<'PY'
import os, shutil, sys
seed, dest = sys.argv[1:3]
links = copies = 0
for prof in sorted(os.listdir(seed)):
    pdir = os.path.join(seed, prof)
    if not os.path.isdir(pdir) or prof == "incremental":
        continue
    for sub in ("deps", "build", ".fingerprint"):
        top = os.path.join(pdir, sub)
        if not os.path.isdir(top):
            continue
        for root, dirs, files in os.walk(top):
            rel = os.path.relpath(root, seed)
            os.makedirs(os.path.join(dest, rel), exist_ok=True)
            for f in files:
                a, b = os.path.join(root, f), os.path.join(dest, rel, f)
                if os.path.islink(a):
                    os.symlink(os.readlink(a), b)
                elif os.path.getsize(a) >= 65536 and not a.endswith((".json", ".d")):
                    os.link(a, b); links += 1
                else:
                    shutil.copy2(a, b); copies += 1
print(f"{links} files hard-linked, {copies} small files copied")
PY
    # Cargo's own marker files.
    cp -a "$seed"/.rustc_info.json "$seed"/CACHEDIR.TAG "$dest"/ 2>/dev/null || true
    wc="$(cat "$seed/.warm-commit")"; wt="$(cat "$seed/.warm-time")"
    if git -C "$repo" cat-file -e "$wc^{commit}" 2>/dev/null; then
      cd "$repo"
      changed="$(mktemp)"; git diff --name-only -z "$wc" -- > "$changed"
      git ls-files --others --exclude-standard -z >> "$changed"
      # touch all tracked files not in `changed` to just before the warm build.
      git ls-files -z | python3 -I -c '
import sys, os
changed = set(open(sys.argv[1], "rb").read().split(b"\0")) - {b""}
t = int(sys.argv[2]) - 5
keep_dirs = {os.path.dirname(c) for c in changed}   # a deleted/added file shows in its directory time
n = 0; dirs = set()
for f in sys.stdin.buffer.read().split(b"\0"):
    if not f: continue
    d = os.path.dirname(f)
    while d and d not in dirs:
        dirs.add(d); d = os.path.dirname(d)
    if f not in changed and os.path.isfile(f) and not os.path.islink(f):
        os.utime(f, (t, t)); n += 1
nd = 0
for d in sorted(dirs, key=len, reverse=True):          # children first
    if d not in keep_dirs and os.path.isdir(d):
        os.utime(d, (t, t)); nd += 1
print(f"{n} unchanged files and {nd} directories set to before the warm build, {len(changed)} changed/untracked kept")
' "$changed" "$wt"
      rm -f "$changed"
    else
      echo "warm commit $wc is not in this repository: mtimes left alone (workspace crates will rebuild)"
    fi
    echo "seeded $dest from $seed ($((SECONDS - t0)) s); export CARGO_TARGET_DIR=$dest" ;;
  rm) dest="${1:?DEST}"; [[ "$dest" == /root/deploytmp/* ]] || { echo "refusing to delete outside /root/deploytmp"; exit 1; }; rm -rf "$dest" ;;
  *) sed -n '2,17p' "${BASH_SOURCE[0]}"; exit 2 ;;
esac
