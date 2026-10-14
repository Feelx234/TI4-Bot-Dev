# One seat in several tabs (web2), and web + web2 on one domain

## Context

**Goal 1.** A player opens the same seat of an online game in two tabs or on two devices
(desktop and phone). Both tabs show the live game. Both tabs also show the same staged work,
live. Today `web2/AGENTS.md` ("Online play") says: "A duplicated tab is the same player twice.
The server allows it ..., both tabs write the same staged work, and the page does not guard
against it."

**Goal 2.** web and web2 run against one server process, on one domain, behind the Caddy on
this host. Today they are two subdomains, `ti4.*` → :3000 and `web2.*` → :3100 (vite dev),
and both proxy `/api` and `/ws` to `:8080`.

**What exists already.** The server accepts several sockets per seat:
- `connect_player` (`crates/ti4-server/src/session/registry.rs:2174`) keeps a map of
  connections for each seat.
- `GameSession::subscribe` (`session/mod.rs:441`) and the planning runner `subscribe`
  (`planning/runner.rs:367`) have one subscriber for each connection.
- Updates, reaction modes and turn status already reach every tab of a seat.

**What breaks with two tabs of one seat:**
1. **Staged work is per tab.** This is the staged movement, landings and production; the
   private tactical draft with its undo history; and the secondary draft. `useLiveSession`
   reads it from `localStorage` once, at mount (`readPlanOf`, `readFollowOf`), and writes it
   on every change (`useLiveSession.ts:560-600`). The keys come from `onlineKeys(gameId,
   playerId)` (`OnlineApp.tsx:100`). So two tabs overwrite each other's work and never see
   the other's changes. Another device has none of it.
2. **The undone steps are told once.** `GameSession::take_undone` (`session/mod.rs:658`)
   removes them, and only the first connection that subscribes again after an undo gets them
   (`ws/mod.rs:601`). The other tab does not stage the undone steps again.
3. **A ready secondary is sent twice.** The page sends it when the game asks the seat
   (`useLiveSession.ts:666`). Both tabs send it, and one of them gets a "stale nonce" toast.
   The same happens when an answer is clicked in both tabs.

## Decisions (owner, 2026-10-14)

- **Staged work is synced through the server:** the draft, the staged
  movement/landings/production and the secondary draft. UI state stays per tab: the open
  sheet, the pane, the open step of the draft (`plan.open`, `stage`).
- **Layout of the domain:** web at `/`, web2 at `/web2/`, and `/api` and `/ws` go to the one
  server.

## Phase 1 — Server: staged work for each seat

**Protocol.** These changes are additive, and only a connection that opts in gets the new
messages. So web is not affected and `PROTOCOL_VERSION` stays 3.
- `ClientMessage::Subscribe` gets the optional field `staged: bool`
  (`protocol/client.rs`).
- New client message `PutStaged { game_id, slot, base_revision, value: Option<Value> }`.
  - `slot` is one of `"movement" | "plan" | "secondary"`.
  - `value` is opaque JSON; `None` clears the slot.
  - Bounds go in `validate_bounds`: about 48 KiB per value, under
    `MAX_CLIENT_MESSAGE_BYTES` (64 KiB).
- New server message `SeatStaged { game_id, revision, slots: {slot: {revision, value}},
  writer: Option<u64> }`.
  - It is sent in the initial snapshot path to opted-in player connections.
  - It is sent again after every accepted put, to every connection of that seat.
  - `writer` is the id of the connection that wrote the change, so the writing tab can skip
    its own echo.
- Each slot has a revision, and a put is compare-and-set. If `base_revision` is stale, the
  server answers `SeatStaged` with the current slots and does not write, and the client takes
  the server value. Last write wins only between equal bases.

**State and persistence.**
- Keep `BTreeMap<PlayerId, SeatStaged>` in the session's shared state, next to the reaction
  modes (`session/reaction_modes.rs`).
- Save it as `staged.json` beside `reaction_modes.json` (`storage.rs:410/782`, via
  `atomic_write_json`). Load it on recovery.
- Carry it across a session replacement (undo, redo, batch commit) the same way the reaction
  modes are carried.
- The server does not read the values, except the `ready` flag of the secondary in phase 3.
  The movement value carries its nonce, and the client already drops a value with a stale
  nonce.

**ws handler** (`crates/ti4-server/src/ws/mod.rs`):
- Handle `PutStaged` only for `ViewerRole::Player`. The seat always comes from the
  connection, never from the message, as `SetReactionMode` does it.
- Add a rate limit for each connection, like `MIN_INTENT_INTERVAL` but looser (for example
  20 per second). The client also debounces.
- Bridge staged broadcasts to the connections of the seat. Use a per-seat subscriber list on
  the session, as `subscribe_planning` does, or a `ServerMessage` sent through the existing
  subscriber channel and filtered by seat.
- Spectators and other seats never get it. The redaction tests must cover this.

**Undone steps reach every tab.**
- Replace the `remove` in `take_undone` with a stored `(session generation, steps)` for each
  seat.
- Every first snapshot of a player connection on that session gets the steps.
- Clear them when the seat's next decision is recorded.

**Tests** (in `crates/ti4-server/tests/it/`, a new `staged.rs`):
- Two sockets with one token: a put on A reaches B, with the writer id set.
- A stale `base_revision` is refused and answered with the current value.
- Another seat and a spectator get nothing.
- The value survives a restart (recovery) and an undo (session replacement).
- After an undo, both sockets get `undone`.
- An oversized value is refused.

## Phase 2 — web2: a seat store in place of `localStorage` for online games

- **New seam** in `web2/src/session/`: `SeatStore`, with
  `get(slot)`, `set(slot, value)` and `subscribe(listener)`.
  - `localSeatStore(storage, keys)` wraps the `localStorage` code of today. Local games
    (`LocalApp.tsx`) keep using it unchanged.
  - The online store is part of `createWsTransport` (`wsTransport.ts`). It sends
    `staged: true` with Subscribe, keeps the slots and revisions, and debounces puts (about
    250 ms). It ignores echoes with its own writer id.
  - It is exposed as an optional `Transport.staged`, so `game/`, `ui/` and `useLiveSession`
    still do not ask which transport they have.
- **Replace `DraftStore`.** `DraftStore` (`useLiveSession.ts:104`: storage + keys) becomes a
  `SeatStore`. `OnlineApp.tsx:100` passes `transport.staged` in place of
  `onlineKeys(...)` + `localStorage`.
- **React to the other tab** (the main client change). `useLiveSession` subscribes to the
  store. A change written by another tab replaces:
  - `plan.draft`: keep this tab's `open` and `stage`, and clamp `stage` when the draft
    shrank;
  - `follow` (the secondary);
  - the staged `local.movement`, `local.landing` or `local.production`, only when the saved
    nonce equals the open choice's nonce.

  Compare with `same()` before setting, so an applied remote value does not write back (no
  echo loop).
- **Migration.** On the first snapshot, if the server's slots are empty and the old
  `onlineKeys` entries exist, put them once and then remove the local keys. This sits beside
  `moveLegacy` in `onlineKeys.ts`.
- **Trim the plan history** when the serialized draft is over the size bound. Drop the oldest
  undo states first.
- **Tests:**
  - A unit test of the online store with a fake socket, in the style of `wsTransport.test.ts`.
  - A server case for `SeatStore`, added to the transport tests: a value set on one
    connection reaches a second connection of the seat.
  - A dom test: a remote change of the secondary draft shows in the seat row
    (`liveSecondary.test.tsx` style).
  - e2e in `e2e/online.spec.ts`: two tabs with the same `?online=<id>&seat=<p>`. Tab A
    stages a movement, and tab B shows the same tokens and check marks. Tab A edits the
    draft, and tab B's Draft tab follows.
- **Update `web2/AGENTS.md`, "Online play".** Replace the "duplicated tab" bullet: staged work
  is kept by the server for each seat and is the same in every tab and device; what is open
  on the screen is per tab.

## Phase 3 — One sender for answers of a seat

- **The server sends a ready secondary.**
  - When a put of slot `secondary` has `ready: true`, and the game then asks that seat the
    secondary, the server runs the script and applies it. This is the same check the page does
    today: `run_player_draft`, then `apply_player_draft` if the whole script still fits.
  - If the script no longer fits, the server clears `ready` in the slot. Every tab then shows
    "Your draft no longer fits the game. It is not ready."
  - The page no longer sends a ready draft itself (`useLiveSession.ts:666-690`).
  - This also removes the limit in AGENTS.md that "the page must be open when the seat is
    reached".
- **A double answer is quiet.**
  - Case: an answer, plan or apply arrives with a nonce that this seat has already answered,
    from another connection of the seat.
  - The server then rejects it with a new reason `answered_elsewhere`, in `RejectionReason`
    (`protocol/status.rs`) and in the TS mirror (`wire.ts`).
  - web2 shows no error toast for it (`wsTransport.ts` `rejection()`). The update that follows
    already shows the answer.
  - web ignores an unknown reason only if its parser tolerates it. Check `web/src/protocol`
    first; if it does not, send `stale_nonce` to connections that did not opt in.
- **Tests:**
  - A server test: two sockets, both mark ready, the secondary is applied once.
  - An e2e test: two tabs, a ready secondary, no toast in either tab.

## Phase 4 — web2 under `/web2/` on the same origin as web

**web2 build and dev:**
- In `web2/vite.config.ts`, set `base: "/web2/"`.
- Add the new host to `allowedHosts`.
- Keep the proxy for local development without Caddy.
- `/api` and `/ws` stay absolute root paths (`socket.ts` `pageSocket`, `fetch("/api/…")`), so
  they need no change.

**Links that assume `/`.** Grep for `href="/"`, `"/?`, `location.assign`,
`location.href =`, and `new URL(…, location.origin)`. Use `import.meta.env.BASE_URL`.
- The "Start" link (`OnlineApp.tsx`, `START`).
- The addresses built in `onlineStart.ts`.
- The start-screen rows (`StartApp.tsx`).
- `public/manifest.webmanifest`: `start_url` and `scope` become `/web2/`. Check the paths of
  the icons.
- `web2/playwright.config.ts`: `baseURL` and `webServer` URL.

**Server serves both clients.**
- Add an optional `TI4_WEB2_DIR` in `crates/ti4-server/src/public/mod.rs`, next to
  `TI4_WEB_DIR`.
- In `http/mod.rs`, route `/web2/*` to a second `ServeDir` with its own `index.html`
  fallback. Factor the cache-header logic out of `spa_fallback` (`http/mod.rs:196`). The
  cache paths become `/web2/assets/` immutable and the rest `no-cache`.
- `/web2` without the slash redirects to `/web2/`.

**Public mode** (only if the public server will serve web2; this was open question 1 of
`plans/2026-10-12-LOBBY_PLAN.md`):
- web2 needs the password screen. It uses `GET`/`POST /api/access`, as web does; reuse its
  flow.
- The public server must refuse `&seed=` and the scenarios. The server already refuses the
  dev routes.
- Check that the access cookie's `Path` is `/`, so both clients share the login.

**Caddy on this host, dev form.** One site, for example `ti4.beckermann-zibert.com`:

```
ti4.beckermann-zibert.com {
	@server path /api/* /ws/* /health
	reverse_proxy @server localhost:8080 { transport http { read_timeout 0 } }
	handle /web2* { reverse_proxy localhost:3100 }   # vite, base /web2/ (HMR goes through too)
	handle       { reverse_proxy localhost:3000 }     # web vite
}
web2.beckermann-zibert.com { redir https://ti4.beckermann-zibert.com/web2{uri} }
```

**Production form** (`deploy/public/`): the server serves `/`, `/web2/`, `/api` and `/ws`,
and Caddy keeps its single `reverse_proxy` as in `deploy/public/Caddyfile`.
- `deploy.sh` also builds web2 (`npm ci && npm run build`) and installs `dist2/`.
- `env.example` gets `TI4_WEB2_DIR=/opt/ti4-public/current/dist2`.
- Update the README diagram.

**Shared `localStorage` on one origin.** The two clients do not clash:
- web uses `ti4.player-session:<id>` and `ti4_*`.
- web2 uses `ti4.online.*`, `ti4.single`, …
- `ti4.nickname` is shared on purpose.

**Optional: one seat in web and in web2.**
- web2 also accepts web's `ti4.player-session:<id>` as a source of the player session. It
  copies the session and does not move it, so web keeps working. Read web's value format
  first.
- A link "Open in web2" in web and "Open in web" in web2 gives a web tab and a web2 tab of
  the same seat.
- web does not opt in to staged sync, so its staged work stays its own.

## Critical files

**Server:**
- `crates/ti4-server/src/ws/mod.rs`
- `crates/ti4-server/src/protocol/{client,server,status}.rs`
- `crates/ti4-server/src/session/{mod.rs,worker.rs,registry.rs}` (staged state, undone,
  carry-over)
- `crates/ti4-server/src/storage.rs`
- `crates/ti4-server/src/http/mod.rs`
- `crates/ti4-server/src/public/mod.rs`
- `crates/ti4-server/tests/it/staged.rs` (new)

**web2:**
- `web2/src/session/{wsTransport.ts,wire.ts,transport.ts,useLiveSession.ts,onlineKeys.ts}`
- `web2/src/dev/{OnlineApp.tsx,LocalApp.tsx,onlineStart.ts,StartApp.tsx}`
- `web2/vite.config.ts`, `web2/public/manifest.webmanifest`, `web2/playwright.config.ts`
- `web2/AGENTS.md`

**Deploy:**
- `deploy/public/{Caddyfile,deploy.sh,env.example,README.md}`
- `/etc/caddy/Caddyfile`: the owner edits this; nothing is installed by the repo.

## Verification

- **Rust** (gates from `web2/AGENTS.md`):
  - `cargo fmt --all -- --check`
  - `cargo l-web`
  - `cargo t-web staged::`, then the whole `cargo t-web`
  - `cargo b-web`
  - `cargo t-web-slow` before the push
- **web2:**
  - `npm run typecheck`, `npm run lint`
  - `npm test`, which includes the transport suite against `target/debug/server`
  - `npm run e2e -- -g "same seat"`, then the whole `npm run e2e`
- **Manually on this host:**
  - Start `target/debug/server`, the web vite (:3000) and the web2 vite (:3100, base
    `/web2/`), and apply the Caddy snippet.
  - Open `https://ti4.beckermann-zibert.com/web2/?online&fill=random` on desktop, and open the
    same `?online=<id>&seat=<p>` on the phone.
  - Stage a movement on one device and see it on the other.
  - Mark a secondary ready on both devices and see it applied once, with no toast.
  - Undo on one device; both devices restage the undone steps.
  - Open `https://ti4.beckermann-zibert.com/` (web) in another tab and check that it still
    works against the same server.
- **Production form:** set `TI4_WEB_DIR` and `TI4_WEB2_DIR` locally and check that `/`,
  `/web2/`, `/web2/?online=…` (SPA fallback), and `/web2/assets/*` serve with the right cache
  headers.
