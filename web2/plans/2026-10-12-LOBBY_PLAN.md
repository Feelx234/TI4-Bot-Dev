# web2: the start screen, the lobby and the setup of a game

Status: plan, 2026-10-12. Nothing is built. This is the plan that
`2026-10-11-ONLINE_MULTIPLAYER_PLAN.md` names in its last line ("a screen for the lobby of
several humans is the next plan").

## Goal

A game is set up in web2, in single player and in multiplayer, on the same screen and with the
same code. A table of several people is made, filled and started from web2. Today `/` is a
local game on a fixed setup, `?online` starts a game of eight with seven random seats at once
(`dev/OnlineApp.tsx`), and web2 has no screen before a game.

What the owner asked for (2026-10-12):

1. The features of the lobby of `web/` (`web/src/components/Lobby.tsx`, `MapPicker.tsx`,
   `hooks/useLobbySession.ts`), with a better UI.
2. The host chooses the number of players and the map, in the lobby.
3. The host chooses the set of strategy cards.
4. Factions are not bound to the seat. Today seat 1 is always Sol
   (`ti4_engine::seating::seat_faction`).
5. Random seats (`SeatController::BotRandom`) can be added in the lobby, with a wait time that
   the host sets, so that a draft can be tested while the other seats "think".
6. Several seats of one game are played from several tabs of one browser.

## Decisions (owner, 2026-10-12)

1. **Every player chooses between two factions when the game starts.** The two are drawn from
   the seed of the game. A seat that the server plays chooses one of its two at random. The
   lobby shows no faction before Start.
2. **Every faction that the engine implements can be drawn.** The owner says that all are
   implemented now. Found in the code: the six of `IN_SCOPE_FACTIONS`, and a module in
   `ti4-engine/src/factions/` for nineteen more (`MODULES` has 21 entries: the rest of the
   base game, Prophecy of Kings, and the three variants of Keleres), so twenty-five. The six
   factions of Thunder's Edge in the corpus have no module. `seat_faction` and the comments of
   `seating.rs` ("six, by owner decision", the HACK of seats seven and eight) are older than
   this. Phase 4 makes one list in the engine of the factions that can sit down, with a test.
3. **The sets of strategy cards are `pok` (default), `te`, `base_game` and
   `base_game_codex1`**: all four of the corpus. Found in the code: no line of the engine names
   `base2` or `base4`. Base Diplomacy is another card than the one of PoK (it readies every
   planet of the chosen system; PoK readies up to two anywhere). Phase 4 finds out what the
   engine does with these two ids and builds what is missing.
4. **The wait time** is one value for the game, for every random seat, before each decision
   that has more than one option. A reaction window that the seat only declines does not wait.
   The host can change it while the game runs (phase 7).
5. **`/` is the choice between single player and multiplayer.** This replaces decision 5 of
   the online plan ("no mode picker for now"). See "The start screen".
6. **There is no "Ready".** The host starts the game when every seat is filled. A player who
   is in the lobby is in the game.
7. **The seat order is always random.** It is drawn at Start from the seed of the game. The
   host does not move seats, and the lobby lists the players in the order in which they came.
8. **Single player shares the setup of multiplayer**: the player count, the map, the strategy
   cards, the choice of the faction. It is the same screen and the same code (phase 6).
9. **Later, not in this plan** (see "Later"): the tables of this browser on the start screen,
   and the MLP bot.
10. **The lobby has a live connection** (a websocket), from the first phase. The page does not
    ask the server again and again. See "The connection of a table".
11. **What the server keeps for `web/` is small and easy to remove.** See "The table of web2 on
    the server".
12. **A player who does not choose a faction holds the table.** Nothing is built against it:
    the owner plays with friends.
13. **Saved games of single player stay as they are** (one for each address). A list of saved
    games is later.
14. **A visitor of a running game sees the table first** ("Watch", "Rejoin as …"), not the
    game at once.
15. **A table on the server can start from a scenario** ("Start from", as the Dev sheet of a
    local game has it), to test a screen of the late game with several people. It is in
    phase 2, with the other tools for testing.

## How the lobby is built (asked by the owner, 2026-10-12)

- **The lobby looks like the game.** The game screen has the map at the left, a panel at the
  right, and one main button at the bottom of the panel. The lobby uses the same layout: the
  map of the table at the left, the players and the setup in the panel, "Start game" as the
  main button. It is not a form and it has no dialogs. On a phone it is the shell of the
  game too: one pane at a time with the tab bar, and the main button under every pane. See
  "The screen".
- **A change of the host applies at once.** When the host sets the players to 5 or picks a
  map, every player sees it at once. There is no "Apply" button for the setup. (In the game a
  choice is first marked and then sent with the main button; the setup of a table does not
  work like that, and `AGENTS.md` gets a line that says so.)

## Non-goals

- A list of all tables of the server, matchmaking, accounts, passwords for a table, chat.
- A spectator mode of its own. "Watch" opens the game with no seat, as today.
- The host history, the event log, restore and redo (still postponed).
- Bans, a faction draft in turns, a slice draft (Milty). The choice is one of two, for every
  seat at the same time.
- The factions of Thunder's Edge, and any other faction without a module in the engine.
- A map editor. The host chooses a map of the engine, or the random board.
- A time limit or a way around a player who does not choose a faction (decision 12).
- A list of saved games of single player (decision 13).
- Removing the lobby of `web/` now. `web/` keeps working, and what the server keeps for it is
  listed so that it can go in one change ("The table of web2 on the server").

## What `web/` has, and where it goes

| Feature of `web/` | In web2 |
| --- | --- |
| Create a table: player count, nickname, seed (dev) | Phase 1. No form: "Multiplayer" of the start screen makes the table, the rest is in the lobby. The seed stays in the address (`&seed=`) |
| Copy the address of the game | Phase 1, in the header |
| Join with a nickname | Phase 1. The nickname is edited in the row of the viewer |
| Ready / not ready | Removed (decision 6) |
| Start (host), with the reason while it is blocked | Phase 1. The main button of the host; the reason is the note of the footer |
| Leave (not the host) | Phase 1 |
| Watch as a spectator | Phase 1 |
| Rejoin as a disconnected player (takeover) | Phase 1, in the row of that seat |
| Connected / disconnected of each seat | Phase 1, one sign for each row |
| Host removes a player | Phase 1, "×" in the row |
| Host moves a seat up or down | Removed (decision 7) |
| Map picker: list for the player count, preview, "Re-roll", the notice "the host changed the map" | Phase 3. No dialog: the preview is the board of the lobby |
| Start preset (dev: `combat`) | Phase 3, in the address only (`&preset=`), as the seed |
| Add an MLP bot with the password of the server | Later |

New in web2: the start screen, the player count and the strategy cards in the lobby, random
seats with a wait time, the choice between two factions, a seat for each tab, the same setup in
single player, nicknames in the game.

## Findings that shape the plan

Server (`crates/ti4-server`):

- The lobby is HTTP (`http/games.rs`, `session/registry.rs`), with the header
  `x-ti4-player-session`: `POST /api/games`, and under `/api/games/{id}/lobby` the calls `GET`,
  `join` (new, or `takeover`), `leave`, `heartbeat`, `ready`, `reorder`, `start`, `map`,
  `map-preview`, `add-bot`, `remove-bot`. `GET /api/maps?player_count=N` and
  `GET /api/maps/{alias}/preview` are the map catalogue.
- The websocket (`/ws/games/{id}`) answers 404 while a table is a lobby: it is the socket of a
  running session. `web/` asks the lobby again every two seconds.
- Every change of a lobby is written through one function (`save_player_lobby` in
  `session/registry.rs`), and `PlayerLobbyView.lobby_version` counts the changes. So one hook
  there can tell every connection of a table.
- Who is "connected" in a lobby comes from a heartbeat call and a grace time
  (`player_heartbeat`, `PRESENCE_GRACE`).
- **The player count is fixed when the table is made**: it is the number of `slots` of
  `PlayerLobbyRecord`. Nothing changes it.
- **Random seats exist only through `fill_seats: "random"`** of `POST /api/games`, which fills
  every open seat and starts in the same request (`create_player_game_with_random_seats`). The
  lobby record does not know a random seat. The lobby view shows such a seat as a player
  "Random 2" who is not connected.
- A random seat answers at once (`RandomSeat::choose` in `session/worker.rs`).
- The server has scenarios of its own (`dev/scenarios.rs`, `/api/dev/scenarios`), which start
  a running game with no lobby. They are not the scenarios of a local game
  (`ti4-view/src/scenario.rs`: "Late game" and the others): the server cannot start those
  (the online plan, phase 2, says so for the Never window).
- `start` needs every seat filled and every member ready, the host too (`HumansNotReady`).
- A table has two phases, `lobby` and `running`. A step between them does not exist.
- A seat of a running game has an id like `player_1`. The nickname is in the lobby view only,
  and the page names a seat by its id (`NEXT.md`: "Seat PLAY…").

The setup of a game (`ti4-view`, `ti4-engine`), used by the server and by the wasm host:

- **Factions**: `ti4-view/src/map.rs` (`create_game_with_template`, `create_game_with_map`)
  calls `seating::seat_in_scope(player_ids)`: the faction is the place in the seat order. So a
  local game, every scenario and every fixture has the factions by seat too.
  `seating::seeded_faction_assignments(candidates, players, seed)` exists and is "not wired
  into game setup".
- The map preview has the faction of each seat (`SeatPreview.faction`), and
  `placeholder_homes` builds a preview with the homes of the factions by seat.
- **Strategy cards**: `ti4_engine::setup::strategy_card_setup(content, sources)` takes the set
  from the sources, and every host starts a game with `POK`. No host can name a set.
- **The wasm host has the maps**: `ti4-wasm` uses `TemplateLoader`, `default_template_for` and
  `create_game_with_template` of `ti4-view`. A local game takes the recommended map of its
  player count; nothing chooses another.
- A local game is `?local=<seed>&players=8&humans=<mask>` (`ti4_play(seed, players, mask)`),
  and it is saved as its seed and its answers (`SavedGame`, format 1).

web2:

- `dev/OnlineApp.tsx` is the whole entry of a game on the server, `dev/LocalApp.tsx` of a
  local game. `main.tsx` routes the addresses; `/` is the local game `DEFAULT_GAME`.
- **The player of a game is kept in `localStorage`** under `ti4.online.<game_id>.session`
  (`onlineKeys` in `wsTransport.ts`), and so are the draft, the plan and the staged movement.
  `localStorage` is shared by all tabs. So a second tab of the same game is the same player,
  and two seats in one browser cannot be played. (`web/` uses `sessionStorage`: a tab has its
  player, but a tab that is closed loses the seat, and a duplicated tab copies it.)
- The board is drawn from `BoardView` (`model/board.ts`, `game/board/GalaxyMap.tsx`), which is
  made from an update of a game. A preview is a list of `BoardTileView`, the `map_tiles` of a
  snapshot.
- The frame of the game is `GameShell` over a `GameSession`. It is not checked yet how much of
  it (the header, the footer with the main button, the tab bar of a phone) is free of the
  session.

## The start screen

`/` is the choice (decision 5): two rows, "Single player" and "Multiplayer", each the control,
with the keys 1 and 2.

- "Multiplayer" loads `?online`, which makes a table on the server and opens its lobby.
- "Single player" opens the same screen for a table in the page (phase 6). Until phase 6 it
  loads the local game of today.
- The header link of a game ("Local game" online, "Demo" in a local game stays) becomes
  "Start", to `/`. On a phone it is the last item of "Reference and more".
- What changes with it, and must be found before the change:
  - The installed app opens `/` (`public/manifest.webmanifest`, `start_url`): it opens the
    choice then.
  - `e2e/local.spec.ts`, `e2e/local.phone.spec.ts`, `e2e/entry.spec.ts` and the shots open the
    local game at `/`. They get its address (`?local=42&players=8&humans=1`, as
    `DEFAULT_GAME`).
  - The saved game of `/` is kept under the key of `DEFAULT_GAME`: check that the same game
    opens from its address, so nobody loses a saved game.
  - `AGENTS.md`: "Local play" ("the bare address `/` is that game") and "Online play" ("there
    is no mode picker") are rewritten.
- `?example=…`, `?gallery`, `?local=…` and `?scenario=…` are untouched: a test and the Dev
  sheet still start a local game on a fixed setup with no screen before it. The quick start of
  a game on the server (eight seats, seven random, started at once) stays as
  `?online&fill=random`, for the row "Multiplayer" of the Dev sheet and the e2e.

## The screen

The lobby is one screen with the layout of the game: the board is the map that the table will
play, the list of the players is in the panel, and the panel has the setup with the main
button in the footer.

Desktop (1920×1080), eight seats, the two long names of the demo data:

```
┌ Lobby · 6 players ─────────────────────────── [Copy link] [Start] ┐
│                                        │ SETUP                    │
│                                        │ Players        − 6 +     │
│          the map of the table          │ Strategy cards  PoK ▾    │
│     (six places of seats, not yet      │ Random seats wait − 1 s +│
│          given to a player)            │──────────────────────────│
│                                        │ PLAYERS  3 of 6          │
│  ┌ maps for 6 players ───────────────┐ │ ★ Ada (you) ✎            │
│  │ ▸Standard  Hyperlanes  Random  ↻  │ │   Bartholomew…        ×  │
│  └───────────────────────────────────┘ │ ⚄ Random 3            ×  │
│                                        │   Open    [Random seat]  │
│                                        │──────────────────────────│
│                                        │ Waiting for 3 open seats │
│                                        │ [      Start game      ] │
└────────────────────────────────────────┴──────────────────────────┘
```

- **One screen, no dialog** (pattern 2). `web/` opens the map picker as a dialog and the bot
  form as another. Here the map is always in view, and so are the players and the setup.
- **A control is where the choice is made** (pattern 1). The list of the maps is on the board,
  under the map that it changes. The nickname is edited in the row of the viewer ("✎").
  "Random seat" is in the row of the open seat. "×" is in the row of the player that it
  removes.
- **The same screen for everyone** (pattern 10). A player who is not the host sees the setup
  with its values and no controls. A visitor with no seat sees the same, with "Join" in the
  footer.
- **A state has one sign and a word on hover** (patterns 4 and 6), in a column of fixed width:
  "★" host, "⚄" a seat that the server plays, "◇" not connected, an empty outline for an open
  seat. The list has no numbers and no seat colours: the seats are drawn at Start
  (decision 7).
- **Names** (pattern 13): a column of fixed width, cut with "…", the full name on hover; on a
  phone the name takes a second line.
- **Amounts change in steps of one** (pattern 8): the player count is a counter. Beside the
  open seats there is one button for the usual target: "Fill with random seats".
- **The main button** is in the footer, and Enter is the main button: "Join" for a visitor,
  "Start game" for the host. A player who is not the host has no main button; the note says
  "Waiting for Ada to start". While Start is blocked, the note says why ("Waiting for 3 open
  seats"). Esc leaves the editor of the nickname.
- **A change of the host is told to the others once**, as a toast ("The host changed the
  map").
- No text that explains the lobby is visible. What "Random seat" and the wait time do is behind
  "?" (`Hint`).

Phone (412×923): the same blocks in the panes of the game shell, **Map · Players · Setup**,
with the footer under every pane, so the main button is always in reach. A pane may scroll
down. Nothing is cut at the side with eight seats and the long names.

### The choice of the faction

After "Start game" the same screen asks every player for a faction (decision 1). It is a
decision as in the game, so it uses the parts of a decision (`AGENTS.md`, Decisions).

- The seats are drawn now: the list of the players is in seat order, with the number and the
  colour of each seat, and the map marks the place of each seat. The place of the viewer has
  a sign of its own.
- The panel has the heading "Choose your faction" and a list of two rows, with the keys 1 and
  2. A row has the name and the symbol of the faction; its sheet (the abilities, the leaders,
  the starting technologies, the starting fleet, the home system) is behind the name, as
  printed, from the catalogue. Rule text is not written in web2.
- A row is staged ("Selected", "Not sent"); the board then shows the home system of that
  faction at the place of the viewer. The main button "Choose faction" sends it. Esc clears
  the choice.
- Every seat chooses at the same time. A row of another seat says "▶ Deciding" or "✓ Chosen",
  never which faction and never which two it has (pattern 11). When the last seat has chosen,
  the game starts and every faction is shown.
- A seat that the server plays has chosen at once.
- A player can change the choice until the last seat has chosen ("Change", as a chosen thing
  of a strategy card).

## One setup, two hosts

The setup of a game is decided in Rust, in `ti4-view`, and both hosts call it (decision 8, and
decision 6 of the online plan: one implementation). The page has one screen over one seam.

- **`ti4-view/src/setup.rs`** (new):
  - `TableSetup { player_count, map, strategy_cards, seed }`: what a host chooses.
  - `setup_options(player_count)`: the maps that build for the count, and the sets of strategy
    cards with their names and the names of their cards.
  - `preview(&TableSetup)`: the tiles of the board and the places of the seats, with no
    faction.
  - `draw(&TableSetup, players)`: the seat order and the two factions of each seat, from the
    seed.
  - `GameSetup { seats, factions, strategy_cards, map }`: the drawn order with the chosen
    factions. `create_game_with_template` and `create_game_with_map` take it.
    `GameSetup::legacy(players)` is the game of today (factions by seat, the order as given,
    the set of the sources): the scenarios, the fixtures, `?local=…` and the tables of `web/`
    use it, so no seeded game changes.
- **The server** keeps a `TableSetup` in the lobby record, answers the lobby calls from these
  functions, and keeps the choices of the seats while they choose.
- **The wasm host** gets exports for the same functions (`ti4_setup_options`,
  `ti4_setup_preview`, `ti4_setup_draw`) and a way to start a game from a `GameSetup`.
- **The page** has `Table` in `session/`, the seam of the setup as `Transport` is the seam of a
  game: the table as it is (the players, the setup, the phase, the offer of the viewer), the
  preview, and the calls (set a value of the setup, add or remove a seat, start, choose a
  faction). `serverTable` talks to the server (a socket and the HTTP calls); `localTable` (phase 6) keeps the table in the page
  and asks the wasm. The screen and `select/lobby.ts` do not know which one they have, and
  `boundaries.test.ts` holds it.

## The connection of a table

A table has a websocket of its own from the moment it is made (decision 10):
`/ws/tables/<game_id>`. It is not the socket of a game.

- The page sends `Subscribe` with its `player_session` (or none, as a visitor). The server
  answers with the table as that viewer may see it, and sends it again after every change:
  a player who came or left, a value of the setup, the phase, a choice of a faction. The
  message is the lobby view, whole; a table is small.
- A player is "connected" while a socket of that player is open. The heartbeat call is not
  used by web2.
- The calls of a table stay HTTP (`join`, `leave`, `settings`, `start`, …): they are there,
  they answer with the reason when they are refused, and the answer of a call and the push
  of the socket carry the same view with its `lobby_version`; the page keeps the newer one.
- When the phase is `running`, the page closes this socket and opens the socket of the game
  (`/ws/games/<game_id>`), as today. The sheet "Table" of a running game (phase 7) opens the
  socket of the table again while it is in view.
- Reconnect as the game does (`wsTransport.ts`: 0.5 s, then twice as long, up to 15 s), with
  the same one signal: "Offline · reconnecting" in the header. Take that code out into one
  helper that both sockets use.
- Server: `ws/table.rs`, and in the registry one channel for each table
  (`tokio::sync::watch` of the `lobby_version`), told from `save_player_lobby` and from the
  change of presence. `PROTOCOL_VERSION` is checked as on the game socket.

## The table of web2 on the server

What web2 needs and `web/` does not must not spread through the lobby code (decision 11).

- **One field, one branch.** `PlayerLobbyRecord.setup` is `legacy` (default: a table of
  `web/`) or `table` (web2). It is read in one place: `start`. A `legacy` table starts as
  today (everyone ready, the order of the slots, the factions by seat). A `table` goes to the
  draw of the seats and the choice of the factions.
- **New code is in files of its own**: `session/table.rs` (the settings, the random seats,
  the draw, the choice) and `ws/table.rs`. The lobby functions of the registry that both use
  (`create`, `join`, `leave`, `take_over`, `remove`, the map) are not changed in what they do.
- A call that only web2 has (`settings`, `random-seat`, `faction`, `nickname`) is refused for
  a `legacy` table; `ready` and `reorder` are refused for a `table`. That is one check at the
  top of each such call, not a branch inside it.
- **What goes when `web/` goes**, in one change: the field and the `legacy` branch of
  `start`; the calls `ready`, `reorder`, `heartbeat`, `add-bot`; `ready` of
  `PlayerLobbyMember` and `HumansNotReady`. `GameSetup::legacy` stays, because the scenarios
  and the fixtures use it. This list is kept up to date by each phase, and it joins the list
  of the online plan ("Postponed").

## A seat for each tab

The address of a tab says which player the tab is: `?online=<game_id>&seat=<player_id>`. The
id of a player is public; the credential is not in the address.

- `localStorage` keeps every player that this browser has in a game:
  `ti4.online.<game_id>.players` is a map from the id of a player to its `player_session`.
- The draft, the plan, the staged movement and the draft of a secondary are kept for each
  player: `ti4.online.<game_id>.<player_id>.draft` and so on. Today they are kept for each
  game, so two seats of one browser would share a draft.
- A tab without `seat`: with one player of this browser in the game, the tab is that player and
  the address gets its `seat`; with none it is a visitor; with several the list marks them
  ("yours") and a click on such a row makes the tab that player.
- A row of an open seat has "Join in a new tab" for a player of the table: it joins as another
  player ("Ada 2") and opens the tab with that `seat`. This is the way to play several seats.
- A reload, a closed tab that is opened again and a bookmark are the same seat. A duplicated
  tab is the same player twice, which the server allows (two connections of one player).
- The key `ti4.online.<game_id>.session` of today is read once and moved to the map.

## Phases

Each phase leaves `?local=…`, `?online&fill=random` and `web/` working and green.

### Phase 1: the start screen, the lobby, and its connection (web2, a small part of the server)

The server gets the socket of a table and nothing else. The lobby has what the server can do
today.

1. **The start screen** ("The start screen"): `/` is the choice; "Single player" loads the
   local game of today. The tests that open the local game at `/` get its address.
2. **Routing.** `?online` makes a table (`POST /api/games` with no `fill_seats`; the player
   count is the last one that this browser used, else six) and goes to
   `?online=<game_id>&seat=<player_id>`. `?online=<game_id>` asks the table: the phase `lobby`
   shows the lobby, `running` mounts the game as today, 404 is "The server no longer knows
   this game."
3. **The socket of a table** on the server ("The connection of a table"): `ws/table.rs`, the
   channel of a table in the registry, presence from the open socket.
   `tests/it/table_socket.rs`: a second connection is told when a player joins, leaves and
   when the map changes; a visitor gets the view with no credential in it; a closed socket is
   "not connected" after the grace time.
4. **`session/table.ts`**: the `Table` seam and `serverTable`: the socket for the table as it
   is, the HTTP calls that the server has for the changes. Of two views the one with the
   higher `lobby_version` is kept. A failed call is a toast with the reason; the lobby stays
   as it was. A lost socket is "Offline · reconnecting", and the page tries again by itself.
5. **`session/select/lobby.ts`** makes a `LobbyView` (in `model/`): the rows of the players
   with their signs, the setup rows with `editable`, the footer (the main button, the note).
   The components decide nothing from names (as `ScoreRowView.open`).
6. **The screen**: the list of the players, the setup block (in this phase the player count
   is shown and cannot be changed), the footer, the header with "Copy link" and "Start". The
   map is the preview of `GET …/lobby/map-preview`, read-only, asked again when `map_revision`
   changes.
   - Check first how much of `GameShell` is free of a `GameSession` (the header, the footer,
     the tab bar of a phone). Take the frame out as a component that both use; do not copy it.
   - Check how `GalaxyMap` can draw a list of `BoardTileView` with no game. A `BoardView` with
     the tiles only, made in `select/lobby.ts`, is the aim; the camera and "Fit board" then
     work as in the game.
7. **A seat for each tab**, as the section above.
8. **The nickname**: kept in `localStorage` (`ti4.nickname`), sent with `POST /api/games` and
   `join`. In this phase it is edited before the join only; phase 2 adds the rename.
9. **No "Ready"** (decision 6). The server of today asks for it, so the page marks its player
   ready when it joins and shows nothing of it. Phase 2 removes the need.
10. **The start**: when the table says `running`, every tab mounts the game with its player.
   (In this phase the seats are in the order of the join and the factions are by seat.)
11. `AGENTS.md`: a section "Lobby" with the rules of "The screen", and the changed lines of
    "Online play" and "Local play".

Tests: `select/lobby.test.ts` (rows and signs for the host, a player, a visitor; the reasons of
a blocked start); `table.test.ts` over a socket of the test and a fake `fetch` (an older view is dropped, a failed
call, a lost socket);
a test of the keys (two players of one browser do not share a draft; the old key is moved);
`e2e/lobby.spec.ts` against the real server: **one browser context with three pages** makes a
table of three from the start screen, joins from two more tabs, starts, and each page is
another seat of the running game; a tab sees a player who joins with no reload and no wait;
a reload of a page keeps its seat. A phone case: nothing is
cut at the side, and the main button is in view.

### Phase 2: the player count, random seats and their wait time (Rust + web2)

The debug tools of the owner. After it a draft can be tested alone: one or two tabs, the other
seats random, with time to draft.

Server:

1. **The lobby record knows who plays a seat.** `PlayerLobbyMember.controller` (`human` by
   default, so the records of today load). `PlayerSlotView.controller` is public. A seat that
   the server plays cannot be taken over and is never "not connected".
   `start_player_lobby_with` reads the random seats from the record; its `random` argument and
   the special path of `create_player_game_with_random_seats` are removed (`fill_seats:
   "random"` fills the record, then starts as every table).
2. **`POST …/lobby/random-seat`** `{ slot_id }` (host, phase `lobby`): the open seat is played
   by the server. With no `slot_id`: every open seat. `remove-bot` already removes any seat but
   the host's; it removes a random seat too.
3. **`POST …/lobby/settings`** (host, phase `lobby`), each field optional:
   - `player_count` (2 to 8). More seats are added open. Fewer seats: the open seats go first,
     then the random seats; a seat of a person is never removed (`409` with the names that are
     in the way). The map of a table is for one count: the choice becomes the default map of
     the new count (`default_template_for`), and `map_revision` goes up. The host's own choice
     of a random board stays.
   - `random_wait_ms` (0 to 30 000).
   - `POST …/lobby/nickname` renames the player of the credential.
4. **The field `setup`** of the lobby record ("The table of web2 on the server"): `legacy` or
   `table`. `POST /api/games` takes it; web2 sends `table`. The new calls of this phase are
   in `session/table.rs`.
5. **No "Ready"** (decision 6): `start` of a table of web2 needs every seat filled and nothing
   else. The `ready` call stays for `web/`.
6. **The seat order is drawn at Start** (decision 7): the players are shuffled with a stream
   of the seed of the game (a domain of its own in `GameRng`, as
   `seating:faction-assignment:v1`). `reorder` is refused for a table of web2. The preview of
   the lobby shows the places with no player on them.
7. **The wait** (decision 4). `SeatController::BotRandom` stays as it is on the wire; the wait
   is a value of the session (`SessionConfig`, kept in `init.json` and in the lobby record).
   The random seat waits before it answers, only in the live game:
   - not while a log is replayed (a recovery, an undo, the host's history) and not on a copy
     (`RunDraft`, `check_script`);
   - with no lock held, so a snapshot, `RunDraft` and `SetReactionMode` are answered while a
     seat waits. Read `session/worker.rs` first: where the decider is called and what is locked;
   - in short slices that end when the session is replaced or closed (as the 5 ms loops of
     `session/mod.rs`), so an undo does not wait for a seat;
   - the draw of the seat is made after the wait, from the same stream, so the game of a seed
     is the same game with every wait time. A test proves it.
   - The update must say who the game waits for while a random seat waits (`TurnStatus` or the
     pending choice of that seat): the page then says "Waiting for Random 3", as for a person.
8. `PlayerLobbyView` gets `random_wait_ms` and `setup`.
9. **"Start from"** (decision 15). `start_from` of `lobby/settings`: none ("Round 1"), or the
   id of a scenario of `ti4-view` (`scenario::find`, `scenario::apply`), the same list that
   the Dev sheet of a local game has. `GET /api/setup-options` lists them with what each
   holds.
   - A scenario is a game of its own: it has its seed, its number of seats, its board and
     its factions. So the table takes the player count of the scenario, and the map, the
     strategy cards and (later) the choice of the faction are not asked: their rows show the
     values of the scenario with no control.
   - The seats are not drawn: a player sits in the order of the join, the host first. A case
     of a scenario is at a certain seat, and a test must reach it. Open seats are filled with
     random seats as in every table.
   - Read how the wasm host starts a scenario (`ti4_play_scenario`) and use the same function
     of `ti4-view`; the server keeps the first state in `init.json` as for every game, so a
     recovered game is the scenario. The scenarios of `dev/scenarios.rs` are not touched.
   - It is a tool for testing and it is not hidden: the row is in the setup of every host,
     and the lobby says "Start from: Late game" to every player.

web2:

10. The setup rows get their controls for the host: the counter of the players ("− 6 +"; "−" is
   blocked with the reason when a person is in the way), the counter of the wait in steps of
   half a second ("Off" at zero). The row of an open seat has "Random seat"; under the list
   "Fill with random seats". A random seat has "⚄" and "×".
11. The setup row "Start from" is a list: "Round 1" and the scenarios, with what a scenario
    holds behind "?", as in the Dev sheet.
12. The nickname is edited in the row of the viewer at any time before Start.
13. The page no longer sends `ready`. `?online&fill=random` sends `fill_seats` as today, and
    can name `wait=<seconds>`.

Tests: `tests/it/lobby_settings.rs` (grow and shrink; a person in the way; the map follows the
count; only the host; a table of `web/` still needs ready and keeps its order),
`tests/it/random_seats.rs` (a seat that is added in the lobby; the same game with a wait of 0
and of 50 ms; a recovered game; an undo while a seat waits is not held up; the seat order is
the same for a seed and another for another seed), `tests/it/table_scenario.rs` (a table of
two people and random seats starts the late game; each person has the seat of the join; a
recovered game is the same), `protocol_roundtrip.rs`; the fixture test
and the lobby e2e of `web/` stay green. web2: the selectors, and in `e2e/lobby.spec.ts` the
case of the owner: two tabs and four random seats with a wait of one second, the secondary of
a card is drafted in one tab while the random seats wait, and the toolbar says who the game
waits for. A second case starts "Late game" with two tabs and opens a sheet of the late game
in each.

### Phase 3: the map (web2)

1. The strip of the maps on the board, for the host: one entry for each map of
   `GET /api/maps?player_count=N` and "Random", with the name, the author and what
   `MapChoiceView` says (the number of systems, hyperlanes, "recommended"). The chosen one has
   "▸". A click saves it (`POST …/lobby/map`) and the board shows it. "↻" draws the open
   places again (the same call: each one draws a new seed). Read
   `web/src/presentation/mapPicker.ts` for the order and the words; do not import it.
2. A player who is not the host has no strip: the name of the map is in the heading of the
   board. The map of the lobby is asked again when `map_revision` changes, and the change is a
   toast.
3. The board marks the home places of the seats. Before Start no player is on a place.
4. Looking at another map before choosing it (`GET /api/maps/{alias}/preview`) is not built:
   a choice applies at once and can be changed until Start, so the board is the preview. Ask
   the owner again when the list of maps is long.
5. The start preset of `web/` (dev, `combat`) is `&preset=` of the address that makes the
   table. No control.

Tests: the selector of the strip (host and not host); `e2e/lobby.spec.ts`: the host chooses a
map, the other tab shows it and the toast, the started game has that map; a shot of the lobby
with eight seats and the long names at 1920×1080 with no vertical scroll (`e2e/shots.spec.ts`)
and on the phone.

### Phase 4: the setup in one place, the factions that can sit down, the strategy cards (Rust + web2)

1. **`ti4-view/src/setup.rs`**, as "One setup, two hosts": `TableSetup`, `setup_options`,
   `preview`, `GameSetup`, `GameSetup::legacy`. `start_from` of phase 2 becomes a field of
   `TableSetup`, so single player has it from the same code (phase 6). `create_game_with_template` and
   `create_game_with_map` take a `GameSetup`. The server moves its preview and its map
   catalogue (`maps/catalog.rs`) onto these functions; the seat order of phase 2 moves into
   `draw`. Guard: every fixture of web2 and of the server is the same byte for byte.
2. **One list of the factions that can sit down**, in `ti4-engine` (`seating.rs`): the six of
   `IN_SCOPE_FACTIONS` and every faction of `factions::MODULES` (decision 2). A test plays the
   setup of each (`seating::deploy`, its home on a board) and fails for one that does not
   start. `IN_SCOPE_FACTIONS` itself is not changed: coverage, training and the advisor read
   it. Update the comments of `seating.rs` that say six.
   - **A faction can change the board**: the Ghosts of Creuss have their home off the map and
     a gate in its place (`seating::place_creuss_home`), and other factions may ask for more.
     Read how `create_game_with_template` and `build_template_galaxy` seat such a faction
     today, and test a table with each faction of the list on a template and on the random
     board.
   - Keleres is one entry of the list. Its variant (`keleresm`, `keleresx`, `keleresa`) is
     drawn when the game is made, among those whose base faction is not at the table
     (`seating::tribuni_variants_available`).
3. **Until phase 5 a table of web2 gets its factions at random**: `draw` gives each seat one
   faction of the list (`seating::seeded_faction_assignments`). This is the step that removes
   "seat 1 is Sol"; phase 5 makes it two to choose from.
   - The preview tells no faction: `SeatPreview.faction` and `faction_name` are left out for a
     table of web2, and the tile of a home is sent as a place of a seat. Read
     `maps/build.rs` (`placeholder_homes`, `default_template_for`): whether a template builds
     must not depend on which faction sits where, or the preview and the start must use the
     same rule.
   - web2 shows every faction of the list: the symbol and the colour of a seat, the name, the
     rows that only some factions have (faction technologies, the flagship, the mech). Play a
     table of eight with each faction once in a test of the selectors, so that no screen shows
     an id. The seat of the viewer is found by the player, never by the place (check
     `select/players.ts` and the mock data for "seat 1 is Sol").
4. **Strategy cards** (decision 3). `ti4_engine::setup::strategy_card_setup` gets a variant
   that takes the alias of a set; `GameSetup` names it, and `legacy` reads the sources as
   today. `strategy_cards` is a field of `TableSetup`, of the lobby record and of
   `lobby/settings`.
   - The four sets with their names and the names of their cards come from `setup_options`
     (`GET /api/setup-options` on the server). The page writes no name of a set or a card.
   - **The base cards.** Find how the engine resolves a card (by its id, as `te4construction`
     and `te6warfare`, or by its number) and what a game of `base_game` does today with
     `base2` and `base4`. Build what is missing: base Diplomacy readies each exhausted planet
     of the chosen system; base Construction is the card of PoK with another wording of its
     secondary (check the corpus text against the rules before treating it as the same). This
     is a new card of the engine, not a change of a game that exists. It can ship after the
     rest of the phase: until then the lobby offers `pok` and `te`.
   - A test in `ti4-engine` plays the primary and a secondary of every card of every set, and
     checks the two base cards against their printed text.
   - Check that the cards of `te` and of the base game are right in a game with the sources
     `POK` (a card may read what only its own expansion has).
   - web2: `session/select/strategy.ts` and `names.ts` must know the cards of the other sets
     (`te4construction`, `te6warfare`, `base2`, `base4`): the action picker, the seat list of
     a strategic action and the draft of a secondary. Check each with a game of each set.
5. **web2**: the setup row "Strategy cards" is a list of the sets (the name; the cards that
   differ from the default behind hover).

Tests: `ti4-view` (`legacy` is the game of today; a table of web2: every seat has another
faction, the same seed gives the same table); `ti4-engine` (the list of the factions, the test
of the sets); `tests/it/lobby_setup.rs` (the settings, the preview has no faction, a recovered
game has its factions and its set); `e2e/lobby.spec.ts` (two tables with different seeds have
other factions; a game of the `te` set shows its Construction in the action picker).

### Phase 5: every player chooses between two factions (Rust + web2)

1. **`ti4-view/src/setup.rs`**: `draw` gives each seat two factions, all of them different
   (sixteen of twenty-five at a table of eight), from a stream of the seed. Keleres is offered
   as one faction; its variant is drawn after the last choice. `choose(seat, faction)` takes
   only one of the two of that seat. A random seat's choice is a draw of the same seed. When
   every seat has chosen, the result is the `GameSetup`.
2. **Server**: a third phase of a table of web2, `choosing`, between `lobby` and `running`.
   - `start` draws the seats and the offers, keeps them in the lobby record (a recovered
     server goes on with the choice), lets the random seats choose, and goes to `choosing`.
     Nobody can join or leave now; a player who is gone can be taken over.
   - The lobby view of a player has `your_offer` (two factions) and `your_choice`, for the
     credential of that player only. Every viewer gets, for each seat, only "chosen" or not.
   - `POST …/lobby/faction { faction }` sets or changes the choice of the player. With the
     last choice the game is made and the phase is `running`.
   - A redaction test, in the style of `tests/it/projection_redaction.rs`: no view of another
     player and no view without a credential names an offer or a choice before the game runs.
   - `web/` never sees `choosing`: only a table of web2 has it.
3. **web2**: "The choice of the faction" of "The screen". The sheet of a faction comes from the
   catalogue of the wasm (`Transport.catalog` has the abilities, the leaders and the
   technologies as printed); check what a faction sheet needs that the catalogue does not have
   (the starting fleet, the starting technologies, the home system, the commodities) and add
   it in `ti4-view`, from `ti4-content`.
4. A player who does not choose holds the table. Nothing is built for it (decision 12).

Tests: `ti4-view` (the offers are different, the same for a seed, a choice outside the offer is
refused, Keleres); `tests/it/lobby_factions.rs` (three people choose, a change before the last
choice, the redaction, a recovery in the middle, random seats); web2 selectors (the rows of
the seats, the staged row); `e2e/lobby.spec.ts`: three tabs choose, no tab shows a faction of
another seat before the last choice, and each tab then plays the faction that it chose.

### Phase 6: single player uses the same setup (Rust + web2)

1. **The wasm host** exports the functions of `setup.rs` (`ti4_setup_options`,
   `ti4_setup_preview`, `ti4_setup_draw`) and starts a game from a `GameSetup`
   (`ti4_play_setup`). `ti4_play(seed, players, mask)` stays, with `GameSetup::legacy`.
2. **`localTable`** in `session/`: a `Table` that lives in the page. Its players are "You" and
   random seats; the host is the page. It has the player count, the map, the strategy cards,
   the wait time of the random seats (the page waits before it lets a seat decide; check
   against "Other seats: Step" of the Dev sheet, which is the same idea) and the choice of the
   faction. It has no link to copy, no nicknames to edit and nobody to wait for.
3. **"Single player" of the start screen** opens the lobby screen over `localTable`, when
   this browser has no saved game of single player; with one it opens that game, as today
   (decision 13). "New game" of the Dev sheet opens the setup.
4. **The saved game** gets the setup: `SavedGame` format 2 has the `GameSetup` beside the seed
   and the answers; format 1 is read as `legacy`. The address of such a game names the save,
   not the setup (a setup does not fit an address). Export and Import keep working.
5. "Play as" and several seats in the page (the mask) stay tools of the Dev sheet.
6. The row "Multiplayer" of the Dev sheet stays the quick start.

Tests: the contract of `Table` runs against `serverTable` (a real server) and `localTable` (the
wasm), as `transportContract.test.ts` does for a game: the options, a change of the count, the
preview, the draw, a choice, the start. `savedGame.test.ts` (format 1 and 2).
`e2e/local.spec.ts`: from the start screen, a game of four on another map and the `te` set, a
faction is chosen, the game is played for a turn, a reload continues it.

### Phase 7: the table in the game (Rust + web2)

1. **Nicknames in the game.** The snapshot and the update get the seats of the table as a
   fact: the nickname and who plays the seat (`human`, `random`), per player id. It is public.
   The page names a seat by its nickname everywhere it says "Seat PLAYER_1" today (the player
   table, the toolbar status, the seat list of a strategic action, the sheets), in the column
   of fixed width; a random seat has "⚄".
2. **The sheet "Table"** in "Reference and more": the lobby screen, read-only (pattern 10),
   for the running game: who sits where, who is connected, the map by its name, the set of
   strategy cards. The question that it answers: "who is at this table, and how was it set
   up?" For the host it has the counter of the wait of the random seats
   (`POST /api/games/{id}/settings`, host only, for a running game).
3. **A visitor of a running game** (`?online=<game_id>` with no player of this browser) gets
   the same screen in place of the game, with "Watch" as the main button and "Rejoin as …" in
   the row of a seat that can be taken over (`can_take_over`, `join` of kind `takeover`).
   Today such a visitor is a spectator at once (decision 14).
4. Update `NEXT.md` and `TODOS.md` (the line "Lobby, map picker with preview, nicknames, seats
   and bots"), and the list of what `web/` has and web2 does not (the online plan,
   "Postponed").

Tests: the redaction test gets the new fact (it holds no credential); a selector test of the
names; `e2e/lobby.spec.ts`: the names of three tabs are in the player table of each; the host
changes the wait in the running game; a tab that is closed is taken over from another tab
after the grace time (the server is started with a short one for the e2e, if it can be set).

## Later

Not in this plan (decision 9).

- **The tables of this browser on the start screen**: one row for each game that
  `localStorage` has a player for and the server still knows, so a closed tab finds its table
  again. The map `ti4.online.<game_id>.players` of phase 1 is what it reads.
- **The MLP bot.** `web/` adds it through the bot service of the server (`add-bot`, a process
  of its own, the advisor, libtorch). The owner's idea: a simple inference code with no
  libtorch, so that the bot is a kind of seat beside the random seat, on the server and in the
  wasm of single player. `Table` and the lobby record already name who plays a seat
  (`controller`), so it is one more value there and one more button in the row of an open
  seat.
- A list of the saved games of single player.
- **More of the setup** (ideas, owner 2026-10-12: nice to have). Each is one more row of the
  setup block and one more field of `TableSetup`; none is checked against the engine yet.
  - The points that win (10, or 14 for a long game).
  - The content of the game: with or without Thunder's Edge and the codices. The engine has
    the source sets `BASE`, `POK` and `FULL`; both hosts start every game with `POK` today.
    It changes the cards, the systems and the relics, and it is the natural owner of the
    strategy card set ("as the content says", or another).
  - The factions that can be drawn (leave some out), and how many each player chooses from.
  - House rules of the seats: whether the undo of a seat is allowed, whether a visitor may
    watch.

## Checks that every phase must pass

- Rust (phases 2, 4, 5, 6, 7): `cargo c-web`, `cargo t-web`, `cargo fmt --all -- --check`,
  `cargo l-web`; server tests with `cargo test -p ti4-server --test it <file>::`. Do not build
  other crates.
- `PROTOCOL_VERSION` stays 3: a phase adds optional fields, endpoints, a socket and the field
  `setup` with the behaviour of today as the default. `web/`'s fixture test and its lobby e2e stay green;
  check `web/src/protocol/decode.ts` for a decoder that refuses a field that it does not know.
- The records on disk (`PlayerLobbyRecord`, `PlayerGameInitRecord`) refuse unknown fields: a
  new field has a default, and a table of today loads.
- Rebuild `session/ti4.wasm` (`scripts/build-wasm.sh`) after phases 4, 5 and 6.
- web2: `npm test`, `boundaries.test.ts` (the calls of a table are in `session/`; `game/` and
  `ui/` do not know the server), the e2e on desktop and phone. A changed screen is checked with
  eight seats and the two long names. Format only the files that changed (`npx oxfmt <files>`).
- The owner looks at the lobby on the phone after each phase.

## Risks

- **Hidden information before the game.** The seed of a table never leaves the server. The
  offers and the choices of the factions are private until the game runs (the redaction test
  of phase 5). A field of the lobby view without `your_` is read by a visitor with no
  credential.
- **A seed tells the offers.** Whoever knows the seed of a table knows the two factions of
  every seat. The seed is in the address only for development (`&seed=`), as in `web/`.
- **A wait that holds a lock** would stop the drafts that it is there to test. Phase 2, item 7
  reads the worker first; the test "an undo while a seat waits" and the e2e of the secondary
  draft are the guard.
- **Reaction windows with a wait.** Decision 4 keeps a pass of a window without a wait. If the
  server cannot tell such a decision from the others, measure a round of eight with a wait of
  one second before choosing another rule.
- **Two tabs, one player.** A duplicated tab is the same player twice, and both write the same
  draft key. The server allows it; the page does not guard against it. Say so in `AGENTS.md`.
- **Seeded games change** if the setup of today changes. `GameSetup::legacy` keeps it, and the
  fixtures prove it.
- **A set or a faction that the engine half knows.** The owner says that all factions are
  implemented; the plan found modules for twenty-five and no code for the two base cards. The
  tests of phase 4 (every faction starts, every card of every set resolves) are the guard, and
  what they find is told to the owner before it is built around.
- **Twenty-five factions on every screen.** web2 was looked at with the six factions by seat,
  and the two of seats seven and eight. Phase 4, item 3 plays each faction once.

## Open questions (owner)

None. Every decision of this plan is made.

## Order and size

Phase 1 comes first: every other phase puts a control on its screen. Phase 2 is next, because
it gives the owner the tools to test drafts (random seats with a wait, several tabs). Phase 3
is independent of the phases after it. Phase 4 is mostly Rust and can run beside phase 3.
Phase 5 needs 4. Phase 6 needs 4, and 5 for the choice of the faction. Phase 7 is last.

| Phase | Where | Size |
| --- | --- | --- |
| 1 the start screen, the lobby, its socket, a seat for each tab | web2, ti4-server (the socket of a table) | large (two new screens, the frame taken out of `GameShell`, the keys, the addresses of the tests) |
| 2 player count, random seats, wait, no ready, random seat order, "Start from" | ti4-server, web2 | medium to large |
| 3 the map | web2 | medium |
| 4 one setup, the list of factions, strategy card sets | ti4-view, ti4-engine (the list, the base cards), ti4-server, ti4-wasm (signature), web2 | large |
| 5 the choice between two factions | ti4-view, ti4-server, web2 | medium |
| 6 single player on the same setup | ti4-wasm, web2 | medium |
| 7 the table in the game | ti4-server, ti4-view (a fact), web2 | medium |
