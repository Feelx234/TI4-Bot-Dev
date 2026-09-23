# Web UI E2E Invariant Test Improvement Plan

## Objective and evidence

Prevent a pending decision from appearing actionable while leaving the active player unable to progress. Start with the reported empty tactical-movement state, then extend coverage to representative decision workflows. Distinguish a dispatched click, an accepted action, and an authoritative engine transition.

The current `web/e2e/multiplayer_invariants.spec.ts` stops after the first two of three humans draft strategy cards. It does not enter the Action phase. Its DOM, connection, privacy, and VP checks cannot detect a stalled decision.

**Diagnosis before hardening:** `crates/ti4-engine/src/tactical.rs` offers `done_moving` with decline kind, and `TacticalMovementOverlay.tsx` already resolves either the model's decline option or an option matching that ID/kind. The overlay does have a silent return if no such option resolves, but that is not yet a proven explanation of the reported live stall. Reproduce the observed state and record the pending choice, viewer seat, rendered workflow, clicked control, outbound submission, rejection/error (if any), and next authoritative server state. Do not infer the root cause from a hypothetical missing option.

## Acceptance invariants

1. **Reachable action:** For an active seat waiting for a decision, a legal choice exposes a visible, enabled, keyboard-accessible path. Minimized workflows expose a way to resume. Other seats must not see or submit the private choice.
2. **Empty-state completion:** If a workflow has no selectable units/resources and the engine offers an explicit finish/decline option, that option is accessible and actionable. If it is absent, display an accessible error instead of silently returning. Never interpret an arbitrary sole option as a finish option.
3. **Authoritative progress:** After a choice is submitted, assert the expected newer server version and subsequent decision, stage, phase, or game state. A sent WebSocket frame, `action_accepted`, closed modal, or changed nonce alone is insufficient. A rejection must be reported, not counted as success.
4. **Failure recovery:** Rejection, disconnect, stale nonce, or an interrupted `usePipelineRunner` queue must leave the user with a clear error/recovery path and restore usable controls once the state permits it. Avoid blanket assertions that *every* enabled control must send a network request: selectors and steppers legitimately change local UI state first.

Use bounded, condition-based Playwright assertions; record the timeout and failure diagnostics. Do not use retries as evidence of correctness.

## Priority 1: Reproduce and protect empty movement (INV-01)

- Add a focused test beside `web/src/components/TacticalMovementOverlay.test.tsx` with a server-shaped `movement_step` containing only `{ id: 'done_moving', kind: 'decline', label: 'finish movement' }`. Derive the renderer model as in production. Assert the empty label, enabled Done Moving control, and exact `onSubmit('done_moving')` call. Test the missing-explicit-finish case separately: no arbitrary submission, visible `role="alert"`.
- Reproduce the reported failure through a real browser, server, and engine. Use a fixed setup/seed if reliably reachable; otherwise introduce one narrowly scoped authenticated test fixture/checkpoint only after establishing the minimal session data required. Assert a newer authoritative state *after* the movement step, not just disappearance of the tray. Check rejection and error paths. Capture the exact reproduction in test evidence before choosing a code fix.
- Keep UI changes limited to the verified cause. A failed or unresolvable submission should show an accessible error; do not add `choice.options.length === 1` as a fallback. Extend the existing movement component test that currently covers a finish button with move options present.

## Priority 2: Three-seat live integration smoke (INV-02)

Extend `web/e2e/multiplayer_invariants.spec.ts` and `lobbyHelpers.ts` to open the third human's page, finish the draft, and assert that all three clients observe the Action phase. Exercise one deterministic first Action-phase choice if feasible. Keep this test short; do not chain movement-with-ships, empty movement, combat, and later phases into a single brittle script. Use isolated fixtures for hard-to-reach decisions.

## Priority 3: Auditable component decision matrix (INV-03)

Start with a small set of real protocol-shaped fixtures: empty/normal movement, one payment, one production, a reaction pass, and a generic constrained choice. Assert classification, visibility and accessibility, and dispatch of an *offered* option ID. For a pipeline of multiple choices, advance the fixture nonce and choice after each acknowledged submission; a resolved `onSubmit` mock alone does not prove the pipeline completes.

Maintain a coverage ledger mapping each fixture to its engine subtype, workflow, source (captured engine output or explicitly synthetic), expected option IDs, and boundary condition. The 16 `ChoiceWorkflowKind` members are UI categories, **not** an exhaustive list of engine subtypes. Expand the matrix as uncovered production subtypes are identified; require an explicit coverage decision when adding or changing a subtype. Do not claim 100% coverage from 16 representative examples.

Check assumptions against `web/src/presentation/choiceModel.ts` before writing expectations: `tactical_production` currently matches the earlier payment classification branch; `produce_unit`/`place_unit` reach the production branch. A fixture expecting `tactical_production` to render as production must first establish the intended actual server output and, if necessary, fix classification in a separate focused change. Preserve fixture provenance and inspect semantic diffs when regenerating versioned fixtures using `crates/ti4-server/examples/generate_protocol_fixtures.rs`; generation from Rust types alone does not prove that a choice is reachable in a game.

## Priority 4: Selective mocked-browser protocol tests (INV-04)

Use `web/e2e/decision_workflows_mocked.spec.ts` for cases that component tests cannot establish: browser visibility, focus, overlays, board highlights, and outbound WebSocket serialization. Register `page.routeWebSocket` **before navigation** and match the actual socket URL. The client fetches an HTTP initial snapshot before/alongside opening its socket, sends `subscribe`, and expects versioned messages with `game_id`, viewer, view/state, turn status, and pending-choice envelope. Mock the HTTP snapshot as well as the socket handshake, then send valid `initial_snapshot`/`state_update`/`pending_choice` and `action_accepted` or `action_rejected` messages with coherent game versions and nonces. `decision_resolved` is an event kind, not a standalone server message. Do not mix every fixture in one asynchronous route handler; isolate cases so failures identify the offending workflow.

Mocked browser tests establish frontend behavior against a simulated backend, not engine legality or progression. Verify outbound `option_id`, `nonce`, `expected_version`, and rejection feedback. Reserve authoritative transition claims for the real-engine test.

## Priority 5: Targeted real-engine checkpoints, if needed (INV-05/06)

Start with one or two high-value decisions that cannot be reached cheaply through a deterministic live test (empty movement first, then a representative combat or agenda state). Before building a harvester or HTTP loader, determine whether existing `SessionConfig`, `GameSession`, recovery tests, and a test-only setup can instantiate a faithful pending decision. `GameState` alone may not preserve the decision window, galaxy layout/map tiles, seat controllers, credentials, RNG, and session history. Verify that the loaded pending choice and legal IDs match the captured state before using it as evidence.

If an endpoint is necessary, bind it to a separate loopback-only test server/build, accept **only allowlisted named fixtures** (never arbitrary file paths or caller-provided credentials/state), keep fixtures versioned and size-bounded, and ensure the production binary cannot expose the endpoint. Generate per-run synthetic test sessions rather than storing reusable credentials. Validate load and replay/transition semantics in a Rust integration test before adding `web/e2e/checkpoint_gameplay.spec.ts`. Add further checkpoints based on measured coverage gaps, not a predetermined corpus size or timing promise.

## Separate follow-ups

- **Board inspector option ID:** `SystemInspector.onSelectAction` passes an `optionId`, but `Board.tsx` discards it and calls `onSelectTarget(systemId)`; `BoardProps` has no option-ID submission callback. Specify an explicit `onSelectAction(optionId)` contract through the parent, test inspector actions with target-bearing options, then fix that path separately. A targetless `done_moving` is not an inspector action.
- **Zero-minimum multi-select:** First capture an engine choice with `min_selection: 0` and determine its offered completion option. `usePipelineRunner([])` currently stops without submission, while the wire protocol submits an option ID rather than an empty selection. Only then test and implement the legal completion path; do not invent an empty-selection wire message.
- **Dev decision gallery (INV-07, optional):** Add a dev-only preview route if designers need it after fixtures exist. It is a visual QA aid, not an acceptance gate for liveness.

## Execution and verification order

| Order | Deliverable | Gate |
|---|---|---|
| INV-01 | Actual empty-movement reproduction, component regression, minimal verified fix, real browser/server/engine regression | Explicit offered `done_moving` is submitted; next authoritative state arrives; missing option/errors cannot be silent. |
| INV-02 | Three-seat draft and first Action-phase smoke | Third seat drafts; clients enter Action; chosen first action advances authoritatively. |
| INV-03 | Small, sourced component fixture matrix and subtype ledger | Each included case checks classification and actionable legal ID; omissions documented. |
| INV-04 | Selected browser protocol mocks | Valid HTTP+WebSocket lifecycle, outbound message, accessibility and rejection path tested. |
| INV-05/06 | Selected checkpoints and real-engine E2E, only where justified | Loaded decision is authentic and its submission yields a verified engine transition. |
| INV-07 | Optional dev gallery | Preview works in development without weakening the production/test boundary. |

Run focused Vitest and Playwright specs first, then the affected web suite and any Rust server/engine tests introduced by fixture or loader work. Record exact commands, failures, fixture provenance, actual elapsed time, and results. As a regression-strength check, deliberately break the *tested* empty-movement submission in a temporary local edit and confirm its focused component and real-engine browser tests fail, then restore it. Do not require unrelated suites to fail the same mutation or set unmeasured sub-second/zero-flake guarantees.
