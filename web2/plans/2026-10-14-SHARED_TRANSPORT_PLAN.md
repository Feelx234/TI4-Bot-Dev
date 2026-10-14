# Shared web module for transport and single player (web/ + web2/) (2026-10-14)

Status: planned, not started.

## Context

`web/` and `web2/` both stay maintained, and each has two backends for a game:

| | server | single player (wasm engine) |
|---|---|---|
| web/ | `GameSessionClient` (`src/protocol/client.ts`, WS + HTTP, own reconnect in `resilience.ts`) | `LocalTransport` → Web Worker → `HostRunner`/`Host` (`src/offline/local/`) → wasm `ti4_host_*` = `HostSession`, speaks `ServerMessage` |
| web2/ | `wsTransport.ts` over `socket.ts` (reconnect, backoff, heartbeat), speaks `ServerMessage` | `wasmTransport.ts` (966 lines) on the **main thread**, bespoke C-ABI (`ti4_play*`, `ti4_draft`, `ti4_plan`, `ti4_journal`, …) |

Same wasm binary, two JS drivers, two wasm protocols. `web2/plans/2026-10-14-DEDUPLICATE_HOSTS_PLAN.md`
Step 5 already decides that web2 moves onto `HostSession` and its `ServerMessage` decoding.

**Is the UI-facing transport interface worth sharing? No.** web's `GameTransport` is a stateful
store (`GameSessionState` via `reduceServerMessage`) with history/splice/turn-redo/advisor/reach;
web2's `Transport` is an event stream (`TransportEvent`) with drafts, plans, `SeatStore`, odds and
test capabilities. Merging them means rewriting one UI's state model for no runtime gain.
**The layer below is worth sharing:** a message-level *link* — send a `ClientMessage` (or a local
host command), receive `ServerMessage` objects plus connection status — with two implementations,
websocket and wasm engine. Then each app's transport is one adapter over a link, and web2 gets one
transport for online and single player.

The link stays untyped at the message level (`unknown` in, `object` out): each app keeps its own
wire types (`web/src/protocol/types.ts`, `web2/src/session/wire.ts`). Unifying those is a later,
separate step.

## Mechanism

New dependency-free TS source dir `web-shared/` (repo root), imported as `@ti4/shared/...`:
- both `vite.config.ts`: `resolve.alias { "@ti4/shared": <root>/web-shared/src }`, `server.fs.allow` incl. `..`.
- both `tsconfig.json`: `paths { "@ti4/shared/*": ["../web-shared/src/*"] }`, add the dir to `include`.
- tests of `web-shared` run in web2's vitest (`include` gets `../web-shared/src/**/*.test.ts`); web2 biome lints it (`biome.json` files). web/ only type-checks what it imports.
- `web-shared/AGENTS.md`: what lives here (no React, no app types, no deps), how it is tested; linked from the Topics table in `web2/AGENTS.md` and from `web/README.md`.

Layout:
```
web-shared/src/
  engine/  host.ts (Host, hasJspi, Envelope/HostSave types), runner.ts (HostRunner),
           worker.ts (Worker entry), protocol.ts (ToWorker/FromWorker/HostPort), port.ts (Worker HostPort factory)
  net/     socket.ts (Socket, connect, backoff, HEARTBEAT_MS, pageSocket)
  link/    link.ts (GameLink, LinkEvent), socketLink.ts, engineLink.ts
```

## Steps (one commit each)

1. **Scaffold + move the engine driver (pure move).** `git mv` web/'s `hostApi.ts`, `hostRunner.ts`
   (+test), `hostWorker.ts`, `hostProtocol.ts`, `jspi.ts` into `web-shared/src/engine/`; web/
   imports via `@ti4/shared/engine/*`. Engine-file location stays per app (`engineAsset.ts` in web/,
   `ti4.wasm` in web2) — the Worker gets a `WebAssembly.Module`, so no asset coupling.
2. **Move the socket.** `web2/src/session/socket.ts` → `web-shared/src/net/socket.ts`; web2's
   `wsTransport.ts` and `table.ts` import it. (web/'s `GameSessionClient` keeps its own reconnect for
   now; adopting `connect` is an optional follow-up, it has HTTP resume logic tied to it.)
3. **`GameLink`.**
   ```ts
   interface GameLink {
     send(message: object): void;                 // ClientMessage or a local host command
     request?(command: object): Promise<unknown>; // engine only: save_state, history, queries
     subscribe(listener: (e: LinkEvent) => void): () => void;
     close(): void;
   }
   type LinkEvent = { kind: "message"; message: unknown } | { kind: "opened" } | { kind: "offline" }
     | { kind: "progress"; done: number; total: number } | { kind: "save"; save: HostSave }
     | { kind: "fatal"; message: string };
   ```
   `socketLink(open, …)` wraps `connect` (subscription on `opened`, heartbeat, backoff).
   `engineLink(port, config)` wraps a `HostPort`: `start`, turns `ask`/`reply` envelopes'
   `messages` into `message` events, `save` at each ask, `crash`/`fatal` → `fatal`. Logic lifted
   from `LocalTransport` (`web/src/offline/local/LocalTransport.ts`, its port/waiter handling).
   Tests: a fake `Socket` and a fake `HostPort` (reuse `web/src/offline/local/testKit.ts` ideas).
4. **web/ `LocalTransport` on `engineLink`.** It keeps `GameTransport`, `MessageDrivenState`,
   batch/history/divergence handling; only the port plumbing goes. `LocalTransport.test.ts`,
   `contract.test.ts`, `launcher.test.ts` stay green.
5. **web2 single player on `engineLink`** = dedup plan Step 5 (do its Rust part 5.1 first:
   `HostSession` local commands for humans mask/seat switch, rule seats, stepping, wait,
   scenarios, draft/plan). Then `wsTransport.ts` becomes `messageTransport(link, …)` used by both
   modes: online = `socketLink`, single player = `engineLink` + capabilities (`stepping`,
   `switchSeat`, `wait`, `scenarios`, `save`) mapped to local commands. Saves switch to
   `HostSave` (old web2 saves break — owner already accepted). Delete `wasmTransport.ts`'s game
   loop and the bespoke exports (dedup 5.3); keep `ti4_setup_*`, `ti4_catalog`, `ti4_space_odds`
   (setup/odds are stateless queries — via `Host.query` or kept as today). The engine moves to the
   Worker, so the page's main thread no longer runs the game.
6. **Docs.** `web2/AGENTS.md` topics row; `web2/docs/agents/local-play.md` (Transport, saved game
   format, `ti4_journal`/stepping wording) and `online.md` (socket path); `web/src/offline/README.md`;
   `crates/ti4-wasm/README.md` (one host, two consumers).

Steps 1–4 are independent of the Rust work and can land first; step 5 depends on dedup 5.1.

## Verification

- Each step: web2 `npm run typecheck`, `npm run lint`, `npm test`; web `npx tsc --noEmit`, `npm test`.
- Steps 1/4: `npm run test:e2e:offline` in web/ (offline game start, resume from save).
- Step 2: web2 `npm run e2e -- -g online` with `cargo b-web && target/debug/server`.
- Step 5: `scripts/build-wasm.sh`; Rust gates per `web2/AGENTS.md` (`cargo fmt --all -- --check`,
  `cargo l-web`, `cargo t-web`, one at a time); web2 `transportContract.test.ts` runs against both
  links; full `npm run e2e` in web2; in the browser: `?single` table, hot-seat "Play as", undo,
  draft + Apply, reload from save, `?scenario=…`; and `?online&fill=random` still plays.
