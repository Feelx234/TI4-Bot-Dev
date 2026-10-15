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
  `paths` of each app's `tsconfig.json`.
- In web2 only `src/session/` imports it. `web2/src/boundaries.test.ts` checks that, and that
  nothing here imports an app or a package other than `node:` and `vitest`.
- A Worker is started with a relative `new URL(".../web-shared/src/engine/worker.ts",
  import.meta.url)`: vite bundles a Worker only from a literal path, not from the alias.

## Layout

```
src/engine/  host.ts      Host: one wasm instance plays one game (JSPI), Envelope, HostSave, StartConfig
             jspi.ts      hasJspi(), on its own so that a page checks it without loading the host
             runner.ts    HostRunner: one game at a time in the Worker, a save at each question
             worker.ts    the Worker entry; it gets a compiled WebAssembly.Module, not the file
             protocol.ts  ToWorker, FromWorker, HostPort (the Worker or a fake)
             identity.ts  sha256Hex/sha256 of the engine file, EngineIdentity {build, contentDigest, wasmSha256}
src/net/     socket.ts    Socket, openSocket, pageSocket, connect (reopens a lost socket), backoff, HEARTBEAT_MS
src/link/    link.ts       GameLink (send, request?, subscribe, close), LinkEvent; a game at the level of its messages
             socketLink.ts socketLink: the server over `connect` (subscription on each open, heartbeat, backoff)
             engineLink.ts engineLink: the engine over a HostPort (start, replies, the next question, a gate, saves);
                           web's `LocalTransport` runs on it
```

The engine file stays where each app keeps it (`web/src/offline/local/engineAsset.ts`,
`web2/src/session/ti4.wasm`).

## Tests

- The tests here (`src/**/*.test.ts`) run in web2's vitest, in its `unit` project (node, no DOM):
  `cd web2 && npm test`.
- web2's biome lints this dir: `web-shared/biome.json` extends `web2/biome.json`, and web2's
  `npm run lint` runs it here too.
- web type-checks this dir with its own `tsconfig.json` (`npx tsc --noEmit`); its vitest does not
  run the tests here.
- web's `npm run test:e2e:offline` plays a game through the Worker of `src/engine/`.
