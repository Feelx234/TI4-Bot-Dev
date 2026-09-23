# Twilight Imperium 4 — Web Client

A minimal browser client and test suite for the authoritative `ti4-server` engine, built with React 19, TypeScript, Vite, Vitest, and Playwright.

---

## Quick Start (Interactive Play)

To test the client interactively across multiple players in your browser:

### 1. Start the Rust Backend Server

In a terminal from the repository root:

```bash
cargo run -p ti4-server --bin server
```

* Starts the HTTP and WebSocket authoritative server on `http://127.0.0.1:8080`.
* Start on the home page to create a lobby with 2–8 positions. Share the
  resulting game URL to invite other players or spectators.

### 2. Start the Frontend Dev Server

In a second terminal:

```bash
cd web
npm install
npm run dev
```

* Starts Vite at `http://127.0.0.1:3000`.
* API requests (`/api/*`) and WebSocket connections (`/ws/*`) are automatically proxied to `http://127.0.0.1:8080`.

### 3. Open in Browser

1. Navigate to [http://127.0.0.1:3000](http://127.0.0.1:3000).
2. Choose the player count and create a lobby. The creator becomes its host.
3. Copy the game URL and open it in another browser context to **Join game**
   (next open position) or **Watch** without claiming a position. A disconnected
   participant can be explicitly taken over from another computer after the
   presence grace period.
4. Players mark themselves ready; the host may reorder the lobby and start when
   every position is occupied and ready. Session credentials stay in tab-scoped
   storage, never in invitation URLs. Reconnect in the same tab to resume.
5. Choices and private cards are hidden from opponents and spectators.

---

## Testing

### Unit & Protocol Conformance Tests (Vitest)

Runs wire fixture validations, React component tests, and invariant checks:

```bash
npm test
```

### End-to-End Invariant Tests (Playwright)

Runs real-server end-to-end tests in isolated browser contexts verifying:
* Zero console errors / JavaScript runtime exceptions.
* Live strategy card draft synchronization across multiple tabs.
* Strict DOM privacy redaction (opponents/spectators never receive private card DOM nodes).
* Actionability and decision-making invariants.
* Disconnect and clean reconnection to a live game session.

```bash
# Headless run:
npx playwright test

# Headed run (visible browser windows):
npx playwright test --headed

# Server startup diagnostics:
DEBUG=pw:webserver npx playwright test --reporter=line
```

---

## Project Structure

```text
web/
├── e2e/
│   ├── lobby_lifecycle.spec.ts       # Create, join, watch, leave, and start
│   └── multiplayer_invariants.spec.ts # Playwright multiplayer invariant suite
├── src/
│   ├── components/
│   │   ├── Board.tsx                   # Interactive SVG galaxy board with planets & units
│   │   ├── TurnStatusBar.tsx           # Round, phase, speaker, and live turn status bar
│   │   ├── PlayerSheet.tsx             # Player resources, VP, tokens, and private hand
│   │   ├── PendingChoiceModal.tsx      # Accessible dialog for player decisions
│   │   ├── EventLog.tsx                # Collapsible event log drawer
│   │   └── Lobby.tsx                   # Admission, readiness and host reorder
│   ├── hooks/
│   │   └── useGameSession.ts           # WebSocket connection hook with state sync & heartbeat
│   ├── protocol/
│   │   └── types.ts                    # TypeScript wire DTOs matching ti4-server Rust protocol
│   ├── test/
│   │   └── invariants.test.ts          # State integrity and privacy invariant checks
│   ├── App.tsx                         # Main app container
│   └── main.tsx                        # Entry point
├── playwright.config.ts                # Playwright configuration with auto-spawning dev servers
└── vite.config.ts                      # Vite configuration with proxy to ti4-server
```
