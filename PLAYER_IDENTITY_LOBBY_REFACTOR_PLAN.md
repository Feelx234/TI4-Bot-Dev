# Player Identity and Lobby Admission Refactor

## Status

PIL-01 model/persistence implementation was committed as `e0f234d` on
2026-09-23. PIL-02 was committed as `cf4a01f`; PIL-03 as `37c8823`.
PIL-04 was committed as `83957c3`; PIL-05 as `bd8f2c1`. PIL-06 is implemented
in the working tree, pending independent review. Package progress and verification results are tracked here, without
separate evidence artifacts.

Proposed breaking-change plan. The current game is not live, so no compatibility
adapter, legacy endpoint, or persisted-data migration is required. Existing saved
unstarted lobbies and active sessions may be rejected by the new server version.
This is a hobby project for a small, trusted group (probably four simultaneous
players). Prefer straightforward recovery and understandable behavior over account
systems or elaborate lease/transaction machinery. Breaking API and storage changes
are acceptable before launch. A person must be able to resume their player from a
different computer after a client crash, even if the original credential is lost.

## Objective

Make humans and remote bot agents enter a game through the same automatic
admission flow. A client supplies a game ID, the server assigns the next open
lobby position for a new participant, and the server issues a private resumable
player-session credential. Clients must not choose a position or receive a session
credential as a command line or URL parameter. Someone without a credential may
also take over an *existing disconnected player* from a different computer;
that operation does not create a new participant or change their position.

The host may reorder players while the game is a lobby. Starting the game
atomically freezes the roster and its order. Reordering a player never changes
that player's identity or session credential.

## Decisions

### Identity, session, and position are distinct

Use three separate concepts:

| Concept | Meaning | Lifetime | Authority |
|---|---|---|---|
| `PlayerId` | Stable game participant identity. It is created on first player admission. | Lobby through game completion | Server generated |
| Player-session credential | Opaque, high-entropy bearer credential that resumes one `PlayerId`. | Until replacement on takeover or game deletion | Server generated and private to the client |
| Lobby slot / seat position | A physical ordered position in the lobby, optionally occupied by a `PlayerId`. | Mutable only during lobby | Host reorders; server assigns first open position |

`PlayerId` is not a position such as `p1`. It is retained when its holder moves
from one position to another. The engine's seating order is an ordered
`Vec<PlayerId>` derived once from the occupied lobby slots when the game starts.

Connection presence is a fourth, **ephemeral** concept. A heartbeat tracks whether
a player is currently connected; it never expires a credential, frees a slot, or
changes a `PlayerId`. After a server restart, presence starts as disconnected,
while persisted credentials remain valid.

Keep the word "seat" for game-rule and UI concepts that depend on physical
position. Do not use it in authentication names or credential storage.

### Admission and spectator behavior

`POST /api/games/{game_id}/lobby/join` is the one player entry endpoint.
It has no requested-position input. A new client omits the optional
`x-ti4-player-session` header and requests a new participant; a reconnect
supplies its existing credential. A client that lost its credential requests
takeover of an explicitly selected, disconnected `PlayerId` instead. Make
new admission and takeover distinct typed request variants so an empty slot
cannot accidentally be interpreted as an existing player.

- A new player admission is permitted only during `Lobby` phase.
- Under the registry lock, the server selects the first empty slot in current
  lobby order, creates a `PlayerId` and player session credential, fills that
  slot, sets readiness false, persists the whole updated lobby, and returns the
  result. It returns a conflict when all slots are occupied.
- A reconnect supplies the private player-session credential, receives the same
  `PlayerId`, and never consumes another slot. A valid credential works even if
  the player is currently shown as disconnected.
- A client without a credential may explicitly take over a disconnected player
  in either `Lobby` or `Running`. The server atomically replaces that player's
  credential, preserves their `PlayerId`, position, readiness and host status,
  and invalidates the previous credential. Takeover never uses an empty slot.
  If the player is connected, reject the request; if two takeovers race, only
  one succeeds. Reserve the player as present for a short connection grace
  period when takeover succeeds, so a second request cannot rotate the new
  credential before its recipient can connect. A lost client with the old
  credential must not retain access to private updates or choices after
  replacement.
- A spectator makes an explicit spectator request or uses an unauthenticated
  read-only route. Spectators never invoke admission and never occupy a slot.
- After `Running`, new admission and all seating mutations are rejected. An
  existing authenticated player may reconnect; an unauthenticated client may
  spectate or explicitly take over a disconnected existing player.

Anyone with the game link can take over a disconnected player. This is an
intentional trust tradeoff for the small group, not proof of the person's
identity. Do not add accounts, recovery codes, or host approval. Show a
"Rejoin as Player X" option only after presence has been absent for a short,
documented grace period (for example, heartbeat every 10 seconds and takeover
after 30 seconds without one). A lost connection does not itself rotate the
credential; normal reconnect with the old credential should work immediately.
Track presence per authenticated player and connection, and prevent an old
connection from continuing to receive private data after takeover. Presence
may be reconstructed as disconnected after a server restart; apply the same
grace period before offering takeover.

The game ID is an invitation locator, not an action credential. The private
player-session credential is still required after assignment for reconnects,
readiness, host actions, and choice submission. The explicit disconnected-player
takeover is the one exception that issues a replacement credential to someone
who has only the game link. Credentials must never appear in a URL, lobby
roster, logs, public snapshot, or error response.

### Host and seating controls

Creating a game creates its slot count, assigns the creator to the first slot,
and records that player's `PlayerId` as `host_player_id`. The create response
delivers its private session credential directly to the creating client, which
stores it just as it stores a later join credential.

Add one host-authorized lobby-only reorder operation. It accepts a complete
permutation of the existing slot IDs or occupied player IDs, validates it
exactly, updates the ordered slots and lobby version atomically, then persists.
It cannot add, remove, duplicate, or replace participants. The host remains the
host even if moved to another position.

The UI should show a stable player display label and position number, not imply
that `PlayerId` is a position. A host moves players and open slots; a joining
player is always placed in the first currently open slot.

Use a complete permutation of **slot IDs** for reorder. Each slot moves with
its current occupant (or emptiness); this represents moves of open positions
without guessing which player ID stands for an empty slot. Host authority is
limited to lobby reorder and start. After start, the host has no special
capability. An abandoned unstarted lobby may simply be discarded;
there is no automatic host succession or credential-expiry cleanup.

### Bot-agent lifecycle

`ti4-bot-agent` accepts `--game` and its advisor configuration, but no `--seat`
or `--token`.

1. It calls lobby join before the game has started.
2. It retains the returned private credential in process memory and marks
   itself ready, just like a human participant.
3. It sends authenticated presence heartbeats while waiting to start. These
   report presence; they do not renew or extend a credential.
4. It connects after the game begins, authenticates using that retained
   credential, and obtains its `PlayerId` from the authenticated server reply.
5. It sends application-level protocol `Ping` messages at a bounded interval
   matching the presence interval, processes `Pong`, and resumes with the same
   credential after a transport disconnect. If its process crashes and loses
   the credential, an operator may start a new bot process and explicitly
   choose its disconnected player from the public rejoin list. The agent must
   not automatically take over a disconnected person, even when only one is
   listed; selection is interactive, not a `--seat` argument.

The bot must pass the server-authenticated `PlayerId` to the advisor and reject
a pending choice for any other player. It must never infer identity from seating
position or accept a player identity supplied by command line configuration.

## Target Data Model

Replace the current lobby model in `crates/ti4-server/src/session/registry.rs`:

```rust
struct LobbyState {
    game_id: String,
    phase: LobbyPhase,
    host_player_id: PlayerId,
    slots: Vec<LobbySlot>,
    players: BTreeMap<PlayerId, LobbyPlayer>,
    seed: u64,
    lobby_version: u64,
}

struct LobbySlot {
    slot_id: LobbySlotId,
    occupant: Option<PlayerId>,
}

struct LobbyPlayer {
    ready: bool,
    session: PlayerSession,
}

struct PlayerSession {
    credential: String,
}
```

Use dedicated typed IDs for `LobbySlotId` if they cross a public or persistence
boundary. Preserve deterministic order with `Vec` for slots and `BTreeMap` for
player-keyed data. Generate `PlayerId` and credentials with cryptographically
strong random values, validate their bounds at all protocol boundaries, and
ensure generated player IDs cannot collide with an existing lobby participant.

At `start_lobby`, require every slot to be occupied and every player ready.
Derive exactly one ordered `Vec<PlayerId>` from `slots`; persist it in the game
initialization record and pass it to `create_game_with_map`. The running
`GameSession` owns that order and never exposes any mutation path.

For an active game, session credentials map to `PlayerId`, not a slot. Existing
`seat_tokens: BTreeMap<PlayerId, String>` becomes a player-session credential
map or a dedicated session-authentication service keyed by `PlayerId`. Naming
must consistently use `player_session` or `resume_token`, never `seat_token`.
Store credentials durably so an ordinary server restart does not lock out
players. Do not serialize credentials into public views or include them in
`Debug` output. A takeover must persist the replacement before returning it,
and revoke the old credential in both lobby and running-session authentication;
recovery must load the replacement rather than an older game-init credential.
For running games, keep the current credentials in a small atomically replaced
per-game record separate from the immutable game-init record, or use an equally
simple single authoritative record. Do not write a replacement only to memory
or only to the original init file while another recovery path loads stale data.
No credential expiry timestamp or expiry sweep is needed.

## HTTP and WebSocket Contract

Replace the following lobby API behavior:

| Current | Replacement |
|---|---|
| `players: ["p1", "p2", ...]` at creation | A bounded `player_count` that creates ordered empty slots |
| `POST /lobby/claim` with `{ "seat": "p2" }` | `POST /lobby/join` with no chosen position; server assigns the next open slot |
| `creator_token` | Private `player_session` for the host player |
| `credential` from claim | Private `player_session` from join |
| `x-ti4-seat-token` | `x-ti4-player-session` for authenticated player operations |
| Public roster's `seat`, `available`, and controller assignment | Ordered slots with position, occupancy, public player label, and coarse connected/disconnected presence |
| `bot_seats` selecting server-side `BotFirstOption` | Remove from the public create flow; remote bots join exactly like humans |

Define typed request and response DTOs, use strict Serde decoding, and document
the credential fields as private. Never put a session credential inside a
broadcast `LobbyResponse` or serializable `LobbyState`.
For no-token join, distinguish `{ "kind": "new" }` from
`{ "kind": "takeover", "player_id": "player_..." }`; the latter is allowed
only for an existing disconnected participant and never accepts a position.
The public roster lists eligible disconnected players so the UI can offer an
explicit rejoin modal. The server still validates eligibility at submission.

Suggested responses:

```json
// POST /api/games/{game_id}/lobby/join
{
  "player_session": "opaque-private-bearer-credential",
  "player": { "id": "player_..." },
  "lobby": { "game_id": "...", "phase": "lobby", "slots": [] }
}

// explicit spectator lobby read
{
  "game_id": "...",
  "phase": "lobby",
  "slots": []
}
```

For the running WebSocket protocol, replace `Subscribe.seat_token` with an
optional `player_session` credential. With a valid credential the server derives
`ViewerRole::Player(PlayerId)`; without one it derives `ViewerRole::Spectator`.
The server sends the authenticated viewer identity in `InitialSnapshotMsg` and
`StateUpdateMsg`, as it already does. The credential is never echoed by a server
message.

Application `Ping` updates authenticated presence. Raw WebSocket control pings
remain transport-only and do not affect presence. Invalid or revoked
credentials must fail closed without changing lobby/game state. Browser and bot
clients send authenticated presence heartbeats in the lobby and application
`Ping` during the game. A takeover revokes the old credential and terminates
or downgrades its old subscription before it can receive more private updates;
every choice still authenticates against the current credential.

## Persistence and Failure Rules

- Version the lobby and game-init schemas rather than attempting implicit
  deserialization of the current seat-token records. Bump the WebSocket
  protocol version with the renamed subscription field; old clients need not
  remain compatible.
- This breaking change may reject old persisted records with a clear schema
  error. Do not reinterpret a prior `p1` identity as a movable player without a
  reviewed migration.
- Persist creation, join, takeover, reorder, readiness, and the game-init
  record before returning credentials or start success. Individual file writes
  should remain atomic; there is no requirement for a multi-file transaction
  to protect an unstarted lobby. Treat a valid game-init record as the start
  commit point on recovery; if it is absent, recover as an unstarted lobby
  or discard it. Do not report a started game as recoverable without a valid
  game-init record. Once running, preserve the existing careful decision-log
  and replay recovery behavior.
- On storage failure, do not leak a newly generated credential and leave the
  in-memory lobby or running authentication unchanged. A failed takeover
  leaves the old credential usable. No presence observation alone changes
  roster, readiness, seating order, or credentials.
- If an occupied lobby slot is explicitly vacated before start, retire that
  `PlayerId` and revoke its credential; a later new join gets a new identity.
  Provide an authenticated leave operation for non-host players in the lobby;
  a host who abandons their lobby can discard that unstarted game. Never
  vacate a slot or replace a player automatically after start.
- Every authorization check resolves credential -> `PlayerId`; it must not
  depend on B-tree iteration order, slot iteration outside explicit first-open
  selection, wall-clock values other than the documented presence decision, or
  client-provided player IDs.

## Implementation Packages and Tests

Execute these in order. Each row is one bounded package with focused tests;
split a row further if necessary. The named files are primary edit scopes, not
permission to change unrelated code. Record additional necessary paths in this
plan before editing. The acceptance-test numbers refer to the list below. Run
formatting, focused tests, and affected-crate tests for every code package.
The server may be temporarily unusable between breaking-change commits on this pre-launch branch;
do not claim an intermediate package is a deployable release.

### PIL-01 — Identities and versioned records

- **Depends:** None. **Primary scope:** `ti4-model` ID definitions (only if a
  new typed slot ID is needed), `ti4-server/src/session/registry.rs`,
  `ti4-server/src/storage.rs`, and their focused tests.
- **Contract:** Define ordered slot IDs, stable generated player IDs, private
  player-session credentials, and bounded versioned lobby/game-init records.
  Loading an old record gives a clear schema error. Public serialization and
  `Debug` do not reveal credentials. Define where current running credentials
  live so recovery cannot reload an obsolete value after takeover; do not add
  an account system or expiry service.
- **Gate:** Serialization/size/corruption tests, unique-ID collision tests,
  private-output tests, and an old-format rejection test pass (acceptance 1,
  11, 12 for the model).
- **Progress (2026-09-23):** Added typed `LobbySlotId`, 256-bit generated
  `player_` IDs with collision retry, redacted `PlayerSession`, bounded v2
  private lobby, immutable game-init and authoritative current-session records,
  and a credential-free public lobby projection. Focused tests cover round
  trips, forced ID collision, checksum/size/reference failures, old-record
  rejection, and restart loading of a rotated current credential.
- **Integration boundary:** PIL-02 connected v2 lobby writers/readers and
  running-session recovery through the current-session record. Legacy direct
  session APIs still support v1 records; PIL-04 must ensure rotated credentials
  replace both the current-session record and the active authenticated view.
  Live takeover remains unimplemented. The v2 game-init record is the start
  commit point; a running lobby record without init recovers as unstarted.
- **Security boundary:** v2 persistence DTOs are intentionally
  serializable for disk only; client handlers must serialize the explicit
  `PlayerLobbyView` projection and never a private record. Debug output for
  existing token-bearing lobby/init DTOs is redacted during the transition;
  their private on-disk serialization remains for existing server behavior.

### PIL-02 — New admission and lobby authorization

- **Depends:** PIL-01 model/persistence code. **Primary scope:** registry lobby
  transitions, `ti4-server/src/http/games.rs`, routes, and lobby tests.
- **Contract:** Create takes `player_count` and returns the host credential;
  a new `join` selects the first open slot, while a credential-bearing `join`
  resumes the same player. Authenticated non-host leave retires that identity;
  readiness/start authorization resolves credential to `PlayerId`. Public
  spectator reads do not join. Remove public `bot_seats` and `p1` creator
  assumptions here. Reserve the typed `takeover` join variant for PIL-04;
  until then reject it explicitly, without mutating state.
- **Gate:** Concurrent first-open joins, full lobby, reconnect, leave/new ID,
  spectator read, ready/start authorization, and persistence-failure tests pass
  (acceptance 1–5, 11 as applicable).
- **Additional edit paths:** `crates/ti4-server/src/http/mod.rs`,
  `crates/ti4-server/src/storage.rs`, and focused server tests for routing,
  durable v2 start/recovery, and verification.
- **Progress (2026-09-23):** Implemented `player_count` creation with generated
  host identity and private `player_session`; first-open `/lobby/join`,
  credential reconnect, authenticated non-host leave, readiness/start, public
  spectator lobby read, and explicit rejection of the reserved takeover request.
  The new HTTP paths use only bounded v2 lobby records. Starts persist current
  credentials and a credential-free init; restart recovery loads the current
  credential record. WebSocket messages still use the v1 `seat_token` field
  until PIL-03; the registry recognizes v2 credentials there without leases.
  Pre-launch browser/bot clients still require their later packages.
- **Verification:** `cargo test -p ti4-server` passed (including concurrent
  first-open admission, storage-failure rollback, HTTP authentication and
  spectator reads, v2 restart/replay, and existing WS/protocol coverage);
  `cargo fmt --package ti4-server` applied; `cargo clippy -p ti4-server
  --all-targets` passed with existing dependency warnings. Strict `-D warnings`
  remains blocked by unrelated warnings in `ti4-model` and `ti4-engine`.
- **Next:** PIL-03 replaces the WebSocket subscription field, adds authenticated
  presence/heartbeat behavior and revocation-aware subscriptions; PIL-04
  implements explicit takeover. PIL-05 adds host slot reorder and strengthens
  start/recovery crash-boundary testing. PIL-08 removes legacy direct-registry
  seat/lease paths and updates remaining clients and docs.

### PIL-03 — Authenticated presence and WebSocket identity

- **Depends:** PIL-02. **Primary scope:**
  `ti4-server/src/protocol/`, `ti4-server/src/ws/`, registry presence, and
  their focused tests. Do not implement takeover in this package.
- **Contract:** Bump the protocol version; `Subscribe.player_session` resolves
  the actor, missing credentials mean spectator, and choice submission checks
  current authentication. HTTP lobby heartbeats and application `Ping` update
  ephemeral presence, never credential validity. A documented short grace
  period controls the disconnected display; restart begins with no presence.
  Raw WebSocket control pings do not mark a player present. Provide a way for
  subscriptions to stop private delivery if their credential is later revoked.
- **Gate:** Spectator privacy, invalid credentials, ping/timeout boundary,
  brief credential reconnect, per-choice authentication, and restart-presence
  tests pass (acceptance 4, 5, 11, 12).
- **Additional edit paths:** `crates/ti4-server/src/http/games.rs` (heartbeat),
  and `crates/ti4-server/tests/{player_lobby_admission,protocol_roundtrip,size_bounds,ws_lifecycle}.rs`.
- **Progress (2026-09-23):** WebSocket protocol v3 uses optional
  `Subscribe.player_session`; the old field and protocol version are rejected.
  The authenticated server-reported viewer is derived from the current registry
  credential, and choice submissions reauthenticate before dispatch. Subscribe
  registers an ephemeral per-connection presence record; application `Ping`
  refreshes it, HTTP `/lobby/heartbeat` records an authenticated heartbeat,
  and control pings and spectators do neither. Public slots expose `connected`
  and `can_take_over` (a display hint, not an authorization decision). The
  documented interval is 10-second heartbeats and a 30-second absence grace;
  presence is not persisted, never expires credentials, and restart begins
  disconnected with a fresh grace window. Connection close begins its grace
  window. New admissions have their own grace window even in an old registry.
  Subscriptions and outbound delivery check current authentication and stop
  private updates after revocation; the idle socket also checks at most every
  250 ms. The update forwarder is async and cancels when the outbound channel
  closes. Debug formatting of subscribe messages redacts credentials.
- **Verification:** `cargo fmt --package ti4-server` and `cargo test -p
  ti4-server` passed, including protocol-v3 rejection, spectator/invalid-token
  subscriptions, application-vs-control ping, grace expiry, credential
  continuity, and restart presence. The first full test attempt hung in two
  WebSocket lifecycle tests because a blocking subscription forwarder survived
  runtime shutdown; replacing it with a cancellable async forwarder made the
  isolated WebSocket suite and the full affected-crate suite pass. Full Clippy
  was not run (blocked by `ti4-model`). No independent review was performed.
- **Next/PIL-04:** Implement durable credential rotation and a short takeover
  reservation under the same registry lock as eligibility checks; clear stale
  presence for the replaced credential so its old connection cannot refresh
  or reintroduce presence on disconnect. Update both lobby authentication and
  the authoritative running-session credential record before returning the
  replacement. Serialize choice authorization with credential rotation so a
  previously authorized choice cannot race a takeover, and verify an already
  connected old subscription cannot receive a private message after commit.
  The `can_take_over` hint must be rechecked atomically on submission. PIL-06
  and PIL-07 must send application pings every 10 seconds; PIL-08 updates
  remaining older client/fixture protocol fields.

### PIL-04 — Disconnected-player takeover

- **Depends:** PIL-03. **Primary scope:** registry credential
  rotation, current-credential storage, `ti4-server/src/http/games.rs`,
  `ti4-server/src/ws/`, and focused server/recovery tests.
- **Contract:** The explicit no-token `join` takeover variant selects an
  existing disconnected `PlayerId` in lobby or running game, rotates and
  durably saves its credential before returning it, preserves identity and
  seating, and prevents old live subscriptions from seeing more private data.
  The recipient gets a brief connection grace reservation; a connected player
  cannot be taken over. No host approval, code, or expiry is added.
- **Gate:** Connected-player refusal, concurrent takeover, old-token refusal
  for HTTP/WS/choices, old-subscription closure, storage-failure rollback,
  lobby/running takeover, and restart-loading-new-token tests pass (acceptance
  5, 9, 11, 12).
- **Additional edit paths:** `crates/ti4-server/src/session/mod.rs` (keep the
  active session's internal credential map current),
  `crates/ti4-server/tests/player_lobby_admission.rs`, and
  `crates/ti4-server/tests/ws_lifecycle.rs`.
- **Progress (2026-09-23):** The no-credential typed takeover request now
  rotates only an existing disconnected player, under the registry lock after
  rechecking the 30-second absence/grace rule. A new credential is persisted
  before delivery; lobby takeovers atomically replace the lobby record, while
  running takeovers atomically replace the authoritative current-session record.
  In-memory lobby and active session authentication then switch to the new
  credential; old presence connection IDs are discarded and the recipient gets
  a fresh connection grace reservation. Recovery overlays the running lobby's
  possibly stale credential copies with the authoritative session record.
  HTTP snapshots and WS choice submissions serialize authentication with
  takeover; old WebSocket connections stop on revoked-credential checks.
  Concurrent requests produce one winner, without changing player identity,
  position, readiness or host. No old credential appears in public responses.
- **Verification:** `cargo test -p ti4-server --test player_lobby_admission
  --test ws_lifecycle` passed (8 + 6 tests); `cargo fmt --package ti4-server`
  applied and `cargo test -p ti4-server` passed (all unit, integration and doc
  tests). Full Clippy was not run, as requested (blocked by `ti4-model`).
  No independent review was performed; do not treat PIL-04 as independently
  reviewed or deployable on its own.
- **Next/PIL-05:** Implement the complete slot-ID reorder with exact
  permutation validation and host authorization under the same registry lock;
  verify first-open admission after reorder, immutable started order, and
  crash-before/after-init recovery with running credential rotations retained.
  PIL-06/PIL-07 should use the new explicit takeover request only when the
  public `can_take_over` hint is true and handle conflict if presence changes.

### PIL-05 — Reorder and committed start

- **Depends:** PIL-04. **Primary scope:** registry reorder and
  start, game-init storage/recovery, HTTP reorder route, and focused tests.
- **Additional edit paths:** `crates/ti4-server/src/http/mod.rs` for the
  reorder route and `crates/ti4-server/tests/player_lobby_admission.rs` for
  HTTP, concurrency, and recovery-boundary coverage.
- **Access:** P1; writable paths are the primary and additional paths above
  plus this plan. No external reference, network download, persistent worker
  process, or external-state change; only bounded local test storage.
- **Contract:** Only the lobby host may submit a complete slot-ID permutation;
  join and reorder serialize. Start requires full occupancy/readiness and
  derives exactly one ordered player vector for the engine. A valid init record
  is the start recovery commit point; a partial pre-start transition never
  presents an active game. Host has no post-start privilege.
- **Gate:** Reorder/empty-slot/join race, bad permutations, host preservation,
  exact engine order, rejected post-start changes, and crash-before/after-init
  tests pass (acceptance 6–9, 12).
- **Progress (2026-09-23):** Added host-authenticated `POST
  /api/games/{game_id}/lobby/reorder` accepting strict `{ "slot_ids": [...] }`.
  The registry validates an exact permutation, including empty slots, under
  the same lock as join/start. A changed order increments the lobby version
  and is saved before publication; storage failures leave memory and disk
  unchanged. The host's identity, credential and readiness stay with the
  occupant. Start already derives its only engine order from the final slot
  vector, and writes current sessions and the running lobby before the
  credential-free init commit. Recovery treats a Running lobby lacking init as
  unstarted and treats valid init as started, even if the lobby phase is stale;
  rotated running credentials still come from the authoritative sessions file.
- **Verification:** `cargo fmt --package ti4-server`, focused
  `cargo test -p ti4-server --test player_lobby_admission` (13 passed),
  `cargo test -p ti4-server` (all unit/integration/doc tests passed), and
  `git diff --check` passed. Focused cases cover concurrent join/reorder,
  exact slot validation and HTTP authorization, persistence rollback, ordered
  initialized engine players, and both sides of the init crash boundary with
  post-start credential rotation. Full Clippy was not run as requested
  (blocked by `ti4-model`). No independent review was performed.
- **Next/PIL-06 and PIL-07:** Browser host controls should submit the entire
  current `slot_id` order, including open slots, to `/lobby/reorder`; show
  the returned positions and handle authorization/conflict responses. Bot
  clients should use the reordered public positions for display only; their
  authenticated `PlayerId` remains their sole acting identity. PIL-08 must
  verify the actual browser/bot start path and full workspace gate.

### PIL-06 — Browser flows

- **Depends:** PIL-04 and PIL-05. **Primary scope:** `web/src/`
  UI, protocol decoding and storage, plus focused browser tests.
- **Contract:** Show Join, Watch, and eligible "Rejoin as Player X" actions,
  the stable public label and position, lobby-only reorder controls, and
  heartbeat/ping presence. Store credentials in tab-scoped storage, never
  navigation state or URLs; use the authenticated `PlayerId` after reconnect.
- **Gate:** Browser tests cover new join, watch without admission, remote-
  computer takeover with no stored credential, old-token invalidation, host
  reorder, readiness, and protocol-version handling (acceptance 4–9, 11).
- **Progress (2026-09-23):** Browser creation now sends `player_count` and
  stores the returned private `player_session` only in per-game tab-scoped
  `sessionStorage`. A public lobby GET does not join; an authenticated
  `/lobby/join` reconnect obtains the server-authenticated `PlayerId` (the
  public lobby deliberately has no viewer field). A credential-free client
  can Join the next open slot, Watch as spectator, or explicitly select a
  public `can_take_over` player; a 409 race is shown as an error. Revoked
  credentials are removed after failed reconnect/heartbeat. The lobby shows
  stable player labels, position numbers, readiness and presence; host-only
  controls submit a complete slot-ID permutation including empty slots.
  Game HTTP snapshots and WS subscribe use `x-ti4-player-session` and v3
  `player_session`; player sockets send application pings every 10 seconds
  and reconnect on transport loss with the same in-memory credential. A
  snapshot with a different server-authenticated viewer is refused. HTTP
  lobby heartbeats continue at 10 seconds, including while running.
- **Verification:** `npm run build` (content check, TypeScript and Vite)
  and `npm test` (25 files, 159 tests) pass; browser unit tests exercise
  create, admission vs spectator reads, fresh-computer takeover, old-token
  refusal, host reorder payload, readiness, v3 subscribe/pings/reconnect,
  viewer identity mismatch and v2 rejection. `git diff --check` passes.
  Full Clippy was not run as requested (blocked by `ti4-model`). No
  independent review was performed. The existing v2 golden fixtures are
  identified as legacy and rejected at protocol ingress; regenerating the
  server fixtures and the remaining old Playwright manual-claim flows remain
  PIL-08 integration work. At this point no v3 browser/server E2E had run;
  the focused lifecycle run below was added in the UI follow-up.
- **Create-response correction (2026-09-23):** A real create response was
  rejected by the browser because its old shared string validator capped
  *all* identifiers and credentials at 64 characters. The server-generated
  `player_` ID is 71 characters and the `session_` credential is 72. The
  decoder now uses separate bounds for game/slot IDs (64) and player IDs/
  player sessions (128, matching the WebSocket session bound). A regression
  test decodes the full-length generated shapes on both create and join and
  rejects an oversized credential. Build and all 159 web tests pass after
  this fix; the host's already-created lobby can be reached with its private
  credential only if the original response was stored (the previously
  rejected response was not stored by the browser).
- **Lobby UI and leave follow-up (2026-09-23):** Removed the visible game ID
  and raw `PlayerId` strings from the lobby and rejoin controls. `Player N`
  is derived from the fixed `slot_N` label, which moves with its occupant;
  the current position is shown separately. Copy Game URL copies the current
  browser origin and path, without a credential or query string. Non-host
  lobby participants now see **Leave lobby**, which calls the authenticated
  `/lobby/leave` transition and forgets the tab credential only after a
  successful response; server storage failures leave the credential and
  player in place. The server forbids host leave (no host succession), so
  the misleading Forget button is not shown to the host. Running-game Exit
  Game remains a local exit, not a mid-game roster change.
- **Actual browser/server gate:** Replaced the old manual-claim
  `lobby_lifecycle.spec.ts` with a real Chromium create → spectator watch →
  join → leave → verify slot opens → rejoin → ready → start → reload flow.
  `cargo build -p ti4-server --bin server` passed; the focused Playwright
  test passed (1/1, 5.5 s) against isolated local ports 38080/33000 because
  8080/3000 were already in use (those processes were not touched). Default
  ports remain unchanged; the test runner accepts optional port overrides.
  `npm run build`, `npm test` (25 files, 161 tests), and `git diff --check`
  passed. This is a focused E2E flow, not a claim that the remaining legacy
  Playwright suites or the full PIL-08 gate pass. No independent review or
  full Clippy run was performed.
- **Next/PIL-07 and PIL-08:** Bot agents should retain the join-issued
  credential and server-authenticated identity; do not infer it from lobby
  position. PIL-08 should replace legacy v2 browser fixtures and manual-seat
  Playwright workflows with real create/join/watch/takeover/reorder/start
  flows against the current server, including a running spectator snapshot,
  credential revocation and idle-period heartbeat coverage. Review the
  browser/server boundary before treating this as deployable.

### PIL-07 — Bot-agent flows

- **Depends:** PIL-04 and PIL-05. **Primary scope:**
  `crates/ti4-bot-agent/` and focused bot integration tests.
- **Contract:** Remove `--seat`/`--token`; join from game/advisor configuration,
  mark ready, heartbeat while waiting, send application pings when running,
  and reconnect with the existing credential. A fresh process without that
  credential can offer an interactive selection of eligible disconnected
  players for explicit takeover; it never selects one automatically. Only an
  authenticated server-reported `PlayerId` goes to the advisor; reject choices
  for any other player.
- **Gate:** Bot E2E covers ready/start, idle longer than the presence grace
  period, disconnect/reconnect, fresh-process takeover, and advisor identity
  (acceptance 9–11).

### PIL-08 — Contract cleanup and end-to-end gate

- **Depends:** PIL-06 and PIL-07. **Primary scope:** obsolete
  lobby/claim/seat-token paths, README, protocol fixtures, server/web E2E,
  storage recovery tests, and integration docs. List exact paths in the task
  spec; preserve unrelated tests and utilities.
- **Contract:** Remove obsolete public claim, manual-seat, and `seat_token`
  contracts across server and clients; test the actual new HTTP/WS flow. Old
  persisted formats may fail clearly. Do not weaken running-game recovery.
- **Gate:** All acceptance tests 1–12 pass through appropriate unit and E2E
  layers; format, affected crates, workspace suite, protocol round-trips,
  concurrency, recovery, and a real bot E2E with idle time past the presence
  grace period pass.

## Required Acceptance Tests

1. Creating a three-slot game assigns the creator exactly one stable player
   identity and host session; the returned public lobby contains no credential.
2. Two concurrent new joins receive distinct players and the first two open
   slots in server order; exactly one join wins each slot.
3. A full lobby rejects new player admission without state change.
4. A spectator read or spectator WebSocket subscription never creates a player,
   reports authenticated presence, or changes lobby version.
5. A reconnect using a valid session returns the same `PlayerId` and does not
   consume an open slot; an invalid session is rejected without mutation.
   A brief disconnect does not invalidate that credential. When a non-host
   explicitly leaves before start, their old credential cannot reconnect and
   the next new join receives a different `PlayerId`.
6. Host reorder preserves each player's credential, readiness, and host status;
   the next join uses the first open slot in the new order.
7. Only the host can reorder; malformed, duplicated, missing, or unknown slot
   permutations are atomic rejections.
8. Start rejects incomplete or unready lobbies and, once successful, freezes
   the exact final player order used by the engine.
9. After start, new admissions and reorders fail; valid existing sessions
   reconnect and can act as their original player identity. An unauthenticated
   client may take over a disconnected player after the grace period and gets
   the **same** `PlayerId`; a connected player cannot be taken over. Exactly
   one concurrent takeover succeeds. Old credentials and old live subscriptions
   lose access immediately, including after a server restart.
10. A bot started with only game/advisor configuration joins, survives an idle
     interval exceeding the presence grace period through authenticated
     heartbeats/pings, marks itself ready, reconnects as the same player, and
     sends advisor requests only for its authenticated player. A new process
     without the lost credential can take over its disconnected player.
11. Credentials never occur in public lobby responses, spectator snapshots,
     broadcast messages, URLs, structured logs, or debug output covered by tests.
12. A server restart preserves player credentials and running-game recovery;
     a crash before a complete game-init record does not misreport an active
     game. Presence reconstructs as disconnected without expiring credentials.

## Non-Goals

- User accounts, passwords, third-party identity providers, or cross-game
  identity persistence.
- Allowing a client to pick a position.
- Mid-game roster or seating-order changes.
- Automatic replacement of disconnected players after game start.
- Account-backed identity verification: anyone with the game link can explicitly
  take over a disconnected player's identity. No recovery codes, timed session
  expiry, automatic slot release, or host approval for takeover.
- Compatibility with the existing manual claim API, `p1`/`p2` identity scheme,
  old saved lobby records, CLI parameters, or WebSocket field names.

## Risks to Review

- This is an authorization and persistence-schema redesign; test every public
  boundary, restart path, and credential revocation path before release.
- Confirm game views, replay records, and advisor payloads treat `PlayerId` as
  an opaque stable identity and do not assume `p1` through `p8`.
- Preserve deterministic seating semantics: automatic admission and host
  reorder must be serialized and the final ordered player vector must be the
  only ordering passed to engine setup.
- Presence is a convenience signal, not proof of identity. Test grace-period
  edges, server-restart presence, concurrent takeovers, and revocation of old
  subscriptions; the game link must be treated as an invitation to a trusted
  group.
- Ensure private credentials cannot be copied into debug derives, error text,
  test snapshots intended for public views, or browser navigation state.
