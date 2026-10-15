# web-shared: rules for agents

The code that `web/` and `web2/` share: what lies below each app's UI-facing transport. The plan
is `web2/plans/2026-10-14-SHARED_TRANSPORT_PLAN.md`.

## Ownership and changes

- Both maintainers own `web-shared/`. An app imports only its public modules.
- Some changes need the agreement of both maintainers:
  - a stored format: the save, the DB schema and its migrations, or the export file;
  - the shape of `GameLink`/`LinkEvent`, or of the generated wire types (that is, a protocol
    change in Rust);
  - removing or renaming an export;
  - a new dependency.
- Other changes need no agreement: additive changes, fixes and tests, as long as the gates of
  both apps are green.
  - web2: `npm run typecheck`, `npm run lint`, `npm test`.
  - web: `npx tsc --noEmit`, `npm test`.
- Never in `web-shared/`: React, CSS, view models, text shown to a player, or the state model
  of an app.
- The code follows both apps' toolchains:
  - explicit `.ts` import extensions;
  - ES2022;
  - no vitest globals;
  - no `import.meta.glob` and no `vite/client` types.

## How the apps use it

- An app imports `@ti4/shared/<dir>/<file>.ts`. The alias is in each app's `vite.config.ts`
  (`resolve.alias`, and `..` in `server.fs.allow`) and `tsconfig.json` (`paths`, and
  `../web-shared/src` in `include`).
- `web-shared/` has no `package.json` and no `node_modules`. `vitest` resolves through the
  `paths` of each app's `tsconfig.json`, and `fake-indexeddb` (a dev dependency of web2 only)
  through web2's.
- In web2 only `src/session/` imports it. `web2/src/boundaries.test.ts` checks that, and that
  nothing here imports an app or a package other than `node:` and `vitest` (and
  `fake-indexeddb` in a `*.test.ts`).
- A Worker is started with a relative `new URL(".../web-shared/src/engine/worker.ts",
  import.meta.url)`: vite bundles a Worker only from a literal path, not from the alias.

## Layout

```
src/engine/  host.ts      Host: one wasm instance plays one game (JSPI), Envelope, HostSave, StartConfig
             jspi.ts      hasJspi(), on its own so that a page checks it without loading the host
             runner.ts    HostRunner: one game at a time in the Worker, a save at each question; a restart
                          starts as the game was started, with the stepping and the modes changed since
             worker.ts    the Worker entry; it gets a compiled WebAssembly.Module, not the file
             protocol.ts  ToWorker, FromWorker, HostPort (the Worker or a fake)
             port.ts      workerPort (a HostPort on a Worker that the app made), inlinePort (the engine in
                          this thread, for Node and the tests)
             identity.ts  sha256Hex/sha256 of the engine file, EngineIdentity {build, contentDigest, wasmSha256}
src/net/     socket.ts    Socket, openSocket, pageSocket, connect (reopens a lost socket), backoff, HEARTBEAT_MS
src/link/    link.ts       GameLink (send, request?, subscribe, close), LinkEvent; a game at the level of its messages
             socketLink.ts socketLink: the server over `connect` (subscription on each open, heartbeat, backoff,
                           `again`); web2's `wsTransport` runs on it
             engineLink.ts engineLink: the engine over a HostPort (start, replies, the next question, a gate, saves);
                           web's `LocalTransport` and web2's local game (`wasmTransport`) run on it
src/storage/ types.ts     HostSave (format 2), SavedGameRecord, GameMeta, StoreStatus, StoreError, LIMITS,
                          OfflineSaveStore, FeedbackBackend
             backend.ts   Backend: IdbBackend (DB `ti4-offline`, DB_VERSION, MIGRATIONS), MemoryBackend
             saveStore.ts openOfflineSaveStore: the games, a memory fallback, import and export
             autosave.ts  createAutosave: debounced writes, flushed on pagehide and a hidden tab
             validate.ts  validateSave, the record and the export file (EXPORT_KIND, EXPORT_VERSION),
                          engineDifferences
             index.ts     the public exports
src/wire/    index.ts     the wire types: import them from here
             generated/   one file for each type, and index.ts with every type and PROTOCOL_VERSION; written
                          from Rust by `cargo wire-web`, never by hand
```

The engine file stays where each app keeps it (`web/src/offline/local/engineAsset.ts`,
`web2/src/session/ti4.wasm`).

## Wire types

The types of what the server and the engine send and take, generated from the Rust types with
`ts-rs` (`crates/ti4-wasm/src/wire_ts.rs`). Import from `@ti4/shared/wire/index.ts`.

- The roots: `ClientMessage` and `ServerMessage` (`ti4_host::protocol`, with the views, the
  status, the staged work and the planning wire), `Catalog`, the setup of a table
  (`SetupOptions` and `MapPreview` as the server gives them, `TableOptions`, `TablePreview`,
  `TableDraw`, `GameRequest`), the odds of a space combat (`SpaceOddsRequest`, `BattleResponse`)
  and the save (`HostSave`). Every type that they name is generated with them.
  `PROTOCOL_VERSION` is generated too. The types of `ti4-server` (the lobby of a table) are not.
- The doc comments are those of the Rust types: a comment for a page is written there. A field
  that Rust leaves out when it is empty is `name?: T`. A field that a page may leave out of what
  it sends (serde `default`) is `name?: T` with `#[cfg_attr(feature = "ts", ts(optional))]` on an
  `Option`, and `ts(optional = nullable)` on another type (`TableSetup`, `SeatRequest`, `scope`
  of `run_draft`). `u64` is `number`. The engine's `GameState` is `Record<string, unknown>`.
- web2 imports them through `web2/src/session/wire.ts`, which re-exports them and adds only what
  web2 narrows. web has its own types (`web/src/protocol/types.ts`).
- A change of a Rust type of the wire: run `cargo wire-web` (it writes `src/wire/generated/`
  and removes the files of types that are gone) and commit the files. `cargo t-web` fails while
  they are stale (`wire_types::generated_wire_types_are_current`).
- The derives are behind the `ts` feature of `ti4-model`, `ti4-engine`, `ti4-calc`, `ti4-host`
  and `ti4-wasm`. Only the tests enable it (the dev-dependency of `ti4-server` on `ti4-wasm`), so
  the wasm file and the server binary of a deploy have none of it. A new type that a root names
  gets `#[cfg_attr(feature = "ts", derive(ts_rs::TS))]`; a field that cannot be typed gets
  `#[cfg_attr(feature = "ts", ts(type = "..."))]`. Two types with one name fail the generator:
  rename one with `ts(rename = "...")`.
- The generated files are not linted or formatted (`biome.json`, `.oxfmtrc.json`).

## Save store

The single player games of both apps, in one IndexedDB database of the origin. Import from
`@ti4/shared/storage/index.ts`.

- `await openOfflineSaveStore()` never throws for a storage problem. Without IndexedDB (a
  private window), with a database newer than the build, or when a write fails, it keeps the
  games in memory and says why in `status().warningKind` (`unavailable`, `newer-version`,
  `quota`, `write-failed`). The app shows a visible notice with its own text.
- `put(id, save, meta)` resolves when the write committed; call it before the game goes on
  (write-ahead). `createAutosave(store).schedule(...)` is the debounced path (300 ms, at most
  2 s, the newest save wins).
- `meta = {nickname, factions, engine, slot?, createdAt?, resumeEstimateMs?}`. `factions` has one
  entry per seat, null where the app does not know it. `engine` is `{build, contentDigest,
  wasmSha256}`: the first two from `save.engine`, the hash of the wasm file that the page loaded.
  A later write keeps `createdAt`, `resumeEstimateMs` and `slot` when it does not give them.
- `list()` (newest first, no save), `load(id)`, `findSlot(slot)` (the newest game with the slot),
  `remove(id)`, `removeUnreadable()` (deletes the records that are not read: the saves of an
  older version), `exportGame(id)`, `importGame(file, currentEngine?)` (always a fresh id, and
  no slot; `engineDiffers` says whether the file's engine is another one; later writes keep
  `imported`).
- `createAutosave` flushes on `pagehide`, but a write of IndexedDB that starts there does not
  end when the page is gone (a reload, a navigation). An app that must not lose the last save
  keeps it at once elsewhere: web2 puts it into localStorage and into the store on the next
  load (the saved game in `web2/docs/agents/local-play.md`).
- web2 keeps its local games and its single player game here (`web2/src/session/saves.ts`), with
  the slots below; web/ its offline games.
- **The record** (`SavedGameRecord`, schema 2): `id`, `slot?`, `nickname`, `createdAt`,
  `updatedAt`, `status`, `summary {playerCount, humans, factions, seed, round, phase, answers}`,
  `engine`, `resumeEstimateMs`, `imported`, `sizeBytes`, `save`.
- **The slot** is the fixed address of a game in an app: `"single"`,
  `"local.<seed>.<players>.<mask>"`, `"scenario.<id>.<mask>"`. A game found by its id only has
  none.
- **The save** is the engine's `HostSave`, format 2 (`crates/ti4-wasm/README.md`). `validateSave`
  checks what the store and the lists rely on (2 to 8 players, the humans mask, every seat's
  decisions, the limits); the engine checks the rest when it resumes. A record that fails
  (an older save format or record schema, a broken row) is not shown and is counted in
  `status().unreadable`.
- **The export file** is the one export format of both apps: `{kind: "ti4-offline-game",
  version: 2, exportedAt, game: {nickname, factions, createdAt, engine, save}}`.
- **The schema.** Database `ti4-offline`, version 2: `games` (key `id`, indexes `updatedAt` and
  `slot`), `feedback` (key `localId`), `meta`. A change adds a step to `MIGRATIONS` and bumps
  `DB_VERSION`; an old step is never edited. A database newer than the build is not touched (the
  memory fallback).
- **The limits** (`LIMITS`): 8 MiB a save, 100 games, 128 MiB in all, 20000 answers, 200000
  decisions, a nickname of 40 characters.
- **Feedback.** The object store `feedback` belongs to web's feedback queue
  (`web/src/offline/storage/feedbackQueue.ts`), through `store.feedbackBackend`. It is part of
  this schema because it is one database; the queue's code and its rows are web's.

## Tests

- The tests here (`src/**/*.test.ts`) run in web2's vitest, in its `unit` project (node, no DOM):
  `cd web2 && npm test`.
- web2's biome lints this dir: `web-shared/biome.json` extends `web2/biome.json`, and web2's
  `npm run lint` runs it here too. Not `src/wire/generated/`.
- `cargo t-web` checks that `src/wire/generated/` is current.
- The storage tests (`src/storage/storage.test.ts`) run on `fake-indexeddb`, a fresh
  `IDBFactory` for each test.
- web type-checks this dir with its own `tsconfig.json` (`npx tsc --noEmit`), without the
  `*.test.ts` files; its vitest does not run the tests here.
- web's `npm run test:e2e:offline` and web2's `npm run e2e` play games through the Worker of
  `src/engine/`. web2's `transportContract.test.ts` checks its transport over both links (the
  engine on `inlinePort`, and a real server).
