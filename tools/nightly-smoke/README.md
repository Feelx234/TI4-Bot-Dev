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

**Early fix rounds.** Proctors are told to ask for changes immediately when they see a real error
(crash, stuck game, rejected-submit loop, console error, broken layout, a UI element that does not
work), with a precise reproducible reason: `request_fix.sh <reason...>`. They do not wait for the
error to repeat. The first request of a night writes `nightly-reports/<night>/fix-requested` (time,
run name, reason); `tick` then treats round 1 as due (it still needs one report entry, round 1
enabled and not yet started, the sweep running and now before END). Once round 1 has started, the
next request writes `fix-requested-2` instead (also when round 1 is disabled but round 2 is on):
round 2 then starts early, as soon as round 1 has finished (`.fixer-1.done`), while the sweep keeps
running; its branch is merged between games like round 1's. Requests beyond that (a second
`fix-requested-2`, or round 2 already started, or no round enabled) are not recorded and the script
tells the proctor to put the problem in the report entry. The reason goes into the fixer prompt
(`{{REQUEST}}` in `prompts/fixer.md`, empty for a plain scheduled round) and a line in `report.md`
notes the request. The script is on the proctors' allowed-tools list. Proctors keep their
minor-repair rights; a proctor who repairs something still reports it. The prompt also has a
"new UI elements to watch for" section: the harness saves screenshots to `trace/shots/` (first
decision of rounds 1 to 3, first sight of the trade desk, production builder, payment bar, token
panel, secondary-prep chrome, combat summaries, toasts, objectives and ballot modals), and the entry
has `UI observations` and `Fix requests` sections.

**Strategy card set.** `run_game.sh` plays the Thunder's Edge cards (server default `te`) in 75% of
the runs and Prophecy of Kings (`pok`) in the rest (`NIGHTLY_POK_PROBABILITY`). `meta.json` has
`card_set`; the repro carries `TI4_SMOKE_CARD_SET=pok` only when it is not the default.

**Optional UI exercises** (`web/e2e/smokeExercises.ts`; switches also work in a manual run, e.g.
`TI4_SMOKE_UI_TOUR=1 TI4_SMOKE_RECAP=1 TI4_SMOKE_REDO=1 TI4_SMOKE_SHOT_CAP=all npm run test:e2e:smoke`).
`meta.json` has `exercises` and the repro carries the switches. Results are in `report.json`
(`exercises`, `findings`, `shots`) and the digest. None of them fails a run; a problem is a finding.
- UI tour: Faction card at a calm turn-menu moment, one unit card when a production builder is open
  (before anything is staged); asserts text, on-screen, unclipped, Escape closes it.
- Recap: the seat's browser gets `player_turn_recap=true`; recap toasts are counted by a
  MutationObserver. The harness never treats a corner toast as a decision control.
- Turn redo: at most once per run, after some seats completed turns, at the turn menu of another
  seat in the action phase it clicks "Redo my last turn" on the last finished seat's tab, plays the
  new turn with the normal heuristics (the playthrough pauses while the tab fires the auto-play),
  then randomly clicks Restore original timeline or Keep this timeline. Timeouts: 30 s for the
  rewind, 5 min for the new turn, 150 s for the handoff and for the final state change. Any problem
  (409/400, error text, no handoff) records a finding and tries a restore; if that fails too the run
  ends cleanly with outcome `...STUCK`.
- Secondary pre-planning (`web/e2e/smokePrep.ts`, on in every run): at the start of another seat's
  decision, a seat whose "Prepare your secondary" chip is on screen is offered once per card. With
  probability `TI4_SMOKE_PREP_PROBABILITY` it sets Auto or Review on the player sheet, opens the chip,
  answers the stand-in question with the usual candidate heuristics (seeded RNG, follow 75%), clicks
  Save plan if the mode is still open, and checks the chip says "Prepared". When the seat's real
  secondary opens: Auto must answer by itself with no decision dialog (toast "Auto-played your
  prepared secondary", 20 s limit, nothing is clicked); Review shows the "Prepared" bar, which is
  confirmed (75%) or dismissed with "Choose myself". Follow-up steps (technology, planets, a prepared Warfare production and its first payment) are
  checked the same way for 6 s. Any deviation is a finding and the decision is played normally.
  Counters are in `report.json` `prep`. The prepare chrome is clicked only by this exercise (the
  normal heuristics still exclude it). `TI4_SMOKE_PREP=0` disables it for a manual run.
- Screenshots: up to `TI4_SMOKE_SHOT_CAP` (default 20) per run; the tour and redo contribute at most
  a shot or two inside that cap.

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

**Run names and a killed sweep.** Runs are named `NN-HHMM` under `runs/`. A sweep (also one that
cron restarts after a kill) takes the highest existing `NN` in `runs/` and continues from `NN + 1`;
gaps and directories that do not match `NN-HHMM` are ignored, so ids stay unique within a night.
On SIGTERM, SIGINT or SIGHUP (or any exit) while a run is active, the loop kills the proctor's
whole session (`claude` and its children), runs `run_game.sh stop` (server, vite, browsers), builds
`digest.md` with `digest.py` and appends `## Run NN-HHMM — sweep terminated mid-run (SIGTERM),
proctor failed, mechanical digest only` to `report.md` with the last decision number and the
evidence path. The cleanup runs once, uses timeouts, does nothing when no run is active, and does
not write `sweep.done`, so the next cron tick starts the sweep again.

In the morning: `git checkout <orig_branch>` and merge or cherry-pick from `nightly-fixes-<night>`.
Worktrees under `nightly-reports/<night>/fixer-*` can be removed with `git worktree remove`.

## Commands

    nightly.sh tick                 start whatever is due (cron)
    nightly.sh tick-decide          print what tick would start, change nothing
    nightly.sh loop [night]         the sweep
    nightly.sh fix <round> [night]  an Opus fix round
    nightly.sh summary [night]      the morning summary
    request_fix.sh <reason...>      (proctors) ask for a fix round now (round 1, then round 2)

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
| `NIGHTLY_POK_PROBABILITY` | `25` | percent of runs that play the PoK strategy cards instead of the TE default |
| `NIGHTLY_UI_TOUR_PROBABILITY` | `17` | percent of runs with `TI4_SMOKE_UI_TOUR=1`: open the Faction card and one unit card, check them |
| `NIGHTLY_RECAP_PROBABILITY` | `20` | percent of runs with `TI4_SMOKE_RECAP=1`: one random non-host seat turns the Recap toggle on; recap toasts are counted |
| `NIGHTLY_REDO_PROBABILITY` | `5` | percent of runs with `TI4_SMOKE_REDO=1`: one guarded turn redo round trip (see below) |
| `NIGHTLY_PREP_PROBABILITY` | `0.5` | fraction (0..1) of secondary opportunities a waiting follower plans (`TI4_SMOKE_PREP_PROBABILITY`); `0` switches the exercise off (`TI4_SMOKE_PREP=0`) |
| `NIGHTLY_PREP_AUTO_PROBABILITY` | `0.9` | fraction of plans that use the Auto mode, the rest Review (`TI4_SMOKE_PREP_AUTO_PROBABILITY`) |
| `NIGHTLY_SHOT_CAP` | `20` | cap on screenshots per run (`TI4_SMOKE_SHOT_CAP`; a number or `all` = 20); exercise shots count inside it |
| `NIGHTLY_PRESET_ROTATE_PERCENT` | `20` | percent of preset runs that also rotate the factions (`<preset>+rot`: Jol-Nar and L1Z1X at small tables) |
| `NIGHTLY_NOW`, `NIGHTLY_NOW_FILE` | unset | fake clock for tests |

To switch the Opus rounds off: `NIGHTLY_FIXERS=` in the cron line. Two long Opus sessions per night
are the main cost of this schedule.

## Tests

    tools/nightly-smoke/test_schedule.sh

Runs in a temp directory with stub `claude` binaries and a fake clock: window maths on both sides
of midnight and both daylight-saving changes, the not-before guard, what `tick` starts at each time
(round 1 once at 23:59 or on an early request, a request after round 1 started kept for an early round 2, requests refused after round 2 started or when no round is enabled, the reason reaching the fixer prompt and the report, round 2 only after `sweep.done`, the summary only after round 2, the
previous-night gap), a dry run of a whole night including the between-games merge, a merge that
breaks the build, and a merge conflict. It never starts a real proctor, game or build and never
touches the live checkout. It also SIGTERMs a loop mid-run (fake proctor and game) and checks the
entry, the absence of stray processes, the restart numbering and the `highest-run-number` rule. `test_preset_pick.sh` tests the preset and card set choice.
`coverage.py <reports-dir>...` lists the decision subtypes no run ever offered (and those offered in
fewer than two runs) by diffing the engine's literal subtypes against the runs' `report.json`; `test_coverage.sh` tests it.
