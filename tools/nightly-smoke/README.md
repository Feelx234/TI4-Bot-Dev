# Nightly UI smoke sweep

Random-click UI playthroughs of the TI4 web client, proctored by Sonnet agents, with two Opus
fix rounds and an Opus morning summary. Cron calls `nightly.sh tick` every five minutes
(`*/5 * * * * /root/TI4-Bot-Dev/tools/nightly-smoke/nightly.sh tick >> nightly-reports/cron.log`);
`tick` decides what is due.

## Schedule (Berlin time)

| When | What |
|---|---|
| 20:30 | the sweep starts: one proctored game after another (Sonnet) until about 06:00 |
| 23:59 (or earlier on a proctor's request) | Opus fix round 1 starts while the sweep keeps running |
| about 06:00 next day | the last proctor finishes, `sweep.done` appears, Opus fix round 2 starts |
| after round 2 | the Opus morning summary (it lists both fix rounds) |

A night is named after the Berlin date on which its window started (`nightly-reports/2026-10-07/`).
The window is 9.5 hours, 10.5 or 8.5 on the two daylight-saving days. Day arithmetic goes through the
calendar, never "minus 86400".

**First night: 2026-10-06.** `NIGHTLY_NOT_BEFORE` (default `2026-10-06`) makes `tick` ignore every
earlier night, so switching to this schedule never switches a checkout people are working in.

**The 06:00 to 20:30 gap.** Round 2 may use up to 3 hours and the summary up to one more,
so they can run after the next window has started. `tick` therefore also looks at the previous
night, and the next sweep waits while the previous night's round 2 or summary is running or due
(at most `NIGHTLY_START_DEFER_SECONDS`, default 2 hours, past 20:30). Both use the same checkout.

**Early fix round.** Round 1 is scheduled for 23:59, but a proctor may request it earlier:
`request_fix.sh <reason...>` writes `nightly-reports/<night>/fix-requested` (time, run name,
reason). `tick` then treats round 1 as due (it still needs one report entry, round 1 enabled and
not yet started, the sweep running and now before END). Only the first request of a night counts;
the script refuses when fixers are disabled or round 1 already started. The reason goes into the
fixer prompt (`{{REQUEST}}` in `prompts/fixer.md`, empty for a scheduled round) and a line in
`report.md` notes the early start. The proctor prompt tells proctors to ask only for a defect that
will make several following runs fail the same way, at most once per night; the script is on their
allowed-tools list.

## How a night runs

1. The sweep saves the checkout, uncommitted work included, as one snapshot commit on the branch
   `nightly-fixes-<night>`, checks that branch out in the live checkout and plays there. The
   branch the checkout was on is recorded in `nightly-reports/<night>/orig_branch`. Nothing is
   pushed.
2. Before every game, `$NIGHTLY_BUILD_CMD` (default: build the server) must pass. Proctors may
   commit minor repairs on the night branch; a repair or merge that breaks the build is reset to
   the last good commit.
3. **Fix rounds** (`nightly.sh fix <round>`) run in their own git worktree
   `nightly-reports/<night>/fixer-<round>` on branch `nightly-opus-<night>-r<round>`, started from
   the night branch. They never touch the live checkout while games run. Each reads the report,
   clusters the findings, and for the most frequent real bugs writes a failing test, fixes it,
   runs the targeted tests and commits (at most 6 fixes, wall-clock budget 4 h for round 1 and
   3 h for round 2). Rules-timing and engine-semantics changes are left to the user and listed in
   `fixer-<round>.md`.
4. **Hand-off:** a finished round with commits leaves `fixer-<round>.ready`. The sweep merges a
   ready branch into the night branch only between games (before the build check), so a game never
   runs on sources that change underneath it. Round 2 merges directly once the sweep has ended and
   build-checks the result. Outcomes are marker files and lines in `report.md`: `.fixer-N.merged`,
   `.fixer-N.rejected` (the merge broke the build and was dropped), `.fixer-N.conflict` (aborted).
   A branch is never deleted; it stays for the user.
5. The morning summary (`summary.md`) waits for round 2 (done, skipped, disabled, or more than
   `FIX2_MAX_SECONDS` + 30 minutes late).

In the morning: `git checkout <orig_branch>` and merge or cherry-pick from `nightly-fixes-<night>`.
Worktrees under `nightly-reports/<night>/fixer-*` can be removed with `git worktree remove`.

## Commands

    nightly.sh tick                 start whatever is due (cron)
    nightly.sh tick-decide          print what tick would start, change nothing
    nightly.sh loop [night]         the sweep
    nightly.sh fix <round> [night]  an Opus fix round
    nightly.sh summary [night]      the morning summary
    request_fix.sh <reason...>      (proctors) ask for fix round 1 before 23:59

## Settings (environment variables, defaults in `config.sh`)

| Variable | Default | Meaning |
|---|---|---|
| `NIGHTLY_START`, `NIGHTLY_END` | `20:30`, `06:00` | window, Berlin time (`NIGHTLY_TZ`) |
| `NIGHTLY_NOT_BEFORE` | `2026-10-06` | earlier nights are ignored by `tick` |
| `NIGHTLY_FIXERS` | `1 2` | rounds that run; `1`, `2` or an empty value (`NIGHTLY_FIXERS=`) to disable |
| `NIGHTLY_FIXER_MODEL` | `claude-opus-5-5` | model of the fix rounds |
| `NIGHTLY_FIX1` | `23:59` | start of round 1 (a proctor request starts it earlier) |
| `NIGHTLY_FIX1_MAX_SECONDS`, `NIGHTLY_FIX2_MAX_SECONDS` | `14400`, `10800` | wall-clock budgets |
| `NIGHTLY_MAX_FIXES` | `6` | fixes per round |
| `NIGHTLY_START_DEFER_SECONDS` | `7200` | how long the next sweep waits for the old night |
| `NIGHTLY_BUILD_CMD` | `cargo build --quiet -p ti4-server --bin server` | check before every game and after merges |
| `NIGHTLY_PROCTOR_MODEL`, `NIGHTLY_SUMMARY_MODEL` | Sonnet 5.5, Opus 5.5 | |
| `NIGHTLY_PRESET_PROBABILITY` | `70` | percent of runs that start from a prepared state |
| `NIGHTLY_PRESET` | `combat combat cards agenda relics invasion techs leaders` | one preset name or a list to pick from per run (see `crates/ti4-server/src/preset.rs`; `endgame` is left out of the mix) |
| `NIGHTLY_PRESET_ROTATE_PERCENT` | `20` | percent of preset runs that also rotate the factions (`<preset>+rot`: Jol-Nar and L1Z1X at small tables) |
| `NIGHTLY_NOW`, `NIGHTLY_NOW_FILE` | unset | fake clock for tests |

To switch the Opus rounds off: `NIGHTLY_FIXERS=` in the cron line. Two long Opus sessions per night
are the main cost of this schedule.

## Tests

    tools/nightly-smoke/test_schedule.sh

Runs in a temp directory with stub `claude` binaries and a fake clock: window maths on both sides
of midnight and both daylight-saving changes, the not-before guard, what `tick` starts at each time
(round 1 once at 23:59 or on an early request, request refused after round 1 started or when disabled, the reason reaching the fixer prompt and the report, round 2 only after `sweep.done`, the summary only after round 2, the
previous-night gap), a dry run of a whole night including the between-games merge, a merge that
breaks the build, and a merge conflict. It never starts a real proctor, game or build and never
touches the live checkout. `test_preset_pick.sh` tests the preset choice.
`coverage.py <reports-dir>...` lists the decision subtypes no run ever offered (and those offered in
fewer than two runs) by diffing the engine's literal subtypes against the runs' `report.json`; `test_coverage.sh` tests it.
