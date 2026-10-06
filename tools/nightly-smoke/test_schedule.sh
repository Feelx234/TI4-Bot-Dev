#!/usr/bin/env bash
# Scripted tests for the nightly schedule: window maths (incl. daylight saving), what `tick` would
# start, and a dry run of a whole night with stub `claude` binaries on a fake clock. Needs only
# bash, git, flock and python3: no real proctors, fixers, games or builds, and it never touches
# the live checkout or nightly-reports (everything runs in a temp dir).
#
#   tools/nightly-smoke/test_schedule.sh
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
TMP=$(mktemp -d)
trap 'kill $(jobs -p) 2>/dev/null; rm -rf "$TMP"' EXIT

checks=0
failures=0
check() { # check <name> <actual> <expected>
  checks=$((checks + 1))
  if [ "$2" = "$3" ]; then echo "ok    $1"; else echo "FAIL  $1: expected [$3], got [$2]"; failures=$((failures + 1)); fi
}
check_true() { # check_true <name> <command...>
  local name="$1"; shift
  checks=$((checks + 1))
  if "$@"; then echo "ok    $name"; else echo "FAIL  $name"; failures=$((failures + 1)); fi
}
epoch() { TZ=Europe/Berlin date -d "$1" +%s; }

# ---------------------------------------------------------------------------------------------
echo "== window maths (09:00 -> 06:00 Berlin, crossing midnight)"
window_at() { # window_at "<Berlin date time>" -> "<night> <start> <end>"
  NIGHTLY_NOW=$(epoch "$1") NIGHTLY_REPO=/nonexistent bash -c "source '$HERE/config.sh'; current_window"
}
expect_window() { # expect_window "<time>" <night> <next-day>
  check "window at $1 is night $2" "$(window_at "$1")" "$2 $(epoch "$2 09:00") $(epoch "$3 06:00")"
}
expect_window "2026-10-07 08:59" 2026-10-06 2026-10-07
expect_window "2026-10-07 09:00" 2026-10-07 2026-10-08
expect_window "2026-10-07 12:00" 2026-10-07 2026-10-08
expect_window "2026-10-07 23:30" 2026-10-07 2026-10-08
expect_window "2026-10-08 00:30" 2026-10-07 2026-10-08
expect_window "2026-10-08 05:59" 2026-10-07 2026-10-08
expect_window "2026-10-08 06:00" 2026-10-07 2026-10-08
expect_window "2026-10-08 06:05" 2026-10-07 2026-10-08
# Daylight saving: clocks go back on 2026-10-25 (the window is an hour longer) and forward on
# 2026-03-29 (an hour shorter); the window must still start at 09:00 and end at 06:00 local time.
expect_window "2026-10-25 05:59" 2026-10-24 2026-10-25
expect_window "2026-10-25 06:05" 2026-10-24 2026-10-25
expect_window "2026-10-25 09:00" 2026-10-25 2026-10-26
expect_window "2026-03-29 05:59" 2026-03-28 2026-03-29
expect_window "2026-03-29 06:05" 2026-03-28 2026-03-29
expect_window "2026-03-29 09:00" 2026-03-29 2026-03-30
len() { NIGHTLY_REPO=/nonexistent bash -c "source '$HERE/config.sh'; read -r s e < <(window_for_night $1); echo \$((e - s))"; }
check "window 2026-10-24 is 22 h (fall back)" "$(len 2026-10-24)" 79200
check "window 2026-03-28 is 20 h (spring forward)" "$(len 2026-03-28)" 72000
check "window 2026-10-07 is 21 h" "$(len 2026-10-07)" 75600

# ---------------------------------------------------------------------------------------------
echo "== tick decisions"
RR="$TMP/decide"
decide() { # decide "<Berlin time>" [ENV=VALUE...]
  local when="$1"; shift
  env NIGHTLY_NOW="$(epoch "$when")" NIGHTLY_REPORT_ROOT="$RR" NIGHTLY_REPO=/nonexistent "$@" \
    bash "$HERE/nightly.sh" tick-decide | tr '\n' '|' | sed 's/|$//'
}
hold() { ( exec 9>"$1"; flock -n 9 || exit 1; exec sleep 120 ) & HOLD=$!; sleep 0.3; }
release() { kill "$HOLD" 2>/dev/null; wait "$HOLD" 2>/dev/null; }
reset_night() { rm -rf "$RR"; mkdir -p "$RR/$1"; }
entry() { printf '## Run 01-0900 — clean\n' > "$RR/$1/report.md"; }
N=2026-10-07

reset_night 2026-10-06
check "before NOT_BEFORE: nothing at 2026-10-06 12:30" "$(decide '2026-10-06 12:30')" ""
check "before NOT_BEFORE: nothing at 2026-10-06 23:00" "$(decide '2026-10-06 23:00')" ""
check "before NOT_BEFORE: nothing at 2026-10-07 07:00 (old night 10-06)" "$(decide '2026-10-07 07:00')" ""
check "NOT_BEFORE can be lowered" "$(decide '2026-10-06 09:02' NIGHTLY_NOT_BEFORE=2026-10-06)" "sweep 2026-10-06"

reset_night $N
check "08:59: not yet" "$(decide '2026-10-07 08:59')" ""
check "09:02: sweep starts" "$(decide '2026-10-07 09:02')" "sweep $N"
touch "$RR/$N/sweep.done"
check "no second sweep once sweep.done exists; round 2 skipped, no entries" "$(decide '2026-10-07 09:02')" "skip-fix $N 2"
check "no second sweep once sweep.done exists (fixers disabled)" "$(decide '2026-10-07 09:02' NIGHTLY_FIXERS=)" ""

reset_night $N; mkdir -p "$RR/$N"; touch "$RR/$N/.loop.lock"; hold "$RR/$N/.loop.lock"
check "sweep running, no entries: round 1 waits at 12:00" "$(decide '2026-10-07 12:00')" ""
entry $N
check "sweep running, entry: 11:59 nothing" "$(decide '2026-10-07 11:59')" ""
check "sweep running, entry: 12:00 round 1" "$(decide '2026-10-07 12:00')" "fix $N 1"
check "round 1 stays due until it has started (16:00)" "$(decide '2026-10-07 16:00')" "fix $N 1"
touch "$RR/$N/.fixer-1-started"
check "round 1 not started twice" "$(decide '2026-10-07 12:05')" ""
rm "$RR/$N/.fixer-1-started"
check "FIXERS=2 disables round 1" "$(decide '2026-10-07 12:00' NIGHTLY_FIXERS=2)" ""
check "FIXERS empty disables round 1" "$(decide '2026-10-07 12:00' NIGHTLY_FIXERS=)" ""
check "FIX1 time is configurable" "$(decide '2026-10-07 13:30' NIGHTLY_FIX1=14:00)" ""
check "FIX1 time is configurable (due)" "$(decide '2026-10-07 14:00' NIGHTLY_FIX1=14:00)" "fix $N 1"
check "round 1 still due inside the window (05:50)" "$(decide '2026-10-08 05:50')" "fix $N 1"
touch "$RR/$N/.fixer-1-started"
check "sweep running at 05:30: nothing" "$(decide '2026-10-08 05:30')" ""
check "sweep running past END: nothing (no summary, no round 2)" "$(decide '2026-10-08 06:05')" ""
release

# Round 2 and the summary.
reset_night $N; entry $N; touch "$RR/$N/sweep.done" "$RR/$N/.fixer-1-started" "$RR/$N/.fixer-1.done"
check "round 2 starts after sweep.done (06:00)" "$(decide '2026-10-08 06:00')" "fix $N 2"
check "round 2 starts after sweep.done (even early)" "$(decide '2026-10-08 05:40')" "fix $N 2"
check "round 2 disabled: summary at 06:00" "$(decide '2026-10-08 06:00' NIGHTLY_FIXERS=1)" "summary $N"
check "round 2 disabled: no summary before END" "$(decide '2026-10-08 05:40' NIGHTLY_FIXERS=1)" ""
touch "$RR/$N/.fixer-2-started"
check "round 2 running: summary waits" "$(decide '2026-10-08 06:30')" ""
check "round 2 hopelessly late: summary goes ahead" "$(decide '2026-10-08 09:40')" "summary $N"
touch "$RR/$N/.fixer-2.done"
check "round 2 done: summary" "$(decide '2026-10-08 06:30')" "summary $N"
touch "$RR/$N/.summary-started"
check "summary not started twice" "$(decide '2026-10-08 06:35')" ""
reset_night $N; touch "$RR/$N/sweep.done"
check "sweep ended without any run entry: round 2 is skipped" "$(decide '2026-10-08 06:00')" "skip-fix $N 2"

# The 06:00-09:00 gap: round 2 and the summary of the old night come first, the next sweep waits.
P=2026-10-07; C=2026-10-08
reset_night $P; entry $P; touch "$RR/$P/sweep.done" "$RR/$P/.fixer-1-started" "$RR/$P/.fixer-1.done"
check "09:05: old night's round 2 due, new sweep waits" "$(decide '2026-10-08 09:05')" "fix $P 2"
touch "$RR/$P/.fixer-2-started" "$RR/$P/.fixer-2.done"
check "09:05: old night's summary due, new sweep waits" "$(decide '2026-10-08 09:05')" "summary $P"
touch "$RR/$P/.summary-started" "$RR/$P/summary.md"
check "09:05: old night finished, new sweep starts" "$(decide '2026-10-08 09:05')" "sweep $C"
rm "$RR/$P/summary.md"; touch "$RR/$P/.loop.lock"; hold "$RR/$P/.fixer-2.lock"
check "09:05: old round 2 running, new sweep waits" "$(decide '2026-10-08 09:05')" ""
check "11:05: waited long enough, new sweep starts anyway" "$(decide '2026-10-08 11:05')" "sweep $C"
release

# ---------------------------------------------------------------------------------------------
echo "== dry run of a whole night with stub claude binaries"
setup_env() { # setup_env <dir>: temp repo with the scripts under test, stubs, fake clock
  E="$1"; mkdir -p "$E"
  export NIGHTLY_REPO="$E/repo" NIGHTLY_REPORT_ROOT="$E/reports" NIGHTLY_NOW_FILE="$E/clock" \
    NIGHTLY_CLAUDE="$E/claude" NIGHTLY_BUILD_CMD="${BUILD_CMD:-true}" NIGHTLY_NOT_BEFORE=2026-10-07 \
    STUB_LOG="$E/stub.log" STUB_DIR="$E" NIGHTLY_SH="$E/repo/tools/nightly-smoke/nightly.sh" \
    GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
  git init -q -b main "$NIGHTLY_REPO"
  echo hi > "$NIGHTLY_REPO/README"
  mkdir -p "$NIGHTLY_REPO/tools"; cp -r "$HERE" "$NIGHTLY_REPO/tools/nightly-smoke"
  git -C "$NIGHTLY_REPO" add -A; git -C "$NIGHTLY_REPO" commit -q -m init
  echo wip > "$NIGHTLY_REPO/wip.txt" # uncommitted work: must be saved by the snapshot
  : > "$STUB_LOG"
  cat > "$NIGHTLY_CLAUDE" <<'STUB'
#!/usr/bin/env bash
model=""; prompt=""
while [ $# -gt 0 ]; do
  case "$1" in --model) model="$2"; shift 2 ;; --) shift; prompt="$1"; break ;; *) shift ;; esac
done
now=$(cat "$NIGHTLY_NOW_FILE")
case "$prompt" in
  *"nightly fixer"*)
    round=$(printf '%s' "$prompt" | sed -n 's/.*(round \([0-9]*\)).*/\1/p' | head -1)
    echo "fixer$round model=$model clock=$now cwd=$(pwd)" >> "$STUB_LOG"
    echo "fix $round" > "fix$round.txt"; git add "fix$round.txt"; git commit -q -m "stub fix round $round"
    if [ -n "${STUB_BREAK_ROUND:-}" ] && [ "$round" = "$STUB_BREAK_ROUND" ]; then
      echo x > broken.flag; git add broken.flag; git commit -q -m "stub: breaks the build"
    fi
    printf '## Fixer round %s\n### Fixed\n- stub\n' "$round" ;;
  *"You are a **proctor**"*)
    rd=$(printf '%s' "$prompt" | sed -n 's/^Run directory: //p' | head -1)
    mkdir -p "$rd"; echo '{"preset":"stub"}' > "$rd/meta.json"
    present=no; [ -f fix1.txt ] && present=yes
    echo "proctor model=$model clock=$now fix1_present=$present" >> "$STUB_LOG"
    new=$((now + ${STUB_RUN_SECONDS:-3000})); echo "$new" > "$NIGHTLY_NOW_FILE"
    # what cron would do every five minutes while the game runs
    "$NIGHTLY_SH" tick > /dev/null 2>&1
    for _ in $(seq 1 100); do # let a fixer started by that tick finish before the proctor ends
      pending=no
      for f in "$STUB_DIR"/reports/*/.fixer-*-started; do
        [ -e "$f" ] || continue
        [ -e "${f%-started}.done" ] || pending=yes
      done
      [ "$pending" = no ] && break
      sleep 0.2
    done
    printf '## Run %s — clean (stub)\n' "$(basename "$rd")" ;;
  *)
    printf '%s' "$prompt" > "$STUB_DIR/prompt-summary.txt"
    echo "summary model=$model clock=$now" >> "$STUB_LOG"
    printf '# Summary stub\n' ;;
esac
STUB
  chmod +x "$NIGHTLY_CLAUDE"
}
wait_for() { # wait_for <file> [seconds]
  local i; for i in $(seq 1 $((${2:-30} * 5))); do [ -e "$1" ] && return 0; sleep 0.2; done; return 1
}
ns() { bash "$NIGHTLY_SH" "$@"; }
repo() { git -C "$NIGHTLY_REPO" "$@"; }

BUILD_CMD=true setup_env "$TMP/night"
echo "$(epoch '2026-10-07 09:00')" > "$NIGHTLY_NOW_FILE"
NIGHT=2026-10-07; ND="$NIGHTLY_REPORT_ROOT/$NIGHT"
check "tick at 09:00 decides to start the sweep" "$(ns tick-decide)" "sweep $NIGHT"
timeout 600 bash "$NIGHTLY_SH" loop "$NIGHT" > "$TMP/night/loop.out" 2>&1
check "loop finished and wrote sweep.done" "$([ -e "$ND/sweep.done" ] && echo yes || echo no)" yes
check "checkout is on the night branch" "$(repo branch --show-current)" "nightly-fixes-$NIGHT"
check "orig branch recorded" "$(cat "$ND/orig_branch")" main
check "snapshot saved the uncommitted work" "$(repo show nightly-fixes-$NIGHT:wip.txt 2>/dev/null | head -1)" wip
check_true "snapshot commit exists" bash -c "git -C '$NIGHTLY_REPO' log --format=%s | grep -q 'snapshot of the working tree'"
check "proctor ran on sonnet" "$(grep '^proctor' "$STUB_LOG" | grep -vc 'model=claude-sonnet-5-5')" 0
check "round 1 fixer started exactly once" "$(grep -c '^fixer1' "$STUB_LOG")" 1
check "round 1 ran on opus" "$(grep '^fixer1' "$STUB_LOG" | grep -c 'model=claude-opus-5-5')" 1
check_true "round 1 worked in its own worktree" bash -c "grep '^fixer1' '$STUB_LOG' | grep -q 'cwd=$ND/fixer-1'"
fix1_clock=$(grep '^fixer1' "$STUB_LOG" | sed 's/.*clock=\([0-9]*\).*/\1/')
check_true "round 1 started at or after 12:00 and before 13:00" \
  bash -c "[ $fix1_clock -ge $(epoch '2026-10-07 12:00') ] && [ $fix1_clock -lt $(epoch '2026-10-07 13:00') ]"
# The commit arrives between games: no proctor before the fixer sees it, the next one does.
first_after=$(awk '/^fixer1/{f=1;next} f && /^proctor/{print; exit}' "$STUB_LOG")
check "the proctor right after round 1 already has its commit" "$(echo "$first_after" | grep -c 'fix1_present=yes')" 1
check "no proctor before round 1 had it" "$(awk '/^fixer1/{exit} /^proctor/' "$STUB_LOG" | grep -c 'fix1_present=yes')" 0
check "all later proctors have it" "$(awk '/^fixer1/{f=1;next} f && /^proctor/' "$STUB_LOG" | grep -vc 'fix1_present=yes')" 0
check_true "merge note sits between two run entries in the report" \
  awk '/^## Run /{if(m)a=1; else b=1} /^## Fixer round 1: merged/{m=1} END{exit !(a&&b&&m)}' "$ND/report.md"
check "round 1 marked merged" "$([ -e "$ND/.fixer-1.merged" ] && echo yes || echo no)" yes
check "round 2 not started by the sweep's ticks" "$(grep -c '^fixer2' "$STUB_LOG")" 0

# Night is over: the sweep ended at about 06:00. Round 2 is due; the summary is not.
echo "$(epoch '2026-10-08 06:00')" > "$NIGHTLY_NOW_FILE"
check "after sweep.done: round 2 is due" "$(ns tick-decide)" "fix $NIGHT 2"
check "summary not started before round 2" "$([ -e "$ND/.summary-started" ] && echo yes || echo no)" no
ns tick > /dev/null 2>&1
check_true "round 2 finished" wait_for "$ND/.fixer-2.done"
check "round 2 ran on opus in its own worktree" "$(grep '^fixer2' "$STUB_LOG" | grep -c "model=claude-opus-5-5 .*cwd=$ND/fixer-2")" 1
check "round 2 commit merged into the night branch" "$(repo show nightly-fixes-$NIGHT:fix2.txt 2>/dev/null | head -1)" "fix 2"
check "round 1 commit still there" "$(repo show nightly-fixes-$NIGHT:fix1.txt 2>/dev/null | head -1)" "fix 1"
check "round 2 marked merged" "$([ -e "$ND/.fixer-2.merged" ] && echo yes || echo no)" yes
check "checkout still on the night branch, clean" "$(repo branch --show-current) $(repo status --porcelain | wc -l)" "nightly-fixes-$NIGHT 0"
check "summary only now due" "$(ns tick-decide)" "summary $NIGHT"
check "summary has not run yet" "$(grep -c '^summary' "$STUB_LOG")" 0
ns tick > /dev/null 2>&1
check_true "summary written" wait_for "$ND/summary.md"
check "summary ran on opus" "$(grep -c '^summary model=claude-opus-5-5' "$STUB_LOG")" 1
check "summary ran after round 2" "$(awk '/^fixer2/{f=1} f && /^summary/{print "after"; exit}' "$STUB_LOG")" after
check_true "summary prompt names both fixer branches" bash -c "grep -q 'nightly-opus-$NIGHT-r1' '$TMP/night/prompt-summary.txt' && grep -q 'nightly-opus-$NIGHT-r2' '$TMP/night/prompt-summary.txt'"
check "nothing left to do" "$(ns tick-decide)" ""

# A fixer merge that breaks the build is dropped before the game runs, and marked rejected.
echo "== merge that breaks the build is dropped"
export STUB_BREAK_ROUND=1
BUILD_CMD='test ! -f broken.flag' setup_env "$TMP/broken"
echo "$(epoch '2026-10-07 11:00')" > "$NIGHTLY_NOW_FILE"
NIGHT=2026-10-07; ND="$NIGHTLY_REPORT_ROOT/$NIGHT"
export NIGHTLY_END=14:00 NIGHTLY_START=09:00 STUB_RUN_SECONDS=1800 NIGHTLY_MIN_RUN_SECONDS=600
# Two runs fit before the fix round is due; the fixer's commit then breaks the build.
export NIGHTLY_FIX1=11:50
timeout 300 bash "$NIGHTLY_SH" loop "$NIGHT" > "$TMP/broken/loop.out" 2>&1
check "broken merge rejected" "$([ -e "$ND/.fixer-1.rejected" ] && echo yes || echo no)" yes
check "broken file is not in the checkout" "$([ -e "$NIGHTLY_REPO/broken.flag" ] && echo yes || echo no)" no
check_true "report says the merge broke the build" grep -q "broke the build" "$ND/report.md"
check "the fixer's good commit was dropped with it (branch kept)" "$(repo show nightly-opus-$NIGHT-r1:fix1.txt 2>/dev/null | head -1)" "fix 1"
unset STUB_BREAK_ROUND NIGHTLY_END NIGHTLY_START STUB_RUN_SECONDS NIGHTLY_MIN_RUN_SECONDS NIGHTLY_FIX1

# A merge conflict is aborted and leaves the tree clean.
echo "== merge conflict is aborted"
BUILD_CMD=true setup_env "$TMP/conflict"
echo "$(epoch '2026-10-07 09:00')" > "$NIGHTLY_NOW_FILE"
NIGHT=2026-10-07; ND="$NIGHTLY_REPORT_ROOT/$NIGHT"
repo checkout -q -b evil; echo "from the fixer" > "$NIGHTLY_REPO/README"; repo commit -q -am evil; repo checkout -q main
echo "from the checkout" > "$NIGHTLY_REPO/README"
mkdir -p "$ND"; echo evil > "$ND/fixer-1.ready"
export NIGHTLY_END=11:00 NIGHTLY_START=09:00 STUB_RUN_SECONDS=1800 NIGHTLY_MIN_RUN_SECONDS=600 NIGHTLY_FIXERS=
timeout 300 bash "$NIGHTLY_SH" loop "$NIGHT" > "$TMP/conflict/loop.out" 2>&1
check "conflicting merge marked conflict" "$([ -e "$ND/.fixer-1.conflict" ] && echo yes || echo no)" yes
check "tree clean after the aborted merge" "$(repo status --porcelain | wc -l)" 0
check "checkout content kept" "$(head -1 "$NIGHTLY_REPO/README")" "from the checkout"
unset NIGHTLY_END NIGHTLY_START STUB_RUN_SECONDS NIGHTLY_MIN_RUN_SECONDS NIGHTLY_FIXERS

echo
if [ "$failures" -eq 0 ]; then echo "all $checks checks passed"; else echo "$failures of $checks checks FAILED"; fi
[ "$failures" -eq 0 ]
