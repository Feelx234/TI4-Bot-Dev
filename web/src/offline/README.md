# Offline UI shell (T2)

Everything the user sees around an offline (local, one human vs rule bots) game. It is built against
`OfflineLauncher` in `types.ts`, not against any transport, so it does not depend on T1's
`GameTransport`. A dev-only in-memory launcher (`mockLauncher.ts`, registered by `devMock.ts` when the
page sets `window.__TI4_OFFLINE_MOCK__` or the URL has `?offline-mock`) stands in until T5.

## What T5 must plug in

1. Implement `OfflineLauncher` and call `registerOfflineLauncher(launcher)` at startup. With nothing
   registered the "Play offline" button is absent and `/offline` shows "not available".
   - `capabilities`: use `OFFLINE_CAPABILITIES` (splice preview, turn redo, advisor, multiplayer, instant
     feedback all off). T1's transport should report the same flags; the banner and `useCapability`
     read them.
   - `supported()`: false with a reason when JSPI is missing (Chrome/Edge 137+ only in v1).
   - `list()` (`updatedAt`/`createdAt` are unix seconds; `resumeEstimateMs` from the last measured
     replay), `create`, `resume` (call `onProgress` with `loading-engine`, then `replaying` with
     `done`/`total` as often as is cheap; honour the `AbortSignal` by rejecting), `remove`,
     `exportGame` (seed, answers, engine hash as JSON).
   - `feedback`: an `OfflineFeedbackQueue`. `list()` must return a stable array reference until the
     content changes (it feeds `useSyncExternalStore`); notify subscribers on every state change.
     The queue persists (IndexedDB), sends in the background (`POST /api/feedback` with
     `{category, text, context}`, plus `local` for triage), retries with backoff, and never blocks the game.
2. Game screen: render the real game from `renderGame` of `OfflineRoutes` (`/offline/games/<id>`,
   currently a placeholder) inside T1's transport provider and `CapabilityProvider`, and show the banner
   (`OfflineGameRoute` passes it as the second argument).
3. While a game is on screen call `setActiveOfflineGame({ gameId, local })` (and `null` on leave). The
   feedback sheet then routes through `routeFeedback`: the report goes to `launcher.feedback.enqueue`
   with `local()` (seed, the human's answers, engine hash, table size, seat) and the sheet says
   "Will be sent". Outside an offline game feedback is unchanged.
4. Hide degraded features with `useCapability("turnRedo" | "splicePreview" | "advisor" | "multiplayer")`
   or `<IfCapable name=...>`. The shell does not touch GameShell, the redo hook, the advisor or the map
   picker; T5 wires those flags in.
5. Server side: the feedback endpoint must accept `context.local` (triage replays from it).

Not done here: import of an exported file, switching an offline game to online, the setup's map and
strategy-card-set choices (the setup form has only players, seat, faction, nickname, dev seed).
