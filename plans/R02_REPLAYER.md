# R02 — Interactive branching game replayer

> **Read first (2026-09-18, operator decisions — four lines below are superseded).**
> 1. **`crates/ti4-engine` is frozen.** No engine edits in any R02 package; this drops the
>    "one narrow engine invariant test" allowed to R02-004, and no `Game::legal_options()` change.
> 2. **Nested choices may be presented inside one engine-step window**, so a manual seat must be
>    pauseable at every decision the engine asks, not only at step boundaries. Measured: 670 settled
>    decisions across 602 engine steps, 8 in one step — so the "one engine step consumes at most one
>    generated choice" regression test below cannot be written as a pass condition; `ask`-within-frame
>    is the invariant the primitives carry instead.
> 3. **Execution is one simulation thread owning `LiveReview`/`Game`**, with the R02 decider parking
>    on a rendezvous with the UI; `Game` never crosses the thread. This satisfies the "unless
>    measurement proves bounded UI batches inadequate" clause. "Before every `Game::step()`, inspect
>    `Game::legal_options()`" is therefore replaced by: the decorator being asked *is* the pause
>    trigger, because `legal_options()` cannot see the open transaction window or any nested ask.
>    Abort-and-retry was rejected: `Game` is not `Clone`, and its open windows,
>    `prepared_turn_seq`, `event_sequence` and `galaxy` live on `Game`, not in `GameState`.
>
> Authority: `plans/EXECUTION_STATE.md` (2026-09-18) and `plans/evidence/R02-001.md`. The text below
> is otherwise the operator's unchanged plan.

## Authority and product contract

R02 is an optional sibling to R01, implemented and run only from
`D:\Projects\ti4-engine-rs`. It does not alter training, TTS, or `ti4-review`.

`cargo run --release -p ti4-replayer` opens a native app using the same checkpoint, map pool, seed,
rotation, temperature, profiles, seating, and engine as R01. It adds:

1. Any physical seat can be toggled `Auto / Manual` at any time.
2. A manual seat pauses before its next choice and the human selects an exact legal option.
3. Any historical frame offers `Play from this frame`, creating a verified live child branch.
4. Source history is never destroyed; all branch timelines remain inspectable.
5. R01 remains the unchanged read-only product with compatible files and controls.

The replayer is omniscient like R01. A local model must implement one package at a time, commit only
that package, record its evidence, and stop for review.

## Locked behavior

### Manual seats and stepping

- Control is keyed by physical `PlayerId`, not faction, and applies to that seat's next unresolved
  choice, including reactions, transactions, combat, payments, production, and agendas.
- A pending panel offers `Choose`, `Let bot choose this decision`, and `Return seat to Auto`.
  Delegation is one-shot and does not change persistent Manual mode.
- Changing another seat never disturbs a pending choice. Switching the waiting seat to Auto lets
  its bot resolve it.
- Accept a click only for the current choice fingerprint and an exact offered option ID. Reject
  stale, changed, duplicate, and non-offered submissions without stepping.
- Keep execution single-threaded. Before every `Game::step()`, inspect `Game::legal_options()`.
  No choice or Auto actor steps normally; a Manual actor without an answer pauses before mutation.
- A submitted option is consumed through normal `Table::ask`/`settle` validation. Never mutate
  `GameState` from the UI.
- Every advance command pauses before manual input. Stop/cancel stays a step-boundary operation and
  does not discard the pending choice.
- Add a regression test that one engine step consumes at most one generated choice.

### Play from frame

- Rewind means deterministic reconstruction, never installing `ReviewFrame.state` into `Game`.
- Play creates a child at the selected source frame; the old future remains intact.
- Rebuild from immutable inputs and repeat the exact engine-step and decision prefix.
- For every prefix choice, invoke the underlying bot once and discard its answer before returning
  the recorded answer. This aligns policy RNG/state for later Auto play.
- Validate actor, prompt, ordered option IDs, typed context, chosen ID, and every reconstructed frame
  fingerprint. Refuse at the first mismatch with a typed diagnostic.
- Copy historical seat modes at the target, then allow changes. Frame zero is branchable. A terminal
  or failed frame is inspectable but Play is disabled unless the rebuilt engine can continue.

### Branches and provenance

- Project-local IDs are monotonic (`branch-0`, `branch-1`, ...); names are presentation only.
- Record parent branch/frame, frames, mode changes, and provenance: `Policy`, `Human`,
  `DelegatedToPolicy`, or `ReplayPrefix`.
- Provenance is an R02 sidecar, not a mandatory R01 `ReviewSession` field.
- Navigation is view-only until Play is pressed. Only one branch is live.

## Architecture

```text
ti4-review (unchanged binary)
  -> current LiveReview, ReviewSession and read-only GUI
ti4-replayer (new crate/binary)
  -> R01 loaders/snapshots/summaries/immutable views
  -> ReplayerLive -> ControlHandles -> ControlledDecider -> LiveReview
  -> BranchRebuilder(setup + prefix + validation)
  -> ReplayerProject(branch tree + provenance)
```

Do not add worker-thread deciders/channels unless measurement proves bounded UI batches inadequate.

## Component contracts

`ControlledDecider` wraps the real policy *under* the trace wrapper so human choices still receive
scores and probabilities. Priority: replay prefix (invoke inner once, discard answer, return the
recorded option), one-shot delegation, Auto policy, then queued Manual ID. Manual without an answer
reaching `choose` is an internal controller error. `Rc<RefCell<_>>` is acceptable on one thread.

`PendingManualChoice` owns branch/frame identity, actor/faction, prompt, ordered options, typed
context, R01 labels/payloads/previews/scores/probabilities, and a fingerprint binding actor, prompt,
ordered `(id, kind)` pairs, and context.

The versioned frame SHA-256 binds engine step, decision count, round, phase, active player/system,
pending timing, canonical state, new events, structured events, and terminal/error state. It excludes
paths, branch names, UI selection, and timestamps.

`ReplayerProjectV1` is separate (suggested `.ti4replay.json.zst`) and contains setup identity, input
hashes plus path hints, branches/frames, mode changes, provenance, and optional UI selection.
Importing R01 creates a read-only source until matching checkpoint/map hashes are supplied.

## Non-regression, bounds, and failures

Forbidden: replacing `ti4-review`; making its timeline branch; mandatory R02 fields in R01 files;
changing R01 setup/RNG/policy/seating/stepping/persistence/HTML/CLI/controls; installing snapshot state;
or touching `ti4-bridge`/TTS. Refactor only behind semantic characterization tests; do not copy the
large R01 GUI.

Inherit R01's 2,000,000 steps/command, 1,000,001 frames, and 1,024 MiB serialization bounds. Add 128
branches/project. Reconstruction is cancellable between bounded batches. Inputs are read-only and
hashes authorize reuse. Divergence, illegal prefix, mismatched input, exhaustion, error, or
cancellation never produces a playable branch. No network/server/background service is needed.

## Package graph

```text
R02-001 primitives -> R02-002 R01 hook -> R02-003 manual controller
 -> R02-004 reconstruction -> R02-005 persistence
R02-002 -> R02-006 shared views
R02-003 + R02-005 + R02-006 -> R02-007 GUI -> R02-008 integration
```

Execute sequentially unless integration confirms disjoint scopes. R02-004 needs frontier review;
others need independent local-model review.

## Atomic work packages

### R02-001 — Choice identity and control primitives

- **Objective:** Create `ti4-replayer` with seat mode, pending choice, fingerprints, replay record,
  and provenance types.
- **Sources/edits:** this plan, engine choice types, R01 option projection; workspace manifest and
  new crate `src/{lib.rs,control.rs}` only.
- **Permission:** P1; no network/process/artifacts/external effects.
- **Invariants/non-goals:** ordered options, physical-seat keys, stale rejection; no GUI, stepping,
  policy setup, or persistence.
- **Tests:** stable/change-sensitive hashes; offered/stale/wrong/double answers; six toggles;
  one-shot delegation; bounds.
- **Checks/evidence:** fmt, lib tests, strict Clippy; `plans/evidence/R02-001.md`.

### R02-002 — Additive controlled-decider injection

- **Objective:** Decorate each real policy before trace wrapping while `LiveReview::new` remains the
  unchanged identity-default path.
- **Depends/edits:** R02-001; R01 `lib.rs`, R02 control, tests/evidence; no GUI.
- **Permission/invariants:** P1; trace outermost; scoring preserved; linear and MLP; no schema change.
- **Tests:** default versus identity has identical manifest/frames/events/decisions/scores/outcome;
  one override traces normally; other seats unchanged.
- **Checks/evidence:** both crate suites, strict Clippy; `plans/evidence/R02-002.md`.

### R02-003 — Manual live controller

- **Objective:** Toggle, pre-step pause, submit/delegate, cancel, and every advance mode.
- **Depends/edits:** R02-002; R02 `src/{lib.rs,live.rs,control.rs}`, tests/evidence.
- **States:** `Ready`, `Running`, `WaitingForHuman`, `Stopped`, `Completed`, `Failed`.
- **Invariants:** inspect before step; no mutation waiting; normal validation; every step creates an
  R01 frame; viewed history never controls live tip.
- **Tests:** top-level/nested choices; immediate toggle; waiting seat Auto; delegate; stale ID;
  Run N/end-round pause; stop/cancel; deterministic command script.
- **Checks/evidence:** affected suites, strict Clippy; `plans/evidence/R02-003.md`.

### R02-004 — Deterministic reconstruction

- **Objective:** Rebuild from immutable inputs/prefix and prove exact target before play.
- **Depends/review:** R02-003; Tier C/frontier.
- **Edits:** R02 `src/{rebuild.rs,fingerprint.rs,control.rs,lib.rs}`; one narrow engine invariant test;
  tests/evidence. Permission P1 with bounded release tests.
- **Invariants:** invoke bot once/prefix choice; replay exact steps including automatic frames;
  validate every choice/frame; never install snapshot state.
- **Tests:** frame zero, automatic, decision, nested, round and terminal targets; manual prefix then
  Auto alignment; tampered actor/prompt/options/context/chosen/state/event; cancellation/bounds;
  repeated rebuild equality.
- **Checks/evidence:** engine/R01/R02 suites, strict Clippy; `plans/evidence/R02-004.md` including
  independent reviewer findings and resolutions.

### R02-005 — Branch tree and persistence

- **Objective:** Non-destructive branches and a separate atomic bounded V1 project format.
- **Depends/edits:** R02-004; R02 `src/{project.rs,persistence.rs,lib.rs}`, tests/evidence.
- **Permission:** P1; task-specific temporary artifacts below 32 MiB.
- **Invariants/non-goals:** monotonic IDs; immutable parents; retain source future; only verified
  branches become live; R01 JSON unchanged; hashes authoritative. No merge/prune/cloud/HTML.
- **Tests:** sibling/nested branches; view without mutation; round trip; interrupted write; malformed,
  oversized, missing-parent/cyclic projects; R01 import verification; 128-branch bound.
- **Checks/evidence:** crate/malformed tests, strict Clippy; `plans/evidence/R02-005.md`.

### R02-006 — Shared immutable reviewer presentation

- **Objective:** Extract board/player/table/objective/action/decision/event/timeline views for both
  apps without changing R01 UX.
- **Depends/edits:** R02-002; `crates/ti4-review/src/{gui.rs,view.rs,lib.rs}`, tests/evidence only.
- **Permission:** P1.
- **Invariants/non-goals:** immutable frame/session inputs; no `LiveReview` access; identical labels,
  colors, geometry, and details. No redesign, R02 controls, HTML refactor, or GUI duplication.
- **Tests:** pure view-model snapshots; app construction; representative before/after data and
  control-availability equality.
- **Checks/evidence:** R01 tests/native build/strict Clippy; `plans/evidence/R02-006.md`.

### R02-007 — Native replayer GUI

- **Objective:** Separate Windows app with six toggles, pending-choice panel, branch tree, and Play.
- **Depends:** R02-003/005/006.
- **Edits:** R02 `src/{main.rs,gui.rs,settings.rs}`, metadata, tests/evidence, ignored `out/replays`
  settings only. Permission P2 for bounded native builds; no network/ports.
- **Layout:** R01 surface plus seat chips, persistent choice panel, branch selector beside timeline,
  Play, rebuild progress/cancel, and live-tip versus viewed-frame marker.
- **Rules:** disable buttons after submit; explain every disabled Play state with a tooltip; never
  overwrite R01 settings.
- **Tests:** GUI reducer states; pause/answer/resume; branch retains future; navigation never branches;
  safe close while rebuilding; separate settings.
- **Checks/evidence:** suites, both release builds, strict Clippy, Windows smoke;
  `plans/evidence/R02-007.md` with observations/screenshots.

### R02-008 — Integration and handoff

- **Objective:** Validate current linear/MLP inputs and freeze operator documentation.
- **Depends/edits:** R02-007; focused fixtures/tests, plan status, execution state, evidence, guide.
- **Permission:** P2 for bounded ignored `out/replays` output, maximum 1,024 MiB; no network/TTS.
- **Campaign:** pre-R02 R01 semantic golden; R01 versus all-Auto R02 equality; manually control all
  seats across action/reaction/combat/transaction/production-payment/agenda; early/middle/round
  branches; branch from a human branch then Auto; save/reopen/reverify; tamper failures; launch both
  binaries independently from D drive.
- **Checks/evidence:** fmt, affected suites, strict Clippy, release builds, deterministic smoke with
  hashes; `plans/evidence/R02-008.md` and unresolved limitations.

## End-to-end acceptance

1. Any physical seat toggles independently at any time.
2. Manual choices pause before mutation and show exactly offered options.
3. Human/delegated/replay/policy provenance is visible.
4. Every advance command pauses, resumes, and cancels correctly.
5. Play from every continuable verified frame creates a child and preserves the old future.
6. Rebuild refuses the first choice/frame divergence.
7. Returning to Auto uses aligned policy state, not a fresh stream.
8. Projects save/reopen atomically; R01 files remain view-only/schema-compatible.
9. R01 setup, stepping, persistence, HTML, GUI, CLI, and deterministic output do not regress.
10. No TTS/bridge edits; both apps build/run from `D:\Projects\ti4-engine-rs`.

## Deferred and definition of done

Deferred: arbitrary state editing, branch merging/pruning, networking, redacted views, interactive
HTML, structural sharing before measurement, rewriting history in place, and TTS integration.

R02 is done when the separate replayer permits manual control of any seat, button selection of every
legal choice, and verified non-destructive continuation from any eligible frame with aligned
automated policies, while the original reviewer remains behaviorally and format compatible.
