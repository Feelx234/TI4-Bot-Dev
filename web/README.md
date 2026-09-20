# Twilight Imperium 4 — Web Client

A minimal browser client and test suite for the authoritative `ti4-server` engine, built with React 19, TypeScript, Vite, Vitest, and Playwright.

---

## Quick Start (Interactive Play)

To test the client interactively across multiple seats in your browser:

### 1. Start the Rust Backend Server

In a terminal from the repository root:

```bash
cargo run -p ti4-server --bin server
```

* Starts the HTTP and WebSocket authoritative server on `http://127.0.0.1:8080`.
* Pre-seeds a default game named `demo` with 3 seats:
  * Seat `p1`: Human player
  * Seat `p2`: Human player
  * Seat `p3`: Automated Bot player

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
2. Enter Game ID: `demo`.
3. Select Seat: `p1` and click **"Join Game"**.
4. To test multi-player synchronization and privacy redaction, open a second browser window or tab:
   * Navigate to [http://127.0.0.1:3000](http://127.0.0.1:3000).
   * Enter Game ID: `demo` and select Seat `p2` (or choose `Spectator`).
5. Notice that choices and private cards are strictly hidden from opponents and spectators. When Player 1 selects a strategy card, Player 2's screen updates live to show remaining choices.

---

## Testing

### Unit & Protocol Conformance Tests (Vitest)

Runs wire fixture validations, React component tests, and invariant checks:

```bash
npm test
```

### End-to-End Invariant Tests (Playwright)

Runs multi-seat end-to-end tests spawning 3 isolated browser contexts (Player 1, Player 2, Spectator) verifying:
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
```

---

## Project Structure

```text
web/
├── e2e/
│   └── multiplayer_invariants.spec.ts  # Playwright multi-seat browser invariant suite
├── src/
│   ├── components/
│   │   ├── Board.tsx                   # Interactive SVG galaxy board with planets & units
│   │   ├── TurnStatusBar.tsx           # Round, phase, speaker, and live turn status bar
│   │   ├── PlayerSheet.tsx             # Player resources, VP, tokens, and private hand
│   │   ├── PendingChoiceModal.tsx      # Accessible dialog for player decisions
│   │   ├── EventLog.tsx                # Collapsible event log drawer
│   │   └── Lobby.tsx                   # Game join / seat selection lobby
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
