#!/usr/bin/env bash
# A proctor's request to start Opus fix round 1 early (before FIX1_HHMM). Writes
# $NIGHT_DIR/fix-requested; nightly.sh decide_night then treats round 1 as due. Only the first
# request of a night counts. See README.md and prompts/proctor.md.
#
#   request_fix.sh <reason...>
set -uo pipefail
source "$(dirname "$0")/config.sh"

[ $# -ge 1 ] || { echo "usage: $0 <reason...>  (symptom, decision/round, evidence path)" >&2; exit 2; }
reason="$*"

read -r night _ _ < <(current_window)
NIGHT_DIR="$REPORT_ROOT/$night"
case " $FIXERS " in
  *" 1 "*) ;;
  *) echo "request_fix: fix round 1 is disabled (FIXERS='$FIXERS'); nothing requested" >&2; exit 1 ;;
esac
if [ -f "$NIGHT_DIR/.fixer-1-started" ]; then
  echo "request_fix: fix round 1 has already started for night $night; nothing requested" >&2
  exit 1
fi
mkdir -p "$NIGHT_DIR"
# A 0-byte marker (written while the disk was full) is no request; it must not block a real one.
[ -e "$NIGHT_DIR/fix-requested" ] && [ ! -s "$NIGHT_DIR/fix-requested" ] && rm -f "$NIGHT_DIR/fix-requested"
run=""
[ -d "$NIGHT_DIR/runs" ] && run=$(ls -1 "$NIGHT_DIR/runs" 2>/dev/null | tail -n 1)
if ( set -o noclobber
     { echo "time: $(TZ="$NIGHTLY_TZ" date -d "@$(now_epoch)" '+%F %T %Z')"
       echo "run: ${run:-unknown}"
       echo "reason: $reason"; } > "$NIGHT_DIR/fix-requested" ) 2>/dev/null; then
  echo "request_fix: recorded for night $night; fix round 1 starts at the next tick (needs at least one report entry)"
else
  echo "request_fix: already requested for night $night (only the first request counts):"
  sed 's/^/  /' "$NIGHT_DIR/fix-requested"
fi
