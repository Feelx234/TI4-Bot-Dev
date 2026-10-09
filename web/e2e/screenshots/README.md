# Screenshot artifacts

Reproducible screenshots of UI states, one folder per artifact, one capture script per screenshot.

    cd web
    npm run screenshots -- all     # every folder
    npm run screenshots -- D       # one folder (id or full name)
    npm run screenshots -- AB      # two-letter ids work the same way

## Folder ids

Folders are named `<ID>-<short-name>` and the ids are sequential: `A`, `B`, ... `Z`, then `AA`, `AB`, ... `AZ`, `BA`, and so on. A new folder takes the next free id, which is the one after the highest id that exists on any branch you are going to merge with. Check `git branch -a` (or `git ls-tree -r --name-only <branch> web/e2e/screenshots | grep -E '^web/e2e/screenshots/[A-Z]+-'`) before picking, so two branches do not take the same id. If two branches clash anyway, rename one folder when merging (the id only has to be unique; nothing else refers to it). The runner matches folders by `^[A-Z]{1,2}-`, so ids go up to `ZZ`; extend the pattern in `_shared/run.mjs` if that is ever not enough.

Each folder holds `capture-*.ts` (Playwright specs), `manifest.json` (title, description, one caption per shot), `out/*.png` and a generated, self-contained `index.html` (ignored by git; rebuilt by the runner).

Shared parts in `_shared/`:

| File | Purpose |
|---|---|
| `mockGame.ts` | `openMockedGame(page, { choice, board, players, events, ... })` renders the real app at `/games/<id>` against routed HTTP and websocket, no backend |
| `fixtures.ts` | gallery board and players, activation options, completed combat, event log helpers |
| `players.ts` | the viewing player with a private hand, and the opponent |
| `shot.ts` | `shot()` writes `<folder>/out/<name>.png` with animations off |
| `hover.ts` | `hoverShot()` hovers, waits for the tooltip and crops around element and tooltip |
| `tooltip.ts` | draws native `title` tooltips, which browsers do not render into screenshots |
| `eventLog.ts` | opens and expands the event log |
| `build-artifact.mjs`, `artifact.tpl.html` | manifest + PNGs to a self-contained page |
| `mockResumable.ts` | `openResumableGame()`: a mocked game whose server can go away and come back (HTTP aborts, websocket closes, status codes, counters) and `sleepAndWake()`; used with `page.clock.install()` for resume-after-idle |
| `mapLobby.ts`, `vite.shots.config.ts` | `openMapLobby()` renders the real lobby and MapPicker against routed HTTP; the watcher-free vite config |
| `run.mjs` | the `npm run screenshots` runner |

Vite runs without its file watcher (`_shared/vite.shots.config.ts`), so a low inotify limit (ENOSPC) does not break the runner.

Fixtures are synthetic, not captured engine states. Playwright starts and stops vite itself (`playwright.config.ts`); set `TI4_SHOT_PORT` to pin the port.
