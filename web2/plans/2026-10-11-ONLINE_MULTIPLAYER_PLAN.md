# web2: online multiplayer

Status: plan, 2026-10-11. Nothing is built yet.

## Goal

web2 has two modes: **Single player** (today's experience, the wasm engine in the page) and
**Multiplayer** (the authoritative `ti4-server` over a websocket). The first multiplayer version
connects as the first seat of an 8-player game; the other seats are random deciders. The bare
address stays the local game; the Dev sheet has the way into multiplayer (decision 5).

The UI (`game/`, `ui/`) must not know which one it is in. The only places that may know are the
entry (`main.tsx`) and the transports in `session/`. Content, rule text and the
catalogue come from the local wasm in both modes.

## Decisions (owner, 2026-10-11)

1. **Derived facts are computed on the server**, per viewer, with the same `ti4-view` code that the
   wasm host uses, and sent in `StateUpdateMsg`. The client never derives them from the redacted
   `state`.
2. **Drafts run on the server.** The wasm cannot run a draft in multiplayer: it has no full game.
   `Transport.runDraft` becomes asynchronous.
   The server gets two script messages (owner, 2026-10-11), and web2 does not use
   the planning runner (`StartPlanning`, `SubmitPlanningChoice`, `ApplyPlanning`). The runner,
   the batch and `web/` can be deleted (owner, 2026-10-11), but not in this plan: see "Postponed".
   - `ClientMessage::RunDraft { game_id, request_id, script }` is answered with
     `ServerMessage::DraftOutcome { request_id, base_version, outcome }`. The server forks the
     planning checkpoint (`session/planning.rs`, `checkpoint`) and calls
     `ti4_view::planning::draft::run_draft`, as the wasm host does. The server keeps no draft:
     the script stays in `localStorage`. It works on the turn of the seat too.
   - `ApplyDraft { nonce, expected_version, choices, steps }` sends a draft or a plan to the live
     game (`submitPlan` is the same with no `choices`). The server answers the questions with
     `ti4_view::planning::steps::plan_answer`. The result has the shape of `PlanStopped`.
   - The owner does not need a draft that is applied while the page is closed, or a draft on
     another device. So the server keeps no script.
   - What a draft may be asked is the same as in the runner: both use `guard_fork` and
     `audit_offer` of `ti4-view/src/planning`. A card, an ability or a reaction window of the seat
     in the action phase is asked as in the live game (Flank Speed after the activation); its
     answer is an entry `{ kind: "choose", option_id }` of the script. A window in a space battle
     or an invasion ends the draft, in both.
3. **Undo**: every player can undo their own actions back to the point where hidden information was
   revealed, randomness was used, or another player acted. Further undo is host-only (the event
   log, as in `web/`). The event log is not in web2 yet, so host-only undo is **out of scope**.
   Single player keeps its undo with no limit (owner, 2026-10-11): it is a tool for testing.
4. **The server is at the address of the page**, as in `web/`: `/api/...` and
   `ws(s)://<host>/ws/games/<id>`. `web2/vite.config.ts` gets the `proxy` block of
   `web/vite.config.ts` (`/api`, and `/ws` with `ws: true`; the port from `TI4_E2E_BACKEND_PORT`,
   8080 by default). Caddy needs no change. There is no `?server=<url>`.
5. **No mode picker for now.** `/` stays the local game. The Dev sheet has a row "Multiplayer"
   that loads the online address: `?online` starts a game, `?online=<game_id>` is that game. An
   online game has the header link "Local game", as the demo has.
6. **Single player and multiplayer use the same code** wherever they can (owner, 2026-10-11), so
   that a test of single player is mostly a test of multiplayer. A rule or a fact has one
   implementation, in Rust, and both hosts (`ti4-wasm`, `ti4-server`) call it. What this asks for:
   - the facts: one function of `ti4-view` (phase 1a);
   - a draft: `run_draft`, in both;
   - a plan or a draft that is sent: one function of `ti4-view` over `plan_answer` that both hosts
     call. The wasm host gets an export for it, and the TypeScript `planAnswer` is removed
     (`NEXT.md` has this as open). Phase 0;
   - Never cards: `ti4_engine::reaction_modes::NeverOffer` in both. The wasm host gets an export
     to set the mode of a card, and `filterOffers` of the page is removed. Phase 0;
   - the other seats: `SeededRandom` in both;
   - the page: `useLiveSession` and everything above it has no branch for the mode.
   What stays different, on purpose: the undo (decision 3), the dev tools of a local game, and
   the saved game.
   One suite of tests of the `Transport` contract runs against both transports (phase 2).
   The aim is less code, not more: each of these removes a copy. The two transports end as thin
   adapters over the same operations (an update, an answer, a script that is run, a script or a
   plan that is sent, the mode of a card, an undo): one calls an export of the wasm, the other
   sends a message.

## Non-goals

- Lobby UI for several humans, joining a game, spectating (the transport takes a `viewer` already;
  the screens come later).
- Host-only history (`change_history`), restore, redo, the event log.
- Seat changes (`switchSeat`), stepping and scenarios online: these are dev tools of a local game.
- A mode picker as the start screen, and the installed app in multiplayer.
- Any new layout. Pattern 10 of `AGENTS.md` holds: live, draft and history are one screen.

## Findings that shape the plan

- `Transport` (`session/transport.ts`) is already the seam, and its comments name the websocket
  (`StateUpdateMsg`, `SubmitChoice`). `useLiveSession(transport)` is the only consumer.
- `SessionUpdate` (`session/wire.ts`) = `StateUpdateMsg` fields + six facts that "the wasm host
  fills": `tactical`, `exploration`, `action_cards`, `secret_objectives`, `score`, `agendas`. The
  server sends none of them (`NEXT.md` says the same for `exploration` and the catalogue).
  `project_session_update` (`ti4-view/src/projection.rs`) sets all six to `None`. They are filled
  by hand in three places: `ti4-wasm/src/lib.rs` (two) and `ti4-view/src/planning/draft.rs`.
- `runDraft` is synchronous and is consumed inside a `useMemo` (`useLiveSession.ts` ~l.528).
- The server's planning runner does not fit the draft of web2. It keeps the draft on the server
  (`PlayerPlan`) and takes one answer for each message, with an `AttemptIdentity`. It refuses the
  seat that has the turn (`session/mod.rs`, `PlanningError::ActivePlayer`); web2 drafts on its
  own turn too. Its answer (`PlanningEnvelope`) has no `SessionUpdate`, no `tactical` facts and
  no `movement`, `invasion` or `production` of a `DraftOutcome`.
- The batch of the server is HTTP (`POST /api/games/{id}/batch`, header `x-ti4-player-session`),
  and `ApplyPlanning` sends the draft that the server keeps. Neither fits
  `applyDraft(nonce, { choices, plan })`.
- The server has no random seat. `SeatController` is `Human`, `BotFirstOption`,
  `BotAlwaysDecline`, `BotScripted`; `ti4_engine::choice::SeededRandom` is there and not used.
  `add-bot` of the lobby starts another process behind a password. `POST /api/games` makes a
  lobby: the game starts with `ready` and `start`.
- The server's `change_history` is host-only. There is no player undo yet.
- `Transport` mixes game play with dev tools: `setStepping`, `step`, `switchSeat`; the Dev sheet is
  "in a local game" only.
- Never cards: wasm mode filters offers in the page (`filterOffers`), the server keeps them itself
  (`SetReactionMode`, `reaction_modes` in the update). `NEXT.md` already lists this.
- `web/src/protocol/` (`client.ts`, `decode.ts`, `planning.ts`, `types.ts`) is a working client of
  the same protocol. Read it; do not import it (`web/` is another app). web2's `wire.ts` names only
  the part that web2 reads.

## Phases

Each phase leaves single player working and green (`npm test`, `e2e`, `boundaries.test.ts`).

### Phase 0: the shape of the seam (web2 and ti4-wasm, no behaviour change)

1. **Async draft.** `Transport.runDraft(script)` returns `DraftOutcome | null` now. Change the
   contract to a subscription: `draft(script)` is asked, the outcome arrives as a
   `TransportEvent` `{ kind: "draft", script, outcome }`, with the script echoed so the page
   drops a late answer for an older script. `useLiveSession` holds
   `outcome` in state (`pending` while waiting) instead of a `useMemo`.
   - The wasm transport answers in the same tick (a microtask). This keeps one code path.
   - Check what the draft screens show while an outcome is pending: keep the last outcome until
     the new one arrives (no flicker), and keep the main button disabled while the outcome is for
     an older script.
2. **Capabilities.** Replace the dev methods by a `capabilities` object on the transport:
   `{ stepping, switchSeat, scenarios, save }`, each absent online. `GameSession` exposes it; the
   Dev menu entry shows from `capabilities`, never from a mode flag. `LocalApp` keeps the saved
   game and the scenarios; they are not part of `Transport`.
3. **Never cards move into the engine of both hosts** (decision 6). `useLiveSession` stops
   filtering offers. The wasm host wraps the seat in `NeverOffer` and has an export to set the
   mode of a card; `filterOffers` is removed. The websocket transport reads `reaction_modes` and
   sends `SetReactionMode`. The page's `cards.never` is a view of the transport. Check that the
   saved game and the undo of a local game still hold (`AGENTS.md`, Action cards): the decline
   of a window is then no saved answer.
4. **Undo as a fact, not a method.** `canUndo` stays in the update event. Its meaning becomes
   "this seat may take back its last answer" in both modes (see phase 3).
5. **One answer to a plan** (decision 6). A function of `ti4-view` answers a parked question from
   the steps of a plan (`plan_answer`), and the wasm host has an export for it that is called
   while the game is parked. `submitPlan` and `applyDraft` of the wasm transport use it; the
   TypeScript `planAnswer` is removed.

Tests: the existing `wasmTransport.test.ts`, `useLiveSession` tests and `neverOffer.test.tsx` pass
unchanged in meaning; add a test for a late draft outcome that must be dropped.

### Phase 1a: one place fills the facts (Rust, wasm only, no behaviour change)

1. `ti4-view` gets `FactsContext { content, sources, galaxy }` and one function that fills the
   six facts of a `SessionUpdate` (`ti4-view/src/{tactical,exploration,action_cards,secret_objectives,score,agendas}.rs`).
   `project_session_update` takes the context and calls it. The three places that fill the facts
   by hand use it. Do not keep two copies (see the note on `plan_answer` in `NEXT.md`).
2. Guard: `wasmTransport.test.ts` and the recorded fixtures of web2 do not change.

### Phase 1b: the server sends the facts, plays the other seats and runs a script (Rust)

1. **Facts.** `StateUpdateMsg` and `InitialSnapshotMsg` (`projection.rs`) carry the six facts, per
   viewer, from the **full** game state. They are optional fields (`skip_serializing_if`), so
   `web/` and the fixtures keep parsing. `GET /api/content/catalog` exists; web2 uses the wasm one.
2. Redaction tests, in the style of `tests/projection_redaction.rs`: an opponent view and a
   spectator view contain no deck order, no other seat's hand, no secret objective of another
   seat in any of the new fields, nor in `held_action_cards`, `held_secret_objectives` and
   `scored_secret_objectives` of `PlayerView`.
3. Golden fixtures: add the new fields to the actor fixture; keep `web/`'s fixture test green.
4. Write down the size of one update of an 8-seat game with the facts. web2 does not read
   `state`; act only if the size is a problem.
5. **Random seats.** `SeatController::BotRandom` with `SeededRandom`, seeded from the seed of the
   game and the seat. `fill_seats: "random"` on `CreateGameRequest` fills the empty seats, and
   readies and starts the lobby in the same request. Test: after a restart of the server the
   game is the same (the answers of the seats are in the decision log, or come from the seed
   again: read `prior_decisions` first). The e2e of phase 2 needs this.
6. **Script messages** (decision 2): `RunDraft` and `ApplyDraft`, each with an integration test
   that plays the scripts of the tests in `ti4-view/src/planning/draft.rs`.
7. A test of `run_draft` for a card of the seat in a window: Flank Speed after the activation,
   played and declined, as `tests/it/planning_runner.rs` has for the runner. `ti4-view` has no
   such test yet.

### Phase 2: the websocket transport (web2)

New `session/wsTransport.ts` implementing `Transport`:

- `connect(gameId, playerSession)`, at the address of the page (decision 4): `Subscribe`, then `InitialSnapshot` and `StateUpdate` map
  to `TransportEvent` `update`. `PROTOCOL_VERSION` is checked; a mismatch is an `error` event with
  the versions in the text.
- `submitChoice(nonce, optionId)` sends `SubmitChoice` with `expected_version` from the last
  update. `ActionRejected` (stale version, stale nonce) becomes the next update (a refreshed
  snapshot) and a toast; it is never silent.
- `submitPlan` and `applyDraft` send `ApplyDraft`. Its result becomes `plan` of the next update
  (`PlanStopped`). The batch of `session/batch.rs` is not used, so its missing step for the
  landings does not matter here.
- `draft(script)` sends `RunDraft` with a new `request_id`; the answer is the `DraftOutcome`, with
  no mapping. An answer for an older `request_id` is dropped. When `game_version` changes while a
  draft is open, the transport sends the script again.
- Not read for now, and named in `wire.ts` as such: `events` and `Event`, `auto_resolved`,
  `history`, `current_path`, `PendingChoice`, `TurnStatus`.
- The keys of `localStorage` (`planKey`, `draftKey`, `cardsKey`) are per `game_id` online. The
  staged movement is kept under the nonce of its choice.
- Reconnect: exponential backoff, resubscribe with the stored `player_session`, the first update
  after it is a full snapshot (`game_version`). While disconnected, an `error`-free
  `{ kind: "offline" }` state is shown in the toolbar (one signal, pattern 6).
- `catalog`: the same wasm catalogue as in single player (`Engine.catalog`), so the transport takes
  the `Engine` too.
- **Handshake check**: the server's content/engine version must equal the wasm's. Add a field to
  the snapshot (hash of `ti4-content` as built in) and compare it with `Engine.hash`'s content
  part; refuse to play on a mismatch. Without this, a stale CDN bundle shows wrong card text.

Tests: a fake websocket in vitest that replays the fixtures of `ti4-server/fixtures`
(actor, opponent, stale submission, game over); an e2e against a real `ti4-server` started by
`playwright.config.ts` (see how `web/e2e` does it), through the `proxy` block of decision 4,
playing the first turns of a seeded game with random seats.

One suite of tests of the `Transport` contract (decision 6) runs against the wasm transport and
against the websocket transport with a real server: an update after an answer, a refused plan, a
plan that another question stops, a draft, a draft that is applied, a Never card. A case that
only one mode has (undo, stepping) is outside the suite.

### Phase 3: undo for the seat (Rust + web2)

Rule: a seat may undo its own decisions back to, but not past, the first of: a hidden
information reveal (a card drawn, an exploration or a secret objective shown), a random result
(a roll, a shuffle), or a decision of another seat.

1. **Server**: a per-seat undo point. The decision log (`DecisionLog`, replay in
   `session/replay.rs`) already records every decision. A *barrier* is where a draft would end:
   use the checks of `ti4-view/src/planning` (`audit_offer`, `validate_knowledge`,
   `activity_stop`) on the recorded decisions, and where the actor changes. Do not add marks to
   the engine for it. A decision of a random seat is a decision of another seat. `canUndo` for a seat is
   "the last decision is its own and lies after the last barrier". Undo replaces the session like
   the host history does (`session/registry.rs` `HistoryAction`), but is allowed for the seat and
   bounded by the barrier.
2. **Protocol**: `ClientMessage::Undo { protocol_version, game_id, expected_version }`; the update
   carries `can_undo` per viewer (extend `HistoryStatus` with a bool; do not reuse `cursor`, which
   is the host's). A rejected undo says why (`barrier: "revealed"|"random"|"other_seat"`).
3. **web2**: `wsTransport.undo()` sends it; `canUndo` of the update drives the button, as now.
   The existing page logic for "an undo took back a movement, stage it again" needs the steps
   back: the server must return them as `undone` (the same shape as `PlanStep[]`), or the page
   re-derives them from its own staged plan. **Decide when starting**; the wasm transport's
   `undone` is the reference.
4. The wasm transport keeps its undo with no barrier (decision 3). So single player undoes
   further than multiplayer: this difference is wanted.

### Phase 4: the entry

1. **Entry** (decision 5): `main.tsx` routes `?online` and `?online=<game_id>` to the online
   app. The Dev sheet has the row "Multiplayer"; an online game has the header link "Local
   game". `/`, `?local=…`, `?scenario=…`, `?example=…` and `?gallery` are untouched.
2. **Multiplayer start** (`?online`): `POST /api/games` with `player_count: 8`, the nickname, a
   seed, the map and `fill_seats: "random"` (phase 1b); keep the `player_session` in
   `localStorage` per `game_id`; set the address to `?online=<game_id>`; open the websocket
   transport as seat 1.
3. **Resume**: a reload with a stored `player_session` reconnects to the same game; a
   `game_id` that the server no longer knows says so and offers the local game.

### Phase 5: the draft of a secondary (Rust + web2)

The owner needs it (2026-10-11). It is built for single player first; with decision 6 the online
draft is then the same code.

1. `run_draft` takes a scope: `PlanScope::Tactical` (as now) or `PlanScope::Secondary { primary,
   card }`. The checks are there and the server's runner uses them (`audit_secondary_offer`,
   `Game::prepare_hypothetical_secondary`, `StopReason::SecondaryComplete`); read
   `ti4-server/src/planning/runner.rs` for how it prepares the copy. `RunDraft` and the wasm
   export carry the scope.
2. web2: the row of the viewer in the seat list of a strategy card opens the editor as a private
   draft (`AGENTS.md`, Strategy cards: "✎ Private draft", "✎ Draft ready"). The script is kept
   as the tactical draft is.
3. "Draft ready": when the live game asks the seat for its secondary, the page sends the script
   (`ApplyDraft`). The page must be open; the server keeps nothing (decision 2).
4. A secondary is drafted only while its card is resolved, as in the runner. A draft before
   that (a card that another seat holds and did not play yet) is not in this plan.

Then a screen for the lobby of several humans is the next plan, not this one.

## Postponed: delete what web2 replaced

**Postponed (owner, 2026-10-11) until every UX feature of `web/` is in web2.** That is after
this whole plan. Nothing of it is done in phases 0 to 5: `web/`, the planning runner and the
batch keep working, and a phase of this plan must not break them.

What can go then:

1. `ti4-server`: the planning runner (`planning/runner.rs`, `session/planning.rs`), the batch
   (`session/batch.rs`, `POST /api/games/{id}/batch`), their messages (`StartPlanning`,
   `ResetPlanning`, `EditPlanningMovement`, `ApplyPlanning`, `SubmitPlanningChoice`,
   `SecondaryPlanning`, `PlanningUpdate`, `PlanningStatus`, `PlanningResult`) and their tests.
   About 4000 lines of source; no crate outside `ti4-server` uses them. Raise `PROTOCOL_VERSION`.
2. `web/`.

Before that, list what `web/` has and web2 does not (the lobby, the host history, the event log,
the map picker, the advisor, and what else is found), and bring each to web2 or ask the owner.

## Checks that every phase must pass

- Rust (phases 1a, 1b, 3): `cargo c-web`, `cargo t-web`, `cargo fmt --all -- --check`, `cargo l-web`.
  Server integration tests: `cargo test -p ti4-server --test it <file>::`. Do not build other crates.
- Rebuild `session/ti4.wasm` (`scripts/build-wasm.sh`) after a change of a fact in `ti4-view`.
- A changed screen is checked on desktop and phone; phase 2 adds a toolbar state ("offline") and
  phase 4 a row of the Dev sheet and a header link, so both layouts, 8 seats and the long names
  apply (`AGENTS.md`, patterns 6, 13).
- Phase 4 adds a row to the Dev sheet. Add a line for it to `AGENTS.md` "Local play", and open
  the row once in `e2e/local.spec.ts`, in the same change. `/` stays the local game.

## Risks

- **A draft against a moving game.** In multiplayer the other seats act while this seat drafts its
  next turn. The base of the draft changes. The transport sends the same script again for the
  new `game_version`; a step that is no longer legal ends the run as "refused" or "stopped",
  which the draft screen already shows. The script is kept. Do not drop the staged work silently.
- **Latency in the draft loop.** Today every change re-runs the draft in the same tick. Over a
  network each change is a round trip. Keep the last outcome visible (phase 0) and send only the
  latest script; if the server cannot cope, add a debounce in the transport (not in the page).
- **Two copies of the plan rules** (`planAnswer` in TypeScript, `plan_answer` in `ti4-view`) already
  drift. Online play uses `plan_answer` on the server (`ApplyDraft`) and not the TypeScript copy,
  so no third copy is made.
- **Hidden-information leaks** are a server property. The redaction tests of phase 1 are the guard;
  review any new field against `ti4-server/tests/projection_redaction.rs` before it ships.
- **Version skew** between the page bundle, the wasm and the server: the handshake check of phase 2.

## Order and size

Phase 0 and phase 1a are independent and can run in parallel (web2 and Rust). Phase 1b needs 1a.
Phase 2 needs 0 and 1b. Phase 4 can start with a fake transport once phase 0 is in. Phase 3 is
last of the first version and may ship after the first playable multiplayer: without it, the
Undo button is simply not shown online. Phase 5 needs phase 0 only for single player, and
phase 2 for online.

| Phase | Where | Size |
| --- | --- | --- |
| 0 seam | web2, ti4-wasm | large (the async draft; plans and Never cards move to Rust) |
| 1a facts in one place | ti4-view, ti4-wasm | small |
| 1b facts, random seats, script messages | ti4-server | large |
| 2 transport | web2 | medium |
| 3 undo | ti4-server, web2 | medium |
| 4 entry | web2 | small |
| 5 secondary draft | ti4-view, ti4-wasm, ti4-server, web2 | large |

## Open

- `ApplyDraft` sends the action, the system and the steps, as the local game does now. The
  answers that a draft recorded for a card or an ability are not sent, so the live game asks
  them again. To send the whole script instead is a change of both modes: ask the owner.
- Phase 3, step 3: the server returns `undone`, or the page derives it.

## Questions and answers

The owner asked these on 2026-10-11. They explain decisions 2 and 6.

### How a draft works

**When does the page talk to the server?** At three times: after every change of the draft,
whenever the live game changes, and at Apply. A draft is not local. The page keeps only the list
of the answers of the player (the script). After each change the engine plays that list, and the
screen is made from what the engine says. Single player works like this already
(`Transport.runDraft`): there the engine is the wasm in the page, online it is the server.

**What happens, step by step?**

1. The player opens the draft. The page sends the script `[tactical]`. The server plays it on a
   copy of the live game and says which systems can be activated. The page shows them.
2. The player activates system 27. The page sends `[tactical, 27]`. The server says what is
   next: a question of a card, or the ships that can move.
3. The player moves a cruiser. The page sends `[tactical, 27, move cruiser]`. The server says
   how the board is after the move, and what can still move.
4. The same for each move, each landing and each purchase: one message, one answer.
5. The player sends the draft with Apply. Now the server gives the answers to the live game,
   one after the other, and every seat gets the update.

The server discards the copy after each run. It keeps nothing of the draft.

**What is local?** The script itself (`localStorage`), and what is staged in one step before it
is recorded (a build that is not paid). The rules, the legal moves, the windows of the cards and
the board after a move come from the engine.

**Another player acts in the live game. How does the draft show it?** The live game has a new
version, and the page gets its update as always. The page then sends the same script again, and
the server plays it on a copy of the new game. If every answer is still legal, the draft is the
same on the new board. If one is not (ships of the other player are in the system now), the run
stops at that answer: the screen says which part does not fit, and the script is kept. A draft
is never brought up to date; it is always the script on the newest game.

### Cards and abilities in a draft

**The answers go as a list. How does the server know which cards to offer?** It does not look
at the list for this. The game asks its questions one after the other, also on the copy, and it
has the hand of the player. For each question the server takes the next answer of the list. A
question that this answer fits (a move, a landing, a purchase) is answered, and the run goes
on. A question that the list has no answer for (a window of a card) stops the run, and the page
shows that question.

**Example: Flank Speed (+1 move after an activation).** The script is `[tactical, 27]`. On the
copy the game opens the window "after you activate a system", because the player holds the
card. The script has no answer, so the run stops there and the page shows "Flank Speed" and
"Decline". The player chooses. The page puts the answer into the script and sends it again:
`[tactical, 27, flank speed]`. Now the copy plays the card, and the ships that can move have +1.
A card that triggers at a production works the same way, before the first purchase.

**Do the other seats play cards in a draft (Sabotage)?** No. `guard_fork`
(`ti4-view/src/planning/fork.rs`) prepares the copy, for the server's runner and for `run_draft`
alike. On the copy, an optional reaction of another seat is not asked, and nothing of its hand
is read. The abilities of the player and every ability that is not optional run as in the game.
A decision that another seat must make ends the draft there. So a draft shows what happens if
nobody stops the player. At Apply the live game asks the other seats; if one of them reacts, the
plan stops there and the rest is staged again.

**What does not work?**

- A window in a space battle or an invasion ends the draft. An action card as the action
  itself is not drafted: the menu of a draft has the tactical action only. Both are limits of
  the checks in `ti4-view`, so the server's runner has them too.
- A question in the middle of a step has no place in the script of the page yet (`NEXT.md`). A
  question before a step has one.
- Apply sends the action, the system and the steps. It does not send the answer of a card, so
  the live game asks it again (see "Open").

### Why the script, and not the drafts that the server has

**What can the server's drafts do that the script cannot?** All of it comes from one fact: the
runner keeps the draft on the server, the script is in the browser only.

- A draft that is applied while the page is closed. Not needed (owner).
- A draft on another device. Not needed (owner).
- A draft that a host undo keeps.
- The draft of a secondary. Needed (owner): phase 5 adds it to `run_draft`.

Nothing of this is lost for good. A script is a short list, and the server can keep one when a
feature needs it.

**What is harder with the script?**

- Each change plays the whole script from its start. The runner keeps a worker at the last
  question. A tactical action has some tens of answers and the wasm plays them in one tick;
  the server with 8 drafting seats is not measured yet.
- Each answer is a whole update with the facts, and up to three more (movement, landings,
  production).
- A client can send many scripts: the server needs a limit per seat and runs only the newest.
- A draft of more than one action would grow the replay. This plan has none.

**What is easier?** A change of an early step (the run checks the later steps; the runner has
messages and state for this), and a game that moved, a reload and a new connection (the script
is sent again; the runner needs an identity on each message).

**Why the script, then?**

- The draft of web2 is a script already: its screens, its undo and redo, its saved state and
  its tests.
- The runner would need a new draft in web2, or an adapter that plays the whole script into it
  on each change. That adapter is the script with more messages.
- The runner must change in any case: it refuses the seat that has the turn, and its answer has
  no facts for the screens.
- Single player and multiplayer then run a draft with the same function (decision 6).

### The same code in both modes

**Is this more code?** No. The aim is one copy of each rule. Today three things exist twice:
once in TypeScript for single player, once in Rust for the server. Each change of decision 6
removes the TypeScript copy and lets the wasm call the Rust one.

**What does "Never cards move into the engine" mean?** No new rule. In single player the page
reads each window and answers "decline" when it offers only Never cards (`filterOffers`). On the
server a wrapper of the seat does this before the player is asked (`NeverOffer` of `ti4-engine`).
The wasm host is to use that wrapper too; then the page only says "never offer this card", in
both modes, and `filterOffers` is deleted.

**Why is phase 0 large, when nothing is added?** "Large" is the work, not the amount of code.
Single player has logic of the game in TypeScript, and it must go to Rust before the modes can
share it:

1. A plan that is sent: `planAnswer` in TypeScript and `plan_answer` in Rust do the same. The
   wasm needs an export that takes the plan while the game is parked. This is the largest part;
   the export for a draft shows that it can be done.
2. Never cards, as above. The decline of a window is a saved answer of a local game now, and is
   not after the change: check the saved game and the undo.
3. The result of a draft arrives later, not in the same tick: a change in the middle of
   `useLiveSession`.

**What is the end state?** The two transports are thin: an update, an answer, a script that is
run, a script or a plan that is sent, the mode of a card, an undo. One calls an export of the
wasm, the other sends a message. Everything above them is one code.

## Done when

- Single player behaves as before: all tests and e2e green, no new import from `session/` into
  `game/` or `ui/` (`boundaries.test.ts`).
- Multiplayer: a browser starts a game as seat 1, plays tactical actions with drafts, the Cards,
  Secrets, Objectives and Agendas sheets show the same facts as in single player, a reload resumes,
  and an e2e plays the first round against a real server.
- The Undo button shows exactly when the server says `can_undo`.
- No component in `game/` reads a mode flag.
