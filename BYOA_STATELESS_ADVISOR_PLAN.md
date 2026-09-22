# Stateless "Bring Your Own Advisor" (BYOA) Architecture & Redaction Unification Plan

## 1. Overview & Architectural Goals

This plan specifies the implementation of the **stateless "Bring Your Own Advisor" (BYOA)** architecture and the unification of server-side state redaction.

### Core Principles
1. **Zero ML / PyTorch in `ti4-server`**:
   The game server remains a pure-Rust, lightweight router. It handles game authority, lobbies, session persistence, and WebSocket subscriptions, compiling quickly with zero C++ or `libtorch` dependencies.
2. **Symmetrical, Headless Clients**:
   Bots and humans connect using the exact same WebSocket wire protocol and unguessable `seat_token` capability model. The server makes no distinction between a human in React, a headless Python script, or an external neural bot daemon.
3. **Single Source of Truth for Redaction (Eliminating Drift Liability)**:
   `ti4-server`'s wire projections (`project_player_view`) currently perform redundant custom hand-scrubbing. We refactor `ti4-server` to build its wire projections on top of the engine's authoritative `ti4_model::view::view_for`. Non-server crates remain untouched (facedown draw decks remain unscrubbed for now, as requested).
4. **Stateless Snapshot Evaluation**:
   The engine's `GameState` is fully serializable (`serde`). The advisor service holds no session state, stores no database, and requires no replay history. Given a single `(GameState, PlayerId, Choice)` snapshot, it extracts features and returns policy probabilities and critic values in <1 ms.
5. **AI Advisor for Human Players**:
   Human players can visualize policy recommendations (probability badges per option) and the critic evaluation bar ($V(s)$ win confidence) in the web UI by querying the stateless advisor.

---

## 2. Architecture & Data Flow

```
                      ┌─────────────────────────────────┐
                      │           ti4-server            │
                      │  (Pure Rust, zero ML runtime)   │
                      └────────┬───────────────┬────────┘
          WebSocket (seat_token)│               │WebSocket (seat_token)
                               ▼               ▼
      ┌──────────────────────────────┐   ┌──────────────────────────────┐
      │     Human Browser Client     │   │      Headless Bot Agent      │
      │  - React 19 + SVG UI         │   │  - Connects as seated player │
      │  - Receives Choice & State   │   │  - Receives Choice & State   │
      │  - Displays Advisor Badges   │   │  - Submits choices via WS    │
      └──────────────┬───────────────┘   └──────────────┬───────────────┘
                     │ HTTP POST                        │ HTTP POST / In-Process
                     ▼                                  ▼
      ┌─────────────────────────────────────────────────────────────────┐
      │                    Stateless Advisor Service                    │
      │                  (`ti4-advisor` / standalone)                   │
      │  - Links `ti4-mlp` & `ti4-policy` with `libtorch`               │
      │  - Evaluates `(redacted GameState, PlayerId, Choice)`           │
      │  - Returns `{ head, value: V(s), options: [{ id, p, logit }] }` │
      └─────────────────────────────────────────────────────────────────┘
```

---

## 3. Work Packages

```
[BYOA-01: Redaction Unification in ti4-server]
                      │
                      ▼
[BYOA-02: Expose Redacted GameState Endpoint & Protocol DTO]
                      │
         ┌────────────┴────────────┐
         ▼                         ▼
[BYOA-03: Stateless Advisor]  [BYOA-04: Headless Bot Agent]
         │
         ▼
[BYOA-05: Web Client AI Advisor UI]
```

---

### Work Package BYOA-01: Redaction Unification in `ti4-server`

#### Objective
Refactor [`crates/ti4-server/src/projection.rs`](file:///home/zibert/github/TI4-Bot-Dev/crates/ti4-server/src/projection.rs) so that client views are derived strictly from `ti4_model::view::view_for`, eliminating the dual-maintenance drift risk where `ti4-server` independently implements private hand scrubbing.

#### Technical Details
1. In `projection.rs`, replace manual filtering of `held_action_cards` and `held_secret_objectives` in `project_player_view`:
   - Run the authoritative `ti4_model::view::view_for(state, viewer_seat)` first.
   - For a spectator viewer, redact all players using `ti4_model::view::redact_player`.
   - Populate `PlayerView` directly from the resulting redacted `Player` struct:
     - Cards matching the engine's `ti4_model::view::HIDDEN` marker (`"?"`) are excluded from `held_action_cards` and `held_secret_objectives`.
     - Hand counts (`action_cards_count`, `secret_objectives_count`) reflect the total hand size without disclosing private card identities.
2. In `crates/ti4-server/tests/projection_redaction.rs`:
   - Maintain 100% passing tests proving opponent/spectator secret objectives and action cards remain strictly redacted.
3. **Zero changes to `ti4-model` or `ti4-engine`**.

---

### Work Package BYOA-02: Expose Redacted `GameState` in Server Protocol

#### Objective
Provide an authenticated, direct way for connected seats (whether human browser or headless bot) to obtain the engine-level redacted `GameState` corresponding to their current position.

#### Technical Details
1. **HTTP Endpoint (`GET /api/games/:game_id/state`)**:
   - In [`crates/ti4-server/src/http/games.rs`](file:///home/zibert/github/TI4-Bot-Dev/crates/ti4-server/src/http/games.rs):
   - Authenticated via `x-ti4-seat-token` header (same as `/api/games/:game_id/snapshot`).
   - If token is valid for seat $P$, applies `ti4_model::view::view_for(&state, &P)` and returns the redacted `GameState` as JSON.
   - If no token or invalid token, returns a spectator view (all hands redacted via `redact_player`).
2. **Optional Snapshot Field / PendingChoice Attachment**:
   - Add an optional `raw_state: Option<GameState>` field to `InitialSnapshotMsg` (or `PendingChoiceMsg`) behind a query param or capability header `x-ti4-include-state: true`.
   - Allows clients to receive the exact position state synchronously alongside choices without a second round-trip.

---

### Work Package BYOA-03: Stateless Advisor Service (`ti4-advisor`)

#### Objective
Build a standalone, stateless HTTP/IPC service crate `crates/ti4-advisor` that links `ti4-mlp` and `ti4-policy` to evaluate any position on demand.

#### Technical Details
1. **Crate Setup (`crates/ti4-advisor`)**:
   - Independent workspace crate depending on `ti4-mlp`, `ti4-policy`, `ti4-model`, `ti4-content`, `axum`, and `serde`.
   - Leaves `ti4-server` untouched by `libtorch`.
2. **API Contract**:
   - `POST /evaluate`:
     ```json
     {
       "state": { /* Redacted GameState JSON */ },
       "player": "p1",
       "choice": { /* Pending Choice JSON */ },
       "temperature": 0.25
     }
     ```
   - **Evaluation Pipeline**:
     1. Deserializes `(state, player, choice)`.
     2. Constructs in-memory `Observed::new(&state, content, sources, galaxy)`.
     3. Calls `ti4_policy::projection::mlp_choice_features(&observed, &choice, &player, ...)`.
     4. Runs forward pass through the preloaded `Actor` and extracts logits and softmax probabilities.
     5. Runs value pass on `CriticInput` to compute $V(s)$.
   - Returns:
     ```json
     {
       "head": "tactical_movement",
       "value": 1.425,
       "options": [
         { "option_id": "move|24|0", "probability": 0.682, "logit": 2.45 },
         { "option_id": "decline",   "probability": 0.318, "logit": 1.12 }
       ]
     }
     ```
3. **Execution Latency**:
   - Single forward pass executes in **~0.87 ms** on CPU.
4. **Startup & Configuration**:
   - CLI flags: `--checkpoint <DIR>` (default: `examples/reviewer/checkpoint-473312`), `--port <PORT>` (default: `8081`).

---

### Work Package BYOA-04: Headless Symmetrical Bot Agent (`ti4-bot-agent`)

#### Objective
Implement a lightweight, standalone bot runner that connects to `ti4-server` as a normal player over WebSocket, proving end-to-end symmetry between humans and bots.

#### Technical Details
1. **Standalone CLI Binary**:
   - Runs as an independent process:
     ```bash
     cargo run -p ti4-advisor --bin bot_agent -- \
       --server ws://localhost:8080 \
       --game game_123 \
       --seat p2 \
       --token <seat_token> \
       --advisor http://localhost:8081
     ```
2. **Event Loop**:
   - Connects to `ws://localhost:8080/ws/games/:game_id`.
   - Sends `ClientMessage::Subscribe { game_id, seat_token }`.
   - Listens for `ServerMessage::PendingChoice`.
   - Fetches current redacted state (or reads attached snapshot).
   - Posts `(state, choice)` to the stateless advisor.
   - Samples option according to advisor probabilities (or picks argmax).
   - Sends `ClientMessage::SubmitChoice { nonce, expected_version, option_id }`.
3. **Zero Rules Engine Coupling**:
   - The bot runner itself does not need `libtorch` if it delegates evaluation to the advisor service over HTTP.

---

### Work Package BYOA-05: Web Client AI Advisor UI

#### Objective
Add an "AI Advisor" toggle and visualization to the React web client in `web/`, rendering policy recommendations and position evaluations for human players.

#### Technical Details
1. **Advisor Hook (`web/src/hooks/useAdvisor.ts`)**:
   - Configurable setting: `Advisor Service URL` (default `http://localhost:8081`, with ability to disable).
   - When a `pendingChoice` arrives and the viewer is the actor, asynchronously queries `POST /evaluate`.
   - Holds `{ value, options: Map<string, OptionAdvice>, head, loading }`.
2. **Visual Badges on Choices**:
   - On `PendingChoiceModal`, `Board` candidate hexes, and workflow drawers:
     - Render percentage badges next to legal options: `[ 68% ]`, `[ 24% ]`, `[ 8% ]`.
     - Highlight the top recommended choice with an accessible star or "AI Recommended" pill.
3. **Position Evaluation Gauge**:
   - Mount an unobtrusive win probability / expected progress bar $V(s)$ in the header status bar or Player Sheet.
4. **Non-Blocking & Graceful Degradation**:
   - If the advisor service is offline or unreachable, the UI functions normally with no errors or latency impact on human turns.

---

## 4. Verification & Testing Matrix

| Package | Test Type | Acceptance Gate |
|---|---|---|
| **BYOA-01** | Unit & Integration | `cargo test -p ti4-server --test projection_redaction` passes (5/5). Proves `view_for` is the sole source of redaction truth. |
| **BYOA-02** | HTTP Integration | Integration test verifies `GET /api/games/:id/state` returns redacted `GameState` matching `view_for` output and rejects invalid seat tokens. |
| **BYOA-03** | Service Integration | Unit/E2E test verifies `POST /evaluate` on standard positions returns finite probabilities summing to $1.0$ and finite $V(s)$. Latency $< 5$ ms. |
| **BYOA-04** | E2E Multiplayer | Automated match test: 1 human player (scripted/mock) + 2 headless bot agents play through multiple game rounds over WebSocket without server errors. |
| **BYOA-05** | Frontend Vitest | Vitest component tests verify option badges render when advisor data is present, and degrade cleanly when advisor is disabled. |

---

## 5. Non-Goals for this Phase
- Scrubbing draw decks (`action_card_deck`, `objective_deck`, etc.) inside `ti4-model` or `ti4-engine` (deferred to maintain zero non-server modifications).
- Embedding PyTorch / `libtorch` into `ti4-server`.
- Client-side WebAssembly neural network inference.
