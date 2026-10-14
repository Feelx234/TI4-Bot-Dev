# Offline play

Status: operator and user documentation for the offline single-player mode. Parts marked
"(as designed)" describe the plan in `OFFLINE_SINGLEPLAYER_PLAN.md` and depend on the web launcher
(T5) that was still in progress when this was written; check them against `web/src/offline/`.

## What it is

"Play offline" starts a game against rule bots entirely in the browser. The rules engine is
compiled to WebAssembly (`crates/ti4-wasm`, built on the shared host crate `ti4-host`) and runs in
a module web worker. After the first load there is no server request: you can switch the network
off. Games are stored in the browser (IndexedDB).

* One human, 3 to 6 players, every other seat is a rule bot (`ti4-rule-bot`). No ML bots.
* Same engine and same rules as the server; a game is fully determined by its seed and your answers.
* The server is needed only to load the page the first time and to deliver feedback.

## Browser requirement

Chrome or Edge **137 or newer** (desktop and Android). The engine blocks while it waits for your
decision and the browser suspends it with WebAssembly JSPI (JavaScript Promise Integration),
which shipped in Chrome 137. There is no fallback in v1: Firefox and Safari (all iOS browsers
are Safari underneath) cannot play offline and the page says so instead of failing later. Online
games work in every browser as before. No special server headers are needed (no
COOP/COEP, no SharedArrayBuffer).

## Using it

1. Start page, "Play offline". Choose the player count, your seat and faction (or random) and a
   nickname. The first start downloads the engine (about 1 MB with gzip, 0.8 MB with brotli,
   4.5 to 6 MB unpacked) and compiles it; a progress bar shows this. Later starts use the browser cache.
2. Play as in an online game. Every accepted answer is written to the browser before the next
   step, so closing the tab loses nothing.
3. "My offline games" lists the saved games with round, phase and an estimated resume time.
   Resuming replays all your answers through the engine, which takes up to about 14 s late in a
   long game; a progress bar shows it and the tab stays responsive.

## Saves, export and import

* A save is `{seed, answers, engine hash, setup}`, not a snapshot. Size is small (about 60 KiB at
  5000 answers).
* **Export** writes a JSON file with the seed, the answers and the engine hash. Keep it as a
  backup, move a game to another browser or send it with a bug report.
* **Import** reads such a file back into the list (as designed). It resumes when the engine hash
  matches; otherwise see the update policy below.
* Clearing site data, a private window or a browser reset deletes the saves. If the browser
  refuses to store (private window, quota) the game keeps running from memory and says so; export
  before closing.
* Offline games are separate from online "My games"; nothing is migrated between them.

## Feedback

The Feedback button works offline. The report is put in a queue in the browser and sent to the
real server in the background when the browser is online; the game keeps going and is never
blocked. A queued report carries everything the server needs for a normal feedback file (category,
text, the usual context) plus a `local` block (`gameId`, `seed`, `answers`, `engineHash`,
`playerCount`, `humanSeat`), so triage can replay the offline game from the report. A failed send
is retried; the queue shows queued, sending, sent, failed. Operators read these like any other
report (`tools/public/list_feedback.sh`, `docs/FEEDBACK_BUTTON_NOTES.md`).

## What is disabled offline

Everything that needs a server or other people: lobby, invitations, other seats, spectators,
presence, takeovers, the access password. Also hidden in v1: the map splice preview, turn redo
(undo and rewind stay), the advisor (win odds, combat odds), the server-format replay export.
The UI hides these through the transport capabilities rather than showing broken buttons. Known
engine gaps apply (7 and 8 players are not offered; Creuss and Antimass limits as in the engine).

## Size and performance

* wasm: about 4.5 MB raw (`--profile wasm-release`, `opt-level = "z"`, no `wasm-opt`), CI budget
  about 7 MB. It is loaded only after "Play offline" (dynamic import), never for online games.
* The server hands out `ti4_host-<hash>.wasm` with `Content-Type: application/wasm`, a one-year
  `immutable` cache and, when the release has the files, `.br` and `.gz` variants chosen by
  `Accept-Encoding` (deploy.sh creates them). Caddy compresses what is left (zstd, gzip).
* A step takes 3 to 6 ms of engine time in Node; a full late-game resume is the slow case.

## Update and engine-hash policy

The engine hash names the exact wasm build a save was made with. Determinism holds per build: a
changed engine can answer the same question differently and make an old answer list diverge.

* Same hash: resume silently.
* Different hash: the app asks before replaying (nothing is replayed or overwritten without a
  yes). If the replay diverges the game is kept untouched and can be exported. Old wasm builds
  are not kept in v1; export a game you care about before you let an update through.
* Hash-named files mean an update never serves a mixed old page with a new engine: the page
  names its wasm by hash and the old file stays cached until the page itself changes.
* Operators: deploy updates normally (see `deploy/public/README.md`, "How to deploy safely").
  Tell testers in the alpha notice when the engine changed, since their offline saves are
  affected even though no server data is.

## Operator notes

* CSP (set in `deploy/public/Caddyfile`): `script-src 'self' 'wasm-unsafe-eval'` (needed for
  `WebAssembly.compile`; plain `unsafe-eval` stays off) and `worker-src 'self'`. The worker is a
  same-origin file, so `blob:` workers are not allowed.
* No COOP/COEP headers are required; do not add them, they would break third-party embeds and the
  Cloudflare layer for no benefit.
* Check headers after any proxy change: `deploy/public/check-headers.sh` starts the server and a
  throwaway Caddy with the repo Caddyfile on loopback and asserts content type, cache, br/gzip,
  CSP and the absence of COOP/COEP.

## Troubleshooting

| Symptom | Likely cause | What to do |
|---|---|---|
| "Play offline" says the browser is not supported | not Chrome/Edge 137+ | update the browser or use an online game |
| Stuck on "Loading engine", console shows a CSP error naming `wasm-eval` | proxy CSP lacks `'wasm-unsafe-eval'` | fix the Caddyfile `Content-Security-Policy`, run `check-headers.sh` |
| Worker fails to start, CSP error naming `worker-src` | CSP lacks `worker-src 'self'` | same |
| `WebAssembly.compileStreaming` fails with a MIME error | wasm served as `text/html` (SPA fallback for a missing file) or `application/octet-stream` | check the file exists in `dist/assets` and the type is `application/wasm` |
| Huge download every time | proxy strips `Cache-Control`, or the file is not under `/assets/` | headers on `/assets/*.wasm` must be `immutable` |
| Game list is empty after a browser update | site data cleared | restore from an exported file |
| "Engine changed" prompt on an old save | the app was updated | export first, then accept; see the policy above |
| Feedback stays "queued" | browser offline, or server unreachable or rate limited (10 per hour) | it retries by itself; "retry" forces it |
| Resume takes very long | late game, many answers | expected up to about 14 s on a desktop, longer on a phone |
