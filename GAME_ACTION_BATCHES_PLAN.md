# Readable game history and atomic decision batches

## Goals

1. **A useful game log.** Show what players did at a human scale: “P1 activated #22 and moved two fighters and a carrier from #16,” with individual choices available on expansion. Fifty engine decisions should not produce fifty equally prominent log rows.
2. **Fast staged actions.** A workflow containing 50 choices should not require 50 browser-to-server round trips. At 500 ms round-trip latency, that is roughly 25 seconds before accounting for processing.
3. **All-or-nothing confirmation.** When a player confirms a staged movement, payment, or similar basket, either every requested choice is legal and committed or none of them is. A mismatch should give developers a precise, actionable error.
4. **Predictable undo and redo.** Offer both individual-decision history controls and meaningful group controls (“Undo batch” and “Undo action”). A tactical action may include several batches and follow-up decisions; these are different boundaries.
5. **Keep the engine authoritative.** Each committed choice must still be an option actually offered by the engine. Preserve deterministic replay, crash recovery, player visibility, and the ability to play one choice at a time.

**Scope of an atomic batch:** one player’s staged UI workflow, such as moving/loading a fleet, spending a basket of resources, or exhausting planets for a vote. Do not wait for another human’s combat or reaction decision inside a batch. The wider tactical action remains a grouping for the log and for action-level undo, not a multi-player atomic transaction.

## Current state

- The engine asks one `Choice` at a time and records each accepted answer as a `DecisionRecord` (player, prompt, option ID, offered IDs, optional `DecisionContext`). The context describes the decision, not an explicit parent action or UI batch. See `crates/ti4-engine/src/choice.rs` and `decision_context.rs`.
- The server persists an initial state and accepted decisions. Undo replays a prefix from the beginning; it does not keep a complete engine checkpoint per choice. After a rewind, the active and redo branches live in atomically replaced `history.json`. See `docs/code/WEB_GAME_HISTORY.md`, `crates/ti4-server/src/storage.rs`, and `session/registry.rs`.
- `undo_pipeline` already rewinds from the latest action-phase decision through subsequent choices. It finds the boundary by searching for the prompt `"action phase"` and comparing context phase/round. This is useful but not an explicit action identity and need not match what a player staged in a drawer.
- Server events currently include a bare `DecisionResolved`, phase changes, initialization, and game over. The web log therefore says “Decision resolved”; the stored decision contains no chosen-option label or payload for presenting a fleet or payment summary. Events carry a `decision_count`, which links them to undo targets. See `protocol/server.rs`, `session/worker.rs`, and `web/src/components/EventLog.tsx`.
- Web workflows stage choices in `usePipelineRunner.ts`, `TacticalMovementOverlay.tsx`, `CargoLoadingTray.tsx`, `PaymentDrawer.tsx`, and `AgendaBallotModal.tsx`; production has another queue in `GameShell.tsx`. They submit one option ID, await acceptance **and a newer authoritative state/nonce** in `web/src/protocol/client.ts`, and then select the next option. Already accepted steps stay committed on an interruption. A browser refresh loses the UI queue.
- A `GameState` snapshot by itself is **not** an engine transaction snapshot: `Game` also owns open tactical/combat/payment windows, RNG streams, a decision table, and other in-flight state (`crates/ti4-engine/src/game.rs`). Publishing choices and then calling Undo would expose intermediate events and could leave partially persisted history on a crash.

## Target architecture

Keep **three layers** distinct:

| Layer | Purpose | Owner |
|---|---|---|
| Decision | One accepted, legal engine choice; replay and single-choice undo unit | Engine/server |
| Batch | One staged UI confirmation, committed atomically; may contain many decisions | Server validates client-declared intent |
| Action | A game-rules action and its follow-ups, possibly spanning batches and other players | Engine/server, independently of the UI |

The server records authoritative decision facts and stable batch/action associations. The browser **folds visible facts** into a readable log: one collapsed row for a movement or payment batch, expandable details for auditing, and an action-level grouping where useful. Presentation can change without changing replay identity. Choices submitted manually, by bots, or by older clients still produce understandable single-decision entries.

### Data flow: successful batch

```text
UI stages a fleet/payment/vote basket (nothing committed)
  -> client sends plan + current nonce/version + request ID
  -> server authenticates actor and reserves this game's decision boundary
  -> private engine run replays the current history, then tries each requested
     step against the newly offered legal options (no writes or broadcasts)
  -> confirm all steps, replay prefix, and next stopping boundary
  -> durably commit decisions, events, group metadata as one timeline change
  -> publish replacement snapshot; all viewers see their own redacted view
  -> UI clears draft, shows the authoritative result and condensed log row
```

If an expected option disappears, the actor/subtype/target changes unexpectedly, the engine errors, another human must answer before the plan completes, or the live version changes, **discard the private run**. Return a structured failure including step number, expected intent, and safe diagnostic information. The decision cursor, redo future, log, and game state remain as they were. If another human is asked *after* the last step, commit the finished batch and offer that human their decision normally.

### Data flow: undo and reconnect

```text
Host selects Undo batch / Undo action / Undo one / restore to event
  -> server resolves the selected group's first decision cursor
  -> existing deterministic replay validates the desired prefix
  -> active decisions, events, and groups split from their redo future
  -> replacement session starts; clients replace their snapshots and log
```

Redo one decision can remain available. Add group-level redo so “Undo batch” is not followed by 50 manual Redo clicks. The old `undo_pipeline` endpoint should eventually use an explicit action boundary; migrate the UI label to “Undo action” without conflating it with “Undo batch.” Historical decisions without metadata retain the current fallback behavior.

## Implementation details

### 1. Specify the batch request and result

Add a bounded, authenticated batch command, preferably an HTTP request analogous to the existing host history endpoint; this avoids the WebSocket's current 4 KiB incoming-message limit and gives an ordinary request/response for a potentially longer computation. It needs an actor's player-session credential, `expected_version`, current `nonce`, a client-generated idempotency `request_id`, workflow kind/target, and ordered **data** describing intended selections. Never transmit JavaScript predicates or claim that future option IDs are already legal.

Illustrative request shape (not a promise that these exact field names exist yet):

```json
{
  "request_id": "unique-confirmation-id",
  "expected_version": 41,
  "nonce": "current-choice-nonce",
  "plan": {
    "kind": "tactical_movement",
    "destination": "22",
    "steps": [
      { "kind": "move", "origin": "16", "unit": "fighter", "count": 2 },
      { "kind": "move", "origin": "16", "unit": "carrier", "count": 1 },
      { "kind": "finish_movement" }
    ]
  }
}
```

For example, a movement plan might request two fighters from #16 to the active system #22, a carrier from #16, optional cargo loads, and an explicitly requested `done_moving`. Each requested unit/count is expanded server-side; at every step, match its origin, unit, damage/source and other necessary attributes to **exactly one** newly offered engine option. Reject missing or ambiguous matches. Check actor, subtype and destination against the offered `DecisionContext`. Define equally explicit schemas for payment and votes later. Repeated trade goods may be encoded as a count, but must be checked anew for every spend. A submitted plan is an instruction to *select legal options*, never an instruction to mutate state directly.

Use a bounded number of expanded steps, bounded request size, a simulation time/step budget, and an explicit failure for an interrupted workflow. Return either `{ request_id, batch_id, start_cursor, end_cursor, snapshot }` or a typed failure such as `{ failed_step, reason, expected, offered_summary }`, redacted to the actor's permitted view. Persist successful request IDs with batch metadata so a timeout/retry cannot execute the same batch twice; a repeated ID returns the original outcome (or its current timeline status), and a stale nonce/version from another request returns a conflict. Document behavior if the original batch has subsequently been undone.

### 2. Build a genuinely private speculative executor

Start with a **temporary `Game` reconstructed from the saved initial state, galaxy, and accepted decision prefix**, using the same deterministic seed/content as recovery. Do not start a normal `GameSession` worker for speculation: it writes logs, publishes events, and can wait indefinitely for remote humans. Supply a decider that replays the prefix, then consumes the plan one offered `Choice` at a time; stop at the next pending human choice after the final step, without accepting an unplanned decision. Inspect automatic engine steps so a success is not reported halfway through a transition. Verify prefix records/hashes and final planned records before commit. If replay itself diverges, fail closed rather than “fixing” history.

Do **not** try to clone only `GameState`, and do not implement rollback by sending speculative choices through `GameSession::submit_choice`. The `Game` struct is not currently cloneable, and replay from the initial state is the existing correctness baseline. Profile replay time on long games and 50-step plans; if too slow, later add a complete in-memory engine fork/checkpoint API, including windows and RNG, behind the same executor interface.

### 3. Serialize, persist, and publish exactly once

Introduce a **per-game** batch gate so competing submissions, undo, bot advancement, and another batch cannot race the expected cursor. Do not hold the registry-wide mutex throughout replay (it would stall other games). Check the live session identity, pending nonce/version, actor, and decision cursor before starting and again at commit; either reserve that game's worker boundary or abort on any change. Existing `submit_player_choice`, history changes, and WebSocket submissions must honor this gate. Define a finite timeout rather than allowing a blocked run to hold the game indefinitely.

Build the complete replacement active timeline **in memory**: all new `DecisionRecord`s, matching decision events, any phase/finish events, batch/action metadata, redo split, monotonically allocated event IDs, and new version/generation. Preserve existing event `decision_count` semantics (one `DecisionResolved` per accepted choice) and the original prefix. Persist it in **one atomic history write**, including on games that have never been rewound: recovery already prefers `history.json` to the old append-only logs. Ensure recovery validation checks the new groups and that old logs/snapshots cannot supersede the committed timeline. A failed save publishes nothing; a crash before the atomic write recovers the old history and a crash after it recovers the entire batch.

Only then install a worker at the committed cursor. Reuse the history-replacement approach where practical, but specify failure recovery around stopping/starting the old worker: a failed persistence or replacement must restore the old authoritative session rather than strand the game. Give viewers a replacement snapshot and prompt reconnect like history changes; avoid the current two-second reconnect delay for this path. Until the commit, even spectators and HTTP snapshots must see only the old timeline. The actor's result and all viewers' log/snapshot must describe the **same committed history generation and cursor** (the new worker may advance its version while reaching its next pending choice); do not send fifty intermediate broadcasts to bounded subscriber queues.

### 4. Record useful facts and stable boundaries

At resolution time, use the actual offered `ChoiceOption` (including its payload) and context to derive small **typed event facts**, rather than treating an opaque option ID or client-provided prose as the audit record. Examples: actor, `movement_step`, origin #16, destination #22, fighter; `pay_resources`, exhaust Jord for four; `load_cargo`, infantry from a planet. Encode public and seat-private details separately; the existing public `DecisionResolved` must not expose a secret objective, card, private selection, or hidden intermediate effect. Use the projection/visibility rules for both live events and reconnect snapshots.

Prefer optional, versioned detail/group fields on `GameEvent` while retaining legacy `decision_resolved` JSON, so existing event counts and historical files remain readable. Persist server-assigned `batch_id` and batch start cursor for every committed batch decision. Introduce a durable action boundary driven by the engine's action lifecycle (including free actions and follow-ups), rather than inferring it solely from `prompt == "action phase"`; an action can include decisions by more than one player. Keep action and batch IDs separate. For manually submitted choices, continue the action association when known, but do not invent a batch. Include metadata in the redo branch and remove it appropriately when a new choice forks history.

### 5. Render readable history and expose the right undo targets

Update `web/src/protocol/types.ts`, `decode.ts`, `client.ts`, and `EventLog.tsx` to accept structured events and replacement snapshots. Fold **only visible** contiguous facts with the same batch/action ID into concise headings: e.g. “P1 moved 2 fighters and a carrier from #16 to #22,” with details expandable. Show an in-progress action appropriately if it has no final summary yet; distinguish “declined/done” from meaningful changes. The UI may group a manually played sequence when the server supplies an action ID, but must not depend on a local React queue surviving a refresh.

Attach “Undo to here” to a real server decision cursor, not a synthetic collapsed-row position. Offer batch undo at `start_cursor`, action undo at its start, and single-decision undo separately. Check private-event gaps, events with the same cursor, old history lacking detail, 500-entry client log truncation, and redo/restore so a collapsed row never points at the wrong boundary. On history generation changes, clear staged plans and replace the log from the snapshot as the client already does.

### 6. Migrate workflows in manageable slices

1. **Foundation:** speculative executor, per-game gate, atomic history commit, idempotent result, one typed batch kind; retain the current `submit_choice` path for individual choices. Test crash and race behavior before wiring a UI.
2. **Tactical movement/cargo pilot:** translate `TacticalMovementOverlay`'s staged `ExecutionPlan` into one plan request. Cover multiple origins and ship/cargo loads; make completion dependent on the engine's offered finish choice. Compare successful outcomes to the existing one-choice-at-a-time path.
3. **Readable log and history controls:** structured server facts, group IDs, web folding/expansion, explicit batch and action undo/redo. Keep older files and non-batched clients functional.
4. **Other baskets:** payment, agenda-planet votes, and production (which currently has a separate queue). Give each workflow an explicit schema and stopping rule; do not let a production batch silently answer placement or another player's decision. Generic decisions stay single-choice until their semantics are specified.
5. **Latency improvements:** measure private replay, atomic-write and replacement-session time against a 50-choice high-latency baseline. Optionally replace the WebSocket bridge's 50 ms polling with wake-on-update delivery for normal single-choice play; that optimization alone will not remove per-choice network latency.

## Verification and done criteria

- **Atomicity:** a late mismatch (including step 50), duplicate request, engine error, storage failure, disconnect, and server restart each leave either the entire batch committed once or none of it. No partial event, leaked speculative state, lost redo future, or ghost action appears to any viewer.
- **Legality and isolation:** each planned step is checked against its fresh offer; ambiguous IDs or changed actor/subtype/target abort. Concurrent human choice, bot decision, undo, credential rotation, and a second batch cannot commit across the reserved boundary. Another player's required response stops an unfinished plan.
- **Recovery and history:** compare final state, decision hashes, event cursors, and RNG-dependent outcomes with an equivalent sequential run. Restart from `history.json`, undo/redo an entire batch and action, then fork with a different decision. Validate old histories without group fields.
- **Visibility and UI:** actor, opponent, and spectator snapshots show only permitted details; grouped rows survive reconnect and show the correct counts/undo target. A failed confirmation keeps the staged draft and explains the exact mismatch without claiming anything was committed.
- **Performance:** with simulated 500 ms round-trip latency, a 50-decision batch requires one client request/response, not 50. Set an explicit latency budget after measuring replay and persistence, and fail with a bounded error rather than hanging.

Useful starting points: `crates/ti4-server/src/session/{registry,worker,replay}.rs`, `storage.rs`, `protocol/{client,server}.rs`, `ws/mod.rs`, `web/src/protocol/{client,types,decode}.ts`, `web/src/hooks/usePipelineRunner.ts`, `web/src/components/{TacticalMovementOverlay,EventLog,GameShell}.tsx`, and `crates/ti4-server/tests/history.rs`.
