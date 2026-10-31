# D2 lane C (`core-d2-c1-status`): the status phase, resume note (2026-10-31)

Branch `core-d2-c1-status`, cut from `core-d2-c2-turn` (`fe3e3b85`). Read `docs/KP_RESUME.md` (lane B: the jobs, `open_finish`, `live_reconcile`),
`docs/D2_LF_RESUME.md` and `docs/D2_LF2_RESUME.md` (the recipe, `NativeSubject`, `playthrough --quick`) first. Never pushed; `nightly-fixes-2026-10-26`
untouched. Packages KS1-KS5 of `docs/todo/D2-islands-composition.md` (section 5).

## What is native

Four arms, no string state: `Arm::Status`, `Arm::Scoring`, `Arm::EventScoring`, `Arm::Tokens` (`driver/live.rs` registry, `driver/windows/status.rs`).

| package | what | where |
|---|---|---|
| KS1 | the scoring windows as HELD windows over one machine `ScoringWin<EVENT>` (`StatusScoring`, `EventScoring`); the payload is the existing `state::Scoring` (the `ScoringBook`); WHO is asked and WHAT they may score is a function of the book and the position (`CoreOffers` over `Seen::{scoreable_publics, scoreable_secrets}`, new, in `seen/objectives.rs`; `query::objectives::feat_of`); a score announces `OBJECTIVE_SCORED`, a public score unlocks heroes (`objectives::award_tail`, no map), a win says `GAME_FINISHED` and closes nothing. `open_event(core, timing, occurrence, one_per_player)` is what the rule windows call to open an event window | `driver/windows/scoring.rs` |
| KS2 | `TurnJob::StatusOpen`: the flag, the 81.1 window in initiative order, `STATUS_SCORING_BEGAN` | `driver/windows/status.rs` |
| KS3 | `TurnJob::StatusBookkeeping` (the CLOSE of the scoring window): the leaders that unlock, 81.2 reveal (an empty deck ends the game), 81.3 draws in initiative order (Neural Motivator, the hooks' bonus, Yssaril's Scheming with its discard question `SchemingDiscard`, the hand limit from rules version 17), 81.4 recall, the Neuraloop, `STATUS_BOOKKEEPING_RESOLVED`, the token window with its report, Cybernetic Enhancements' notes home | `driver/windows/status_bookkeeping.rs` |
| KS4 | the token gain (`rules/token_gain.rs`, lane A's slice) with its display facts for the pools and the redistribution (`live/details.rs`) | `driver/windows/status.rs::advance_tokens` |
| KS5 | `TurnJob::StatusFinish` (the CLOSE of the token window): every seat's redistribution (`Redistribute`, new `Ask::StatusRedistribute`, `Subtype::StatusRedistributeTokens`), typed `STRATEGY_CARDS_WOULD_RETURN` per holder, 81.6-81.8 (technologies, relics, legendary cards, leaders ready; Crown of Emphidia; planets ready; units repair; strategy cards return unless Political Stability keeps them), `COMMAND_TOKENS_GAINED`, Genesis, `STATUS_PHASE_RESOLVED`, typed `STATUS_PHASE_ENDED`; the card moves an effect staged (`CardsStaged` marks) are announced after each typed event, as `Live::emit_typed` does | `driver/windows/status_finish.rs` |

Step boundary (a decision, not an accident): the string closes a window in the step AFTER the last answer (a step with no question). A native
`live_step` is "one decision plus everything that follows until the next decision": the call that applies the last scoring answer also runs the close
(`advance_window` in `status.rs`). A close that asks (Scheming, the hand limit, the Neuraloop, the redistribution) asks through the tape, so a
caller that returns between the questions continues from the position alone (`Pending::nested`, lane B's `TurnFlowWin` carries the three new jobs:
`TurnJob::{StatusOpen, StatusBookkeeping, StatusFinish}`). `NativeSubject` handles the early close (`closes_one_step_early`): the legacy answer step is
compared on the prefix, the legacy close step on everything; the answers a close needs that the legacy answer step did not carry are the next legacy
step's.

New question types (rendered byte-exact, lockstep-tested): `Ask::Strategy(StatusRedistribute)`, `Reason::SchemingDiscard`.

## Gaps (counted, `Unknown::TurnGap(..)`), never silent

`PlotCard` (Plots Within Plots, Firmament dropped), `ScoringSecret` (Strengthen Bonds, Produce en Masse: `secret_progress` is `Unsupported`), `EntropicScar`
(a ship in a scar: Thunder's Edge), `NoteOwner` (Cybernetic Enhancements with two seats of one faction), `AgendaQueue` (an event window closed with
`AgendaQueueAfterEventScoring` on the stack: lane D's `open_next_vote`), `CommanderUnlock` (the 12 factions of `docs/KP_RESUME.md`: the unlock check
at the status bookkeeping and after a public score reuses lane B's `start::unlockable`), `NoRoster` (the unit roster or the printed-name groups of the
table are missing).

## Departures and findings

* Deepgloom Executable (`action_card_draw_requested`, a Thunder's Edge breakthrough): the hook is not run. Scheming is hard-wired for the Yssaril faction (the borrowed mark is the same breakthrough).
* FINDING (string): the Minister of Policy's status-end draw is recorded on `StatusPhaseReport.action_card_draws` and never performed. Parity kept.
* FINDING (string): `finish_status_phase` ignores an error of the redistribution (`let _ =`); an illegal answer is refused by the native window.
* The legacy engine stages events as JSON blobs in `private:#staged:event:*` marks the core does not read: a shadow step that starts with one is counted lent (`an event staged by the legacy engine`); in the product the core stages its own.
* Hand-overs: whoever finishes a rule window that ends an event a secret asks about calls `scoring::open_event` and lets `Arm::EventScoring` serve it; lane D takes `AgendaQueueAfterEventScoring` (remove the gap in `advance_event_scoring`); an EventScoring question asked inside an invasion has no `about_invasion` context yet (M3).
* `KernelEnv::mecatol_area` answers `None` for a position without the map slice (the string's `galaxy: None`).

## Checks (cheap), `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`, private target dir `/root/deploytmp/core-c` (deleted at the end)

* `cargo test -p ti4-engine-core --lib`: 466 passed. `-p ti4-engine-core-factions`, `-p ti4-engine-core-tests` (all binaries, incl. the new `status_phase`: 31 tests), `-p ti4-engine-core-bridge` (lib + integration): all ok.
* `cargo test -p ti4-engine --lib` (default areas): 4961 passed, 1 failed on the first run (the lane A vertical-slice test of `Arm::Tokens`, which the native close changed: fixed, the lib `live::` tests pass); not rerun whole after the fix.
* Lockstep against the string (`ti4-engine/src/status_live_tests.rs`, 13 scenarios): plain phase, Scheming draw and discard, hand limit after the draw, empty objective deck, Political Stability, an offered and scored objective, the third objective unlocking a hero, the Neuraloop, Genesis, Naalu's zero initiative, three event-window cases. Questions as rendered (prompt, options, display facts, context), events, position, dice streams and the events left staged are equal.
* `NativeSubject` shadow behind a legacy game (`native_subject::tests`): the four arms through round one of a lobby table, and every lobby faction in six tables with a counted bail a failure: zero differences except the counted `CommanderUnlock` gap (5 occurrences).
* Clone mid-window: `status_phase` (scoring, the close suspended on Scheming's question, the redistribution).
* `tools/engine_core/check_layers.py core`: 460 files, 0 violations. `lint_core.sh`: 462 files, 0 hits.
* Mutation (`mutants.sh`, Haiku runners on committed snapshots in a scratch worktree, single files; core tests `status_phase` and the engine lockstep as two test sets, union): first runs: core tests alone scoring 12/37, status 9/11, bookkeeping 31/46, finish 24/42, seen offers 7/26; engine lockstep alone 16/37, 9/11, 37/46, 35/42, 11/26. After the survivor tests (core only, snapshot `9c8b386f`): scoring 23/34, bookkeeping 31/46, finish 38/42, seen 21/26. Survivors left: equivalent ones carry `// mutants:skip` with a reason (rebuilt flags, labels, dead defaults, dry-tape returns the driver rolls back); the rest is only visible through the string lockstep or has no cheap position: `scoring.rs` `answer` (`secret` flag of the outcome for a leaked secret, the mapless `unlockable(.., true)` for Maban), `status_bookkeeping.rs` (`unlockable(.., false)` at the bookkeeping), `status_finish.rs` `genesis` (killed by the lockstep), `seen/objectives.rs` (`Ok(None) => false` branches). `query::objectives::feat_of` is a `const fn` (the tool skips it): covered by the event-window tests.
* `playthrough_quick.sh`: see the report; the bin still plays the string `Game` under `all=dual`, so island C's counted bails fall only when the wrapper (D2-8) calls the native arms.

Not run (standing rule): golden/wide dual sweeps, benches, soak, mutation ratchet, census, workspace clippy, the armed dual, the Z-gate.
