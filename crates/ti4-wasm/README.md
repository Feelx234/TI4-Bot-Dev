# ti4-wasm

The rules engine compiled to WebAssembly, for an offline hotseat mode in `web2/`.

It began as a spike (2026-10-09) that answered three questions: how large the wasm file is, what
could make it smaller, and whether the engine's blocking decisions work in a browser. `web2` now
plays a game through it (see "Local play in web2").

## Result

- The engine compiles to `wasm32-unknown-unknown` with one change: `rand` without default
  features in `ti4-engine` (`getrandom` has no backend for this target).
- The file is **4.30 MB, 0.80 MB with brotli**, with the projections of `ti4-view` included.
- A seeded game of three to eight players gives the same state in wasm and natively.
- A decision of a player can suspend the engine and wait for a promise (JSPI). The engine needs
  no change for it.
- The host gets what a client of the server gets: the redacted view, the pending choice and the
  turn status (`SessionUpdate` from `ti4-view`, the shared part of the server's `StateUpdateMsg`).

## Size

The size of `ti4_wasm.wasm`; brotli at quality 11.

| Build | raw | gzip | brotli |
|---|---|---|---|
| `--release` (the workspace profile) | 6.19 MB | 1.89 MB | 1.23 MB |
| `--profile wasm-release` | 4.20 MB | 1.08 MB | 0.78 MB |
| `--profile wasm-release`, then `wasm-opt -Oz` | 3.49 MB | 1.14 MB | 0.83 MB |
| `--profile wasm-release`, with `ti4-view` | **4.30 MB** | 1.11 MB | **0.80 MB** |

- `wasm-release` is `opt-level = "z"`, `lto = "fat"`, `panic = "abort"` (workspace `Cargo.toml`).
- `wasm-opt` makes the file smaller but the download larger. Do not use it.
- The `--release` and `wasm-opt` rows were measured before the fixed buffers (see Memory); the
  `wasm-release` row was 4.22 MB then, so the rows compare.
- The projections cost 0.10 MB raw and 0.02 MB of download. The first three rows and the
  breakdown below are without them.
- With the map templates and their builder the file is 4.52 MB raw and 1.13 MB with gzip
  (brotli not measured). The table and the numbers above are from before that.
- With the draft (the planning checks of `ti4-view`, the request and draft buffers) the file is
  4.65 MB raw and 1.17 MB with gzip (2026-10-10; brotli not measured).
- Speed: 2000 steps take about 7–12 s with `wasm-release` and about 6 s with `--release`, in
  Node. That is 3–6 ms for a step.

### Where the bytes are

From `twiggy` on an unstripped `wasm-release` build.

| Part | Size | |
|---|---|---|
| Code | 3.1 MB | |
| – `ti4_engine` | 1.27 MB | the faction modules are 0.43 MB of it |
| – `alloc` and `core` | 1.52 MB | generic code, compiled once for each type that uses it |
| – `serde_json`, `serde_core` | 0.17 MB | all serde-related code together is 0.2–0.3 MB |
| – all other crates | 0.1 MB | `ti4_model`, `ti4_content`, `rand`, `sha2`, the allocator (8 KB) |
| Data | 1.13 MB | |
| – content JSON | 0.96 MB | `ti4-content/content/*.json`, embedded as written |
| – other strings | 0.17 MB | |

The 1.52 MB of `alloc` and `core` is mostly `Vec`, slices and sorting (about 0.6 MB), `BTreeMap`
and `BTreeSet` (about 0.5 MB) and iterator adapters (about 0.25 MB). The split is by function
name and is approximate.

## What can be cut

| Candidate | Saves raw | Saves brotli | Verdict |
|---|---|---|---|
| Third-party dependencies | 0 | 0 | Nothing to cut. No crate is larger than 125 KB. |
| Minify the content JSON | 270 KB | 4 KB | Cheap, small. |
| Also drop flavour text, authors, short names, emoji fields | 330 KB | 24 KB | Small. Not checked: which of these fields Rust reads. |
| Also drop the map templates | 420 KB | 28 KB | Small. |
| Engine modules that hotseat does not use | 0 | 0 | The linker already removes them. |
| `Debug` implementations | 7 KB | – | Not worth it. |
| Fixed data structures in the engine (next section) | up to 1.1 MB | not measured | Large, and a rewrite. |

- These crates are in the manifests of `ti4-model`, `ti4-content` or `ti4-engine` and are not
  used there: `anyhow`, `tracing`, `globset`, `itertools`, `bitflags`, `smallvec`, `hashbrown`,
  `indexmap`. They cost no wasm bytes. Removing them saves build time only.
- `opening`, `observation`, `fixtures`, `fingerprint` and `registry` link to 0 bytes. `synergy`,
  `preview` and `decision_context` are 8 KB together.

## Later: an engine with fixed data structures (non-goal now)

The largest part that is not rules is the generic container code: 1.1 MB of `Vec`, `BTreeMap`
and iterator code. An arena alone does not remove it, because a `Vec<T>` in an arena is still
one copy of `Vec` for each `T`. What removes it is data structures made for this game:

- ids as small integers, not strings (the guess is that string ids, cloned and used as map keys,
  are the first thing to change; this is not profiled);
- arrays indexed by those ids in place of maps;
- bit sets for sets of players, systems, technologies;
- lists of a fixed capacity where the game has a limit (units of a kind, cards in a hand).

With such a layout the whole game state is one block of a known size. It is allocated once at
the start, or from an arena, and the engine does not allocate while it plays.

Expected effects, none of them measured:

- Size: up to 1.1 MB raw. A few hundred KB of download is a fair guess.
- Speed: probably faster than today. The state is compact and stays in the cache, a copy of the
  state is one `memcpy`, and a step does not allocate.
- Debugging: the state has a fixed address and a fixed layout, and a snapshot is a copy of bytes.

This is a rewrite of the engine and is a non-goal until hotseat works. The rule for new code in
the meantime is below.

## Memory in this crate

New code uses data structures that fit the use, with fixed or arena memory, so that it does not
have to be migrated when the engine changes.

- Everything that crosses the boundary is in a fixed static buffer (`src/buffer.rs`): the result
  of an export (`RESPONSE_CAPACITY`, 256 KiB) and the update for the host (`UPDATE_CAPACITY`,
  512 KiB), the request of the host (`REQUEST_CAPACITY`, 64 KiB) and the outcome of a draft
  (`DRAFT_CAPACITY`, 1 MiB: two updates). A full game state is about 40 KiB. An update of a game of eight is 26 KiB at the
  start, and the largest of one full game (seed 3) was 42 KiB.
- A buffer never grows and never moves. The host reads every result at the same address.
- A value is serialized directly into its buffer. No intermediate `String` or `Value` is built.
- A value that does not fit fails the call at once. The error says what was too large, its size,
  the capacity, and the constant to raise:

  ```text
  ti4-wasm buffer full: the pending choice is 876 bytes, but PENDING_CAPACITY is 512 bytes.
  Raise PENDING_CAPACITY in crates/ti4-wasm/src/buffer.rs and rebuild, or make the pending
  choice smaller.
  ```

  The JS side throws it as an `Error`. A buffer is never cut short and never reallocated: the
  limits are there to be found.
- What still allocates in this crate is what the engine's interface asks for (owned ids, the
  boxed decider, the setup lists) and the text of an error. The engine itself allocates on every
  step, so the global allocator stays for now.
- The views allocate too. A decider is given no game state, so the run keeps a copy of the state
  and refreshes it after each step and at each choice that the engine offers with its position,
  and `ti4-view` builds a `GameView` of maps and vectors from it. Both are shaped by the engine's types and change with them. The view is then serialized
  directly into its buffer.
- The checkpoints for undo are a fixed number (`CHECKPOINTS`, 32), not a fixed size: each is a
  copy of the game made by the engine's `Game::fork`, on the heap. The oldest is dropped when
  a new one is made. This is not an error: the host then takes an answer back by a replay.
- Serializing goes through `dyn Write`. One copy of every serializer for each writer type cost
  39 KB.

## The boundary

Plain C ABI, no `wasm-bindgen`. The crate has its own `[lints]` because the boundary needs
`unsafe` attributes and the workspace forbids `unsafe`.

| Export | |
|---|---|
| `ti4_run_seeded(seed, players, max_steps) -> status` | Random deciders on all seats; the result is the state. |
| `ti4_play(seed, players, human_mask) -> status` | Plays a game to its end. A seat whose bit is set in the mask (bit 0 is seat `a`) is answered by the host through `host.ask`; the others decide at random. |
| `ti4_play_scenario(human_mask, id_len) -> status` | As `ti4_play`, from a prepared board. The request buffer holds the id of the scenario. See "Scenarios". |
| `ti4_scenarios() -> status` | The scenarios, as a JSON list in the response buffer. |
| `ti4_can_undo() -> 0 or 1` | While the game waits in a choice: whether the host may answer it with `-2`. |
| `ti4_set_stepping(0 or 1)` | Whether the host is asked before each decision of a random seat. See "Stepping". |
| `ti4_decided() -> count` | How many decisions the seats made, the random seats included. See "Switching seats". |
| `ti4_switch_seats(decision, human_mask)` | From the decision with this number on, the host answers the seats of the mask. See "Switching seats". |
| `ti4_request_ptr()` | Where the host writes a request (`REQUEST_CAPACITY`, 64 KiB) before the export that reads it. |
| `ti4_draft(request_len) -> status` | Runs a draft script on a copy of the game. See "A draft". |
| `ti4_draft_ptr()` | The address of the outcome of the last `ti4_draft` (`DRAFT_CAPACITY`, 1 MiB). |
| `ti4_response_ptr()` | The address of the result or the error of the last export. |
| `ti4_update_ptr()`, `ti4_update_len()` | The update for the host: while the game waits in a choice, and after `ti4_play` has returned. Length 0 when there is none. |

The result of `ti4_run_seeded` has the state and, as `view`, what seat `a` is shown of it.

A status of 0 or more is the length of a JSON result. A status below 0 is the negated length of
an error text.

The update is `{ viewer, view, pending_choice, turn_status }`, with the names and shapes of the
same fields of the server's `StateUpdateMsg`. A test of `ti4-server` keeps them equal
(`a_session_update_is_the_shared_part_of_a_state_update`). The viewer is the seat that is asked.
The nonce of a choice is its number in the game.

The import `host.ask() -> i32` returns the index of the chosen option. `-2` takes back the
answer before this choice, when `ti4_can_undo` returns 1: that choice is asked again, with the
same number and the same update. Any other negative answer leaves the game: every later choice fails without asking, the run unwinds, and `ti4_play` returns with
`stopped` set. A choice with one option is asked like any other, as on the server: the engine
itself skips the choices that need no answer.

### Stepping

With `ti4_set_stepping(1)` a random seat asks the host before it decides. The update is then the
view of the host's seat (the seat that was asked last, or its first seat) with no
`pending_choice`; `turn_status` names the seat that decides. The host answers `-3` (`GO`), and
the seat draws from the seeded stream as always. `-2` takes back the last answer of the host,
when `ti4_can_undo` returns 1. Any other answer leaves the game.

- A step is not a choice: it has no number, makes no checkpoint, and is not in a saved game. The
  game with steps is the same game.
- The export only sets a flag, so the host calls it at any time.

### Switching seats

The seats of the host can change during a game: the Dev sheet of web2 has "Play as". Every seat
has the same decider, which asks at each decision whether the host answers the seat now.

- A switch is `(decision, mask)`: from the decision with this number on, the host answers the
  seats of the mask. The number counts every decision of the game, also of the random seats.
- While the game waits (in a choice, or before a step), the host calls
  `ti4_switch_seats(ti4_decided(), mask)` and answers `-4`. The decision is asked again, of the
  seat's new decider: a choice of a seat that the host gave up is drawn from the seeded stream,
  and a seat that the host took is asked. A choice that was not answered keeps its number for
  the next choice of the host.
- The answers from before a switch are not taken back: `ti4_can_undo` is 0 at the first choice
  after it. A checkpoint keeps the count of the decisions, so an undo from a checkpoint that
  is older than the switch plays through it as the game did.
- A saved game names its switches. For a game that is played again, the host calls
  `ti4_switch_seats` for each of them before `ti4_play`. They end with that game.
- The view is of the seat that was asked last while the host still answers it, else of the
  first seat of the mask.
- The base of a draft is played with random seats only: a seat of the host may have been a
  random seat in the steps since the checkpoint, and those steps never asked the host.

## Decisions with JSPI

The engine asks for a decision with a blocking call (`Decider::choose`). The server blocks a
thread for it. A browser cannot block its main thread, so the host wraps the import in
`WebAssembly.Suspending` and the export in `WebAssembly.promising` (`js/jspi.mjs`). The wasm
stack is parked until the promise of the answer settles.

- No threads, no `SharedArrayBuffer`, no cross-origin isolation headers.
- The glue is about ten lines of JS, written by hand.
- While the game is parked, other exports of the same instance can be called. They must not
  touch the game, which the parked stack has borrowed. They read a copy that was stored when the
  choice was offered: this is what the update buffer is.
- Such a call must return before the game is resumed. Rust keeps a second stack in linear
  memory, and the calls share it.
- Undo cannot rewind a parked stack. It is a replay from the seed and the decision log.
- Checked in Chromium 156 only. Firefox and Safari are not checked. The fallback is a Web Worker
  that blocks in `Atomics.wait`; it needs cross-origin isolation, and a blocked worker cannot
  answer queries.

## Local play in web2

`web2/?local=<seed>&players=8&humans=<mask>` plays a game of this engine in the shell of web2.
`humans=1` is seat `a` against seven random seats. `humans` is a mask of the seats that are
played in the page.

- `web2/scripts/build-wasm.sh` builds the file and copies it to `web2/src/session/ti4.wasm`
  (not in the repository).
- `web2/src/session/` is the client side: `Transport` (updates in, `submitChoice` out),
  `wasmTransport.ts` (the JSPI glue), the selectors from an update to the view models of web2,
  and `useLiveSession`. A websocket transport for the server needs the same two things.
- The activation, the movement and the landings of a tactical action have their screens (see
  "A tactical action" below). Every other decision is one list in the action panel.
- `cargo run -p ti4-wasm --example record -- <seed> <players> <mask> <choice number> [option id ...]`
  writes the update of one choice. The option ids answer the first choices, so that the game
  reaches the choice that is wanted. The tests of web2 use five such files
  (`web2/src/session/fixtures/`).

### A tactical action

The engine asks for one ship at a time (`move|<system>|<index>`), then for each unit that the
ship loads, and it moves the ship before it asks for the next one. The screen of web2 stages the
whole movement first. Two things connect them.

**Facts.** For the choice of a system to activate, for the choice of a ship to move and for the
choice of a ground force to land, the update has one more field, `tactical`
(`ti4-view/src/tactical.rs`). It says what the choice itself does not: for the activation, how
many ships are in range of each system; for the movement, each ship of the seat with its path,
the gravity rifts on it, why it cannot move (a command token, or the range), and the units that
it can load, also on its way; for the landings (`kind: "invasion"`), the ground forces by kind
and place with their counts, and every planet of the active system with the forces that can
land there. The landing facts are read from the `commit` options of the choice itself. `ti4-wasm` fills
the field when it offers a choice to a seat of the page; the random seats do not pay for it.

- Nothing in `ti4-engine` was changed for this. Every fact is the answer of a public function
  of the engine, asked as `Game::begin_one_move` asks it. What a ship can load is read from the
  `load` options of the engine's own `CargoWindow`. A test plays a move and compares the two.
- The server does not send the field yet (`project_session_update` leaves it empty).

**A plan.** "Move fleet" sends the staged movement as steps that name what is moved and
loaded, not option ids (`web2/src/session/movementPlan.ts`). `Transport.submitPlan` takes them;
this is the contract of the server's batch (`ti4-server/src/session/batch.rs`), so the
websocket transport can post the same steps. The wasm transport answers each choice of the
engine with the option whose payload matches the next step, with the three rules of the
batch: a hold that closed itself needs no "done loading"; a hold that the plan does not load
is declined; a reaction window or a choice of another seat stops the plan, and the rest is
staged again when the movement goes on.

- A step that the engine does not offer takes the whole plan back, from the checkpoints. The
  server gets the same by playing the game again for each batch; here that would cost up to
  the whole engine time of the game.
- Each answer of a plan is an answer of the saved game, so the save, the replay and the
  checkpoints did not change.
- One undo takes a movement back whole: after an undo, the transport goes on while the choice
  that is open again is inside a movement (a hold, or a ship has already moved). It needs no
  stored groups, so it also holds after a reload.
- The transport keeps what each answer chose (the option with its payload, as `RecordedDecision`
  of the server), and a game that is played again fills that list again. The update after an
  undo tells the steps of the movement that were taken back (`undone`), and the page stages
  them again as the draft.

**The landings.** The engine asks for one ground force at a time (`commit|<n>|<planet>`) and
then for the end (`done_committing`). "Commit ground forces" sends the staged landings through
the same `submitPlan`, as steps `land` (planet, unit, damaged, and the place of a unit that is
not in the space area) and `done_committing`.

- When the last ground force has landed, the engine does not ask for the end. A plan whose
  only step left is `done_committing` is done at any other question.
- One undo takes the landings back whole: the transport goes on while the open choice is a
  landing and the answer before it landed a unit.
- The server's batch has no landing step yet.

Limits:

- The engine uses Gravity Drive only for a ship that cannot arrive without it, and it chooses
  the path. The screen shows that; the player cannot give Gravity Drive to another ship.
- The matching of a step to an option is in TypeScript here and in Rust in the server. One
  shared implementation means taking the movement part out of `batch.rs`.
- Not shown yet, because they need the event history: the roll at a gravity rift, units that
  are removed after the movement, space cannon, and the read-only view of a past movement.
  Units that stay behind with no ship that carries them get no warning.
- The other steps (space cannon, combat, bombardment, what follows the landings, production)
  are still lists.

### A draft

A draft is the next tactical action of a seat, tried on a copy of the game: its activation and
its movement. `web2` shows it in the Draft workspace.

- The host writes `{"script": [...]}` at `ti4_request_ptr()` and calls `ti4_draft(length)`. A
  step of the script is `{"kind":"choose","option_id":…}` or a step of a movement plan
  (`DraftStep` in `ti4-view/src/planning/draft.rs`). The script of a tactical action is
  `choose tactical`, `choose <system>`, then the movement.
- The run forks the game at its last finished step, gives the seat a hypothetical turn
  (`Game::prepare_hypothetical_turn`), answers from the script, and stops at the first question
  that the script does not answer. Nothing is kept between two runs: a changed script is run
  again from its start. So the export returns at once and needs no second stack, and undo and
  redo of a draft are a list of scripts in the page.
- The outcome (`DraftOutcome`) has `update`, where the run ended, in the shape of a live update
  with `tactical`; `movement`, the update at the first ship to move, which a whole movement is
  staged against; `consumed`, how many steps fit; and `stop`: `open`, `complete` (the script
  ends the movement and the game offers that), `refused` with the step, `stopped` or `failed`.
- The run does not answer "done moving": what follows rolls dice or asks other seats.
- What a draft may show is checked as on the server (`ti4-view/src/planning/audit.rs`): only
  audited offers, no dice, nothing that the seat did not know. The other seats take no optional
  reactions. These checks and the recorded-decision adapters were moved from `ti4-server`,
  which re-exports them; its `PlanningRunner` is a worker thread around the same checks.
- The game at its last finished step is not kept: the checkpoints are made only after a step
  that asked the host. The base of a draft is the last checkpoint with the steps of the random
  seats since then played again on a copy of their stream, as an undo does. It is made for the
  first draft at a choice and dropped when the game goes on. The checkpoints are in a
  `thread_local` for this (a `Game` is not `Send`); the game that is parked is not touched.
- A test plays the same answers live and compares: the movement that the draft shows is the
  movement that the game then shows.
- The seat of a draft is the seat that was asked last (with stepping: also while another seat
  decides).
- A draft is also run in the strategy phase: the copy is put into the action phase as it
  stands (`Game::prepare_hypothetical_turn`). The status and agenda phases have no draft.

Limits:

- Only the activation and the movement. A question of the seat before its turn menu (a
  start-of-turn ability) stops the draft at its first step.
- The matching of a movement step to an option is now in three places: `DraftStep` in
  `ti4-view`, `planAnswer` in `web2` (for the live plan) and `batch.rs` in the server.

### Scenarios

A scenario is a board that a game only has after hours of play, for testing the screens
(`ti4-view/src/scenario.rs`). It is not a stored state: it is a function over the state of a new
game of a fixed seed, so a game that starts from one is saved, played again and taken back like
any other. `web2/?scenario=<id>&humans=<mask>` plays one, and the Dev sheet lists them.

- `late-game`: round 4, seat `a` (Sol) has the turn with ten fleets, every ship type, every ship
  upgrade and Gravity Drive. Around them are a gravity rift, a nebula, a supernova, an asteroid
  field (the map of eight seats has none, so one tile is replaced), a wormhole pair, fleets of
  other seats, cargo on planets and in space on the way, and a system with a command token of
  the seat. The other seats have command tokens on the board; one system has seven. Next to
  the fleet in the north is a system with two planets and no units (38), for landings without
  a fight.
- `late-game-deflectors`: the same, with Light/Wave Deflector and Antimass Deflectors.
- The tests of `scenario.rs` ask the facts of a tactical action for each case (a move that needs
  Gravity Drive, a rift roll, the move of 1 out of the nebula, the way around the supernova, the
  wormhole, a pickup on the way, three places to load, the command token, out of range). A new
  case is added there with its test.
- Found with it: the engine gives Antimass Deflectors no effect
  (`MovementRules::asteroid_fields_open` is only set in a test), so the asteroid field is closed
  in both scenarios. A test pins that, and fails when the engine changes.
- The save of a scenario is kept under `ti4.local.scenario.<id>.<humans>`, and names the
  scenario.

### The saved game

A game is not kept as its state. It is kept as what plays it again
(`web2/src/session/savedGame.ts`): the seed, the number of seats, the human seats, the SHA-256 of
the engine file, the option id of each answer of the page, and the switches of its seats
(`switches`: the decision, the mask, and how many answers the page had given). The random seats
and the dice follow the seed, so these are the only input that is missing. The server keeps a game the
same way: an init record and the decision log.

- Where: `localStorage`, key `ti4.local.<seed>.<players>.<humans>`, written whole after each
  answer. The bar under the shell has New game, Export and Import (a JSON file).
- A reload plays the answers again and stops at the first choice without one. The engine waits
  for an answer in the middle of a step, on its stack, so a position between two answers
  cannot be stored and resumed.
- Undo does not play the game again. The engine keeps a checkpoint (`Game::fork` and the
  position of the random seats' stream) after each step that asked the page, the last 32 of
  them. The page answers the pending choice with `-2`; the engine goes back to the last
  checkpoint before the answer, plays the random seats' steps since then again, and asks that
  choice again. `ti4_can_undo` says whether a checkpoint holds the answer. For an older answer,
  and when the game is over, the page plays the game again from its seed without that answer.
- An answer that the replayed game does not offer stops the replay. The error names the number
  of the answer, its id and the ids that are offered.
- A saved game of another engine file is not replayed before the player says so: another
  engine may play the same answers differently, and not every difference gives an illegal id.

### Measurements

`cd web2 && node ../crates/ti4-wasm/js/bench.mjs src/session/ti4.wasm` plays one full game in
Chromium: seed 3, eight seats, seat `a` answers option `7n mod options` at its n-th choice.
The game has 296 choices of seat `a` and 2619 decisions in all.

| What | Time |
| --- | ---: |
| Random seats between two choices: median | 9 ms |
| the same, 90% of the choices | under 0.15 s |
| the same, 99% | under 0.51 s |
| the same, longest | 0.57 s |
| Whole game | 14.6 s |
| of that, reading the 296 updates in JS | 0.14 s |
| Replay up to answer 50 (resume or undo there) | 1.8 s |
| Replay up to answer 150 | 5.6 s |
| Replay up to answer 296 | 14.2 s |
| Undo at answer 50, from a checkpoint | 8 ms |
| Undo at answer 150 | 25 ms |
| Undo at answer 290 | 42 ms |
| Writing the save, 296 answers (3.7 KiB) | 0.08 ms |
| Writing the save, 5000 answers (59 KiB) | 0.7 ms |
| Writing the save, 50000 answers (589 KiB) | 6.6 ms |

- Writing the whole save after each answer is not a bottleneck. No game comes near 5000 answers.
- The replay is the bottleneck: it costs all the engine time of the game so far. The page
  shows how far it is. Only a reload pays it; an undo comes from a checkpoint.
- A checkpoint after every step would make the game a third slower (19.7 s in place of
  14.7 s). After the steps that asked the page it costs nothing that can be measured, and an
  undo then costs what the wait for that choice cost.
- The time is the engine's, not the boundary's. The same game with random deciders only (2678
  decisions) takes 8.3 s natively (`--release`), 12.4 s in this file, and 7.9 s in a wasm file
  built with `opt-level = 3` (6.8 MB before compression in place of 4.5 MB). That is about
  3 ms for each decision natively.
- A CPU profile of that game (Node, a build with names) has no single hot function. The time is
  allocation, `String` and `Vec` clones and `BTreeMap` walks. By caller: `Game::apply_choice`
  26%, `Game::advance_turn` 25%, `Game::legal_options` 20%;
  `transactions::may_transact` 18% (it computes the neighbours of a seat each time);
  `fleet::enforce_everywhere` 12%, nearly all of it `units::catalogue`, which builds its map
  on each call; `Resolver::checkpoint` 9%; the copy of the state for the host at each offer 6%.
  The shares overlap.

### What a checkpoint is

`Game::fork` in the engine copies a game at a step boundary: the state, the open windows, the
dice streams, the counters and the resolver. The planning runner of the server uses it. The
test `a_fork_at_a_step_boundary_plays_the_same_game` checks it as a checkpoint for local play,
on eight seats, seeds 3 and 11, 700 steps each, a copy every 7 steps, each played 30 steps on
with a copy of the random seats' stream:

| A copy of the game | Plays the same game |
| --- | ---: |
| `Game::fork` | 192 of 192 |
| Made again from the `GameState` alone, at any step boundary | 54 of 192 |
| the same, only where no window is open | 54 of 61 |

- A window is open at 72% of the step boundaries (no window at 390 of 1400): mostly a trade,
  the end of a turn, a strategy secondary or a tactical action.
- The state alone is not a checkpoint, also where no window is open: the dice streams start
  again. This is what the server's `snapshot.json` holds, so it can check a replay but not
  replace one.
- One fork takes 2.6 ms in a debug build. The state is 37 KiB as JSON.
- A fork lives in memory only. A checkpoint that outlasts a reload, or the server process,
  needs the same parts stored: of the window types only `DiplomacyWindow` can be serialized
  today. Not done: a reload pays the replay once.

Limits:

- The random seats play on the main thread between two choices, and the page does not respond
  in that time (see the table).
- A reload late in a game takes as long as the game took the engine so far. So does an undo
  of an answer older than the 32 checkpoints, or after the game is over.
- With `humans` naming more than one seat, the page shows the hand of the asked seat at once.
  There is no hand-over screen.
- The view has ids, not names. web2 shows technologies by id and has a small table for the
  names and commodity values of the eight factions (`session/select/names.ts`).
- Systems outside the map (the wormhole nexus, the fracture) are not on the board of web2.
- The view does not say whose home a system is, and has no combat strength.

## Eight players

A game is set up on the recommended map template for its player count (`ti4-view`, source set
Prophecy of Kings). Seven and eight players work through three marked hacks (search for `HACK`):

- `ti4-engine/src/seating.rs`, `EXTRA_SEAT_FACTIONS`: seats seven and eight play Sardakk N'orr
  and Yin, which are not fully ported. They are not in `IN_SCOPE_FACTIONS`.
- `ti4-view/src/maps/build.rs`: a hyperlane tile that a template places a second time is
  replaced by another hyperlane tile. Hyperlane paths are not modelled, so nothing is lost today.
- `ti4-view/src/maps/build.rs`: when the filler pool (planet systems) is empty, the open slots
  get the other numbered tiles (anomalies, empty space, wormholes) in tile order.

The templates `7pStaticEq` and `8pStaticEq` still do not build: they name a tile `0g` that the
content does not have.

## Not done

- `ti4-server` does not compile to wasm (tokio, axum, threads, files). The views and the code
  that makes them from a state are in `ti4-view`, which both sides use. Still in the server: the
  messages (`protocol/`, which needs the planning runner), the decision facts and event history,
  `preset.rs` and `maps/`.
- The view of a seat is as old as the last finished step, apart from the combat and invasion
  boundary, which is current. The server has the same limit.
- The JSPI check reads the update at each choice, but nobody holds an action card that early, so
  it does not show the redaction. The tests of `ti4-server` do (`tests/projection_redaction.rs`).
- The other workspace crates were not rebuilt after the `rand` change or the move to
  `ti4-view`. `ti4-engine`, `ti4-view`, `ti4-server`, `ti4-bot-agent` and `ti4-wasm` pass their
  tests. `ti4-advisor` uses the moved paths through the re-exports of `ti4-server` and was not
  built (it needs libtorch).
- The UI queries are the `tactical` field of the update and the draft. No bots other than random. Undo and resume are replays from the seed; a stored
  position that can be resumed would need the engine to stop at step boundaries only.

## Commands

```sh
rustup target add wasm32-unknown-unknown

# Build (about 2 minutes; the result is target/wasm32-unknown-unknown/wasm-release/ti4_wasm.wasm)
cargo build -p ti4-wasm --target wasm32-unknown-unknown --profile wasm-release

# The same seed and step count must print the same line in wasm and natively
# (arguments: seed, steps, players)
node crates/ti4-wasm/js/run.mjs target/wasm32-unknown-unknown/wasm-release/ti4_wasm.wasm 3 2000 8
cargo run -p ti4-wasm --example run -- 3 2000 8

# JSPI in Chromium (from web2/, which has Playwright)
cd web2 && node ../crates/ti4-wasm/js/jspi.mjs ../target/wasm32-unknown-unknown/wasm-release/ti4_wasm.wasm

# Where the bytes are
cargo install twiggy
cargo build -p ti4-wasm --target wasm32-unknown-unknown --profile wasm-release \
  --config 'profile.wasm-release.strip=false' --target-dir /tmp/ti4-wasm-names
twiggy top -n 40 /tmp/ti4-wasm-names/wasm32-unknown-unknown/wasm-release/ti4_wasm.wasm
```
