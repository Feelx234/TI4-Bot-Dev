# Screenshot artifacts

Reproducible screenshots of UI states, one folder per artifact (`A-` to `Z-`, then two-letter ids such as `AI-`), one capture script per screenshot.

    cd web
    npm run screenshots -- all     # every folder
    npm run screenshots -- D       # one folder (letter or full name)

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
| `mapLobby.ts`, `vite.shots.config.ts` | `openMapLobby()` renders the real lobby and MapPicker against routed HTTP; the watcher-free vite config |
| `run.mjs` | the `npm run screenshots` runner |

Vite runs without its file watcher (`_shared/vite.shots.config.ts`), so a low inotify limit (ENOSPC) does not break the runner.

Fixtures are synthetic, not captured engine states. Playwright starts and stops vite itself (`playwright.config.ts`); set `TI4_SHOT_PORT` to pin the port.
