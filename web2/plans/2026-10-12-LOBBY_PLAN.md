# web2: the start screen, the lobby and the setup of a game

Status: plan, 2026-10-12, second version. Nothing of web2 is built. The first version was
written before the merge of `felix/main` (commit `6f0eb749`). That merge built a part of this
plan on the server, in another shape, so the decisions and the phases were made again with the
owner on the same day. "What the merge changed" says what is there now.

This is the plan that `2026-10-11-ONLINE_MULTIPLAYER_PLAN.md` names in its last line ("a screen
for the lobby of several humans is the next plan").

## Goal

A game is set up in web2, in single player and in multiplayer, on the same screen and with the
same code. A table of several people is made, filled and started from web2. Today `/` is a
local game on a fixed setup, `?online` starts a game of eight with seven random seats at once
(`dev/OnlineApp.tsx`), and web2 has no screen before a game.

What the owner asked for (2026-10-12):

1. The features of the lobby of `web/` (`web/src/components/Lobby.tsx`, `FactionPicker.tsx`,
   `MapPicker.tsx`, `hooks/useLobbySession.ts`), with a better UI.
2. The host chooses the number of players and the map, in the lobby.
3. The host chooses the set of strategy cards.
4. Factions are not bound to the seat.
5. Seats that the server plays can be added in the lobby, with a wait time that the host sets,
   so that a draft can be tested while the other seats "think".
6. Several seats of one game are played from several tabs of one browser.

## Decisions (owner, 2026-10-12)

Decisions 1, 2, 3, 6, 7, 11 and 16 were made after the merge. They replace the decisions of
the first version with the same numbers.

1. **A player chooses a faction in the lobby, or leaves it random.** This is what the server
   has (`POST …/lobby/seat-faction`): a person chooses for the own seat, the host for any
   seat, and the choice is public. A seat with no choice gets a faction from the seed of the
   game at Start. The first version had another design (two factions drawn after Start, the
   choice hidden); it is not built.
2. **The factions are the 24 of `ti4_engine::seating::LOBBY_FACTIONS`**: the base game and
   Prophecy of Kings. Keleres, the Obsidian and the factions of Thunder's Edge are not
   offered.
3. **The sets of strategy cards are `te` (default), `pok` and `base_game_codex1`**: what
   `ti4_server::card_set::OFFERED` has. `base_game` is not offered: the engine resolves its
   Diplomacy as the card of Codex I, so the text would not be the effect.
4. **The wait time** is one value for the table, for every seat that the server plays, of
   both kinds. The host sets it in the lobby and can change it while the game runs (phase 7).
5. **`/` is the choice between single player and multiplayer.** This replaces decision 5 of
   the online plan ("no mode picker for now"). See "The start screen".
6. **There is no "Ready", for every table.** The host starts the game when every seat is
   filled. `ready` goes from the server, and `web/` loses its Ready button.
7. **The seat order is always random, for every table.** It is drawn at Start from the seed of
   the game. No host moves a seat: `reorder` goes from the server, and `web/` loses its
   controls for it. The lobby lists the players in the order in which they came.
8. **Single player shares the setup of multiplayer**: the player count, the map, the strategy
   cards, the factions, the kinds of the other seats. It is the same screen and the same code
   (phase 6).
9. **Later, not in this plan** (see "Later"): the tables of this browser on the start screen,
   and the MLP bot.
10. **The lobby has a live connection** (a websocket), from the first phase. The page does not
    ask the server again and again. See "The connection of a table".
11. **There is one kind of table.** `web/` and web2 use the same lobby record and the same
    calls. The first version kept a field `setup` (`legacy` or `table`) so that the part of
    `web/` could be removed in one change; `web/` is developed further now, so nothing is
    split. What web2 adds (the socket, the settings) is there for `web/` too.
12. (Dropped. It was about a player who does not choose between two factions. A seat with no
    choice is random now, so nobody holds a table.)
13. **Saved games of single player stay as they are** (one for each address). A list of saved
    games is later.
14. **A visitor of a running game sees the table first** ("Watch", "Rejoin as …"), not the
    game at once.
15. **A table on the server can start from a scenario** ("Start from", as the Dev sheet of a
    local game has it), to test a screen of the late game with several people. It is in
    phase 2.
16. **The rule bot moves into a crate that the wasm host can use**, so that single player has
    it too (phase 5). Today it is in `ti4-server`.
17. **The list of the maps scrolls**, for now. Six players have fourteen maps. The owner
    judges the list when it can be seen.

## How the lobby is built (asked by the owner, 2026-10-12)

- **The lobby looks like the game.** The game screen has the map at the left, a panel at the
  right, and one main button at the bottom of the panel. The lobby uses the same layout: the
  map of the table at the left, the players and the setup in the panel, "Start game" as the
  main button. It is not a form and it has no dialogs. On a phone it is the shell of the
  game too: one pane at a time with the tab bar, and the main button under every pane. See
  "The screen".
- **A change applies at once.** When the host sets the players to 5 or picks a map, or a
  player picks a faction, every player sees it at once. There is no "Apply" button. (In the
  game a choice is first marked and then sent with the main button; the setup of a table does
  not work like that, and `AGENTS.md` gets a line that says so.)

## Non-goals

- A list of all tables of the server, matchmaking, accounts, passwords for a table, chat.
- A spectator mode of its own. "Watch" opens the game with no seat, as today.
- The host history, the event log, restore, redo, the turn redo and the splice of `web/`
  (still postponed).
- Bans, a faction draft in turns, a slice draft (Milty), a choice between two drawn factions.
- Keleres, the Obsidian, the factions of Thunder's Edge, the set `base_game`.
- A map editor. The host chooses a map of the engine, or the random board.
- A list of saved games of single player (decision 13).
- The password screen, the Alpha notice and the feedback sheet of the public server (see
  "Open questions").

## What the merge changed

Read from the code after the merge. The tests were not run for this plan.

Built on the server, and used by `web/`:

- **Seats that the server plays.** `POST …/lobby/bots` `{ kind, count | fill, nickname,
  faction }`, for the host, with the kinds `random` and `smart` (the rule bot, named "Rule
  bot" in every text). `PlayerLobbyMember.bot` keeps the kind; `PlayerSlotView.bot` is public;
  `PlayerLobbyView.bot_kinds` lists what the server offers (`mlp` too when the bot service is
  configured). Such a seat is always "connected" and cannot be taken over. `remove-bot`
  removes it. `fill_seats: "random"` of `POST /api/games` adds the seats through the same
  function and starts.
- **The wait.** `RandomBotDecider::think` (`session/random_bot.rs`) waits before each answer
  of a seat of both kinds: in slices of 20 ms, with no lock held, with a jitter of 0.6 to 1.4,
  and the pick is drawn before the wait, so a game is the same with every wait. A replay never
  reaches it. The value is one for the process: `TI4_RANDOM_BOT_DELAY_MS`, 600 ms when it is
  not set, 30 s at most. `SessionConfig.random_bot_delay_ms` is there for a session, and the
  lobby does not set it.
- **Factions.** `seating::LOBBY_FACTIONS` (24), `seating::lobby_faction_draw(chosen, seed)`,
  `PlayerLobbyMember.faction`, `POST …/lobby/seat-faction { player_id, faction | null }`,
  `PlayerSlotView.faction` and `PlayerLobbyView.factions`. A faction that another seat has is
  refused (409). Every table gets its factions this way, also a table that web2 makes today:
  "seat 1 is Sol" is gone on the server. The map preview of a lobby seats the same factions
  (`maps::catalog::preview_seated`), by the place of the slot.
- **Strategy cards.** `strategy_card_set` of `POST /api/games`, kept in the lobby record and
  in `init.json`, shown in `PlayerLobbyView.strategy_card_set`. It is chosen when the table is
  made and nothing changes it. A table that names none plays `te`.
- **The speaker.** `speaker_order` of `POST /api/games`: `random` (default, from the seed) or
  `seat` (the first seat; fixtures and tests use it).
- **In `ti4-view`** (`map.rs`): `create_game_with_template_seated` and
  `create_game_with_map_seated` (the factions and the card set are given), `lobby_seat_factions`,
  `SpeakerOrder`. In `ti4-engine`: `start_game_seeded_with_card_set`, `seeded_speaker`. The
  wasm host uses none of them: a local game still has the factions by seat and the cards of
  the sources.
- **Maps.** 31 community maps beside the old ones (`maps/community_maps.json`): 6 for three
  players, 5 for four, 6 for five, 14 for six.
- **The public server** (`public/`, `TI4_PUBLIC=1`): a password with a cookie for `/api` and
  `/ws` (`/api/access`), limits of the rate for each address, 4 lobbies for each address, 200
  games, no dev scenarios, no `start_preset`, no MLP bot, a time after which a silent socket is
  closed (`ws_idle_timeout`). The server can serve one built page (`TI4_WEB_DIR`).
- **On the socket of a game**: the messages `reaction_intent_state` and `secondary_preview`.
  `wsTransport.ts` drops a message that it does not know, so they do no harm.

Not changed by the merge:

- A lobby has no socket. `web/` asks every two seconds.
- The player count is fixed when the table is made.
- `start` needs every member ready (`HumansNotReady`). `reorder` exists.
- A table has two phases, `lobby` and `running`.
- A running game does not know the nicknames.
- The server has no "Start from" of the scenarios of `ti4-view`.

What it means for web2 as it is today (phase 0): `?online` names no set and no seat order, so
its games now play the cards of Thunder's Edge with factions from the seed, a random speaker,
and seats that wait 600 ms. No file of `web2/src` names a card of `te`.

## Other findings that shape the plan

Server (`crates/ti4-server`):

- The lobby is HTTP (`http/games.rs`, `session/registry.rs`), with the header
  `x-ti4-player-session`. Under `/api/games/{id}/lobby`: `GET`, `join` (new, or `takeover`),
  `leave`, `heartbeat`, `ready`, `reorder`, `start`, `map`, `map-preview`, `bots`, `add-bot`
  (MLP), `seat-faction`, `remove-bot`. `GET /api/maps?player_count=N` and
  `GET /api/maps/{alias}/preview` are the map catalogue.
- The websocket (`/ws/games/{id}`) answers 404 while a table is a lobby.
- Every change of a lobby is written through one function (`save_player_lobby`), and
  `PlayerLobbyView.lobby_version` counts the changes. So one hook there can tell every
  connection of a table.
- Who is "connected" in a lobby comes from a heartbeat call and a grace time
  (`player_heartbeat`, `PRESENCE_GRACE`).
- `start` gives the players to the game in the order of the slots
  (`start_player_lobby`), and `seat_factions()` is in the same order.
- `ready` and `reorder` are used outside the server by `web/` (`Lobby.tsx`,
  `useLobbySession.ts`, `protocol/types.ts`, `protocol/decode.ts`, the e2e helpers, the smoke
  harness) and by `crates/ti4-bot-agent` (it calls `ready` after it joins).
- The rule bot is `session/smart_bot*.rs` (four files, 8 300 lines). Its entry is a function
  with no state of the server: `smart_bot::pick(choice, seen, rng)`. It uses
  `ti4_policy::fleet_strength` (178 lines, over `ti4-content` and `ti4-model` only). The
  random pick is `random_bot::pick_index` and `decision_rng`. Only `RandomBotDecider` (the
  wait, the pause of a history change) needs the session.
- The server has scenarios of its own (`dev/scenarios.rs`), which start a running game with no
  lobby. They are not the scenarios of a local game (`ti4-view/src/scenario.rs`).

web2:

- `dev/OnlineApp.tsx` is the whole entry of a game on the server, `dev/LocalApp.tsx` of a
  local game. `main.tsx` routes the addresses; `/` is the local game `DEFAULT_GAME`.
- **The player of a game is kept in `localStorage`** under `ti4.online.<game_id>.session`
  (`onlineKeys` in `wsTransport.ts`), and so are the draft, the plan and the staged movement.
  `localStorage` is shared by all tabs. So a second tab of the same game is the same player,
  and two seats in one browser cannot be played.
- The board is drawn from `BoardView` (`model/board.ts`, `game/board/GalaxyMap.tsx`), which is
  made from an update of a game. A preview is a list of `BoardTileView`.
- The frame of the game is `GameShell` over a `GameSession`. It is not checked yet how much of
  it (the header, the footer with the main button, the tab bar of a phone) is free of the
  session.
- A local game is `?local=<seed>&players=8&humans=<mask>` (`ti4_play(seed, players, mask)`),
  and it is saved as its seed and its answers (`SavedGame`, format 1).

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
- `?example=…`, `?gallery`, `?local=…` and `?scenario=…` are untouched. The quick start of a
  game on the server (eight seats, seven random, started at once) stays as
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
│          the map of the table          │ Strategy cards   TE ▾    │
│     (six places of seats, not yet      │ Bots wait      − 1 s +   │
│          given to a player)            │──────────────────────────│
│                                        │ PLAYERS  3 of 6          │
│  ┌ maps for 6 players ──────────── ↻ ┐ │ ★ Ada (you)     Hacan ✎  │
│  │ ▸Standard                         │ │   Bartholomew…  Random   │
│  │  Hyperlanes                       │ │ ⚙ Rule bot 3    Sol   ×  │
│  │  Random                         ↕ │ │   Open   [Rule] [Random] │
│  └───────────────────────────────────┘ │──────────────────────────│
│                                        │ Waiting for 3 open seats │
│                                        │ [      Start game      ] │
└────────────────────────────────────────┴──────────────────────────┘
```

- **One screen, no dialog** (pattern 2). `web/` opens the map picker as a dialog, and the
  faction picker as a list over the page. Here the map is always in view, and so are the
  players and the setup.
- **A control is where the choice is made** (pattern 1). The list of the maps is on the board,
  under the map that it changes. The nickname and the faction are edited in the row of the
  viewer ("✎"). The kinds of a seat that the server plays are in the row of the open seat.
  "×" is in the row of the seat that it removes.
- **The same screen for everyone** (pattern 10). A player who is not the host sees the setup
  with its values and no controls. A visitor with no seat sees the same, with "Join" in the
  footer.
- **A state has one sign and a word on hover** (patterns 4 and 6), in a column of fixed width:
  "★" host, "⚙" the rule bot, "⚄" a random seat, "◇" not connected, an empty outline for an
  open seat. The list has no numbers and no seat colours: the seats are drawn at Start
  (decision 7).
- **The faction of a row** is a word in a column of fixed width: the name of the faction, or
  "Random". It is public (decision 1), so it is not hidden information.
- **Names** (pattern 13): a column of fixed width, cut with "…", the full name on hover; on a
  phone the name takes a second line.
- **Amounts change in steps of one** (pattern 8): the player count is a counter. Beside the
  open seats there is one button for the usual target: "Fill with rule bots".
- **The main button** is in the footer, and Enter is the main button: "Join" for a visitor,
  "Start game" for the host. A player who is not the host has no main button; the note says
  "Waiting for Ada to start". While Start is blocked, the note says why ("Waiting for 3 open
  seats"). Esc leaves the editor of the row.
- **A change of the host is told to the others once**, as a toast ("The host changed the
  map").
- No text that explains the lobby is visible. What the kinds of a seat and the wait time do is
  behind "?" (`Hint`).

Phone (412×923): the same blocks in the panes of the game shell, **Map · Players · Setup**,
with the footer under every pane, so the main button is always in reach. A pane may scroll
down. Nothing is cut at the side with eight seats and the long names.

### The choice of the faction

The row of the viewer opens in place into its editor, as the row of the viewer in the seat
list of a strategy card. It is the only row of a person that opens.

- The editor is one list: "Random" first, then the 24 factions in two groups, "Base game" and
  "Prophecy of Kings" (`LOBBY_BASE_FACTIONS` says where the second begins; the page gets the
  groups from the server or the wasm and writes no name of a faction). A row has the symbol
  and the name. The list scrolls inside the panel; it has too many rows for keys.
- A row is the control and the choice applies at once ("A change applies at once"). The
  chosen row has "▸".
- A faction that another seat has stays in its place, is dimmed and says who has it ("Ada").
  The server refuses it too (409); that answer is a toast.
- The sheet of a faction (the abilities, the leaders, the starting technologies, the starting
  fleet, the home system) is behind its name, as printed, from the catalogue. Rule text is
  not written in web2.
- **The host chooses for a seat that the server plays**: the row of such a seat opens for the
  host, into the same list. web2 gives the host no control over the faction of another person
  (pattern 11: a row of another player is not a control); the server allows it, and `web/`
  has it.
- After Start nothing can be changed. The game shows every faction.

## One table, two pages

`web/` and web2 use the same table (decision 11). So a change of what every table does is a
change of `web/` too, and it is made in one commit with the server.

- **Removed for every table** (decisions 6 and 7): the calls `ready` and `reorder`,
  `HumansNotReady`, `ready` of `PlayerLobbyMember` and of `PlayerSlotView`. A record on disk
  that still has `ready` must load (the field is read and dropped).
- **What goes in `web/` with it**: the Ready button and the controls that move a seat
  (`Lobby.tsx`), their calls (`useLobbySession.ts`), `ready` in `protocol/types.ts` and
  `protocol/decode.ts`, the e2e helpers (`lobbyHelpers.ts`, `lobby_lifecycle.spec.ts`) and the
  smoke harness, which mark every seat ready. `crates/ti4-bot-agent` calls `ready` after it
  joins: the call goes. The home of a seat in the map preview of `web/`
  (`MapPreviewBoard.tsx`: "Seat 2: Hacan") can no longer name a faction, see below.
- **Who changes `web/`** is open: Felix owns it. See "Open questions".
- **New code is in files of its own** where it can be: `session/table.rs` (the settings, the
  draw of the order, "Start from") and `ws/table.rs`. Both pages may use all of it.
- **The order of the seats in tests.** Many tests of the server and the start presets of the
  smoke harness expect that a player sits in the order of the join. `POST /api/games` gets
  `seat_order`: `random` (default) or `join`, the same way as `speaker_order` has `seat` for
  them. It is an argument of the call that makes the table, no page has a control for it, and
  "Start from" uses `join` (a case of a scenario is at a certain seat).

## One setup, two hosts

The setup of a game is decided in Rust, in `ti4-view`, and both hosts call it (decision 8, and
decision 6 of the online plan: one implementation). Most of the parts are there since the
merge; this section puts them behind one type.

- **`ti4-view/src/setup.rs`** (new, small):
  - `TableSetup { player_count, map, strategy_cards, seat_order, speaker_order, start_from,
    seed }`: what a host chooses.
  - `setup_options(player_count)`: the maps that build for the count, the sets of strategy
    cards with their names and the names of their cards, the factions in their groups, the
    kinds of a seat, the scenarios.
  - `preview(&TableSetup)`: the tiles of the board and the places of the seats, with no
    faction.
  - `draw(&TableSetup, seats)`: the seat order from the seed (a domain of its own in
    `GameRng`), then the factions with `lobby_faction_draw`. A chosen faction stays with its
    player.
  - `create(&TableSetup, drawn)`: calls `create_game_with_template_seated`, with the card set
    and the speaker.
- **The server** calls these from `start` and from the lobby calls. Its own code for the
  preview and the catalogue (`maps/catalog.rs`, `map.rs`) moves onto them.
- **The wasm host** gets exports for the same functions and a way to start a game from a
  setup (phase 5). `ti4_play(seed, players, mask)`, the scenarios and the fixtures keep the
  game of today (the factions by seat, the cards of the sources, the first seat as speaker),
  so no seeded game changes.
- **The page** has `Table` in `session/`, the seam of the setup as `Transport` is the seam of a
  game: the table as it is (the players, the setup, the phase), the preview, and the calls (set
  a value of the setup, add or remove a seat, choose a faction, start). `serverTable` talks to
  the server (a socket and the HTTP calls); `localTable` (phase 6) keeps the table in the page
  and asks the wasm. The screen and `select/lobby.ts` do not know which one they have, and
  `boundaries.test.ts` holds it.

## The connection of a table

A table has a websocket of its own from the moment it is made (decision 10):
`/ws/tables/<game_id>`. It is not the socket of a game. `web/` can use it in place of its
two-second call.

- The page sends `Subscribe` with its `player_session` (or none, as a visitor). The server
  answers with the table as that viewer may see it, and sends it again after every change:
  a player who came or left, a faction, a value of the setup, the phase. The message is the
  lobby view, whole; a table is small.
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
- **The public server.** The socket is under `/ws`, so the middleware `public::guard` must
  cover it as it covers the socket of a game (the password, the limits): a test in
  `tests/it/public_playtest.rs`. A lobby is silent for minutes, and `ws_idle_timeout` closes
  a silent socket: the page sends a ping in a shorter interval, as the game socket does.

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
- The public server allows 4 lobbies for each address. Several seats of one table are one
  lobby, so the limit is not reached by this.

## Phases

Each phase leaves `?local=…`, `?online&fill=random` and `web/` working and green.

### Phase 0: web2 on the merged server (web2)

Before anything is built: the games of `?online` changed under web2 (the cards of `te`, the
factions from the seed, a random speaker, a wait of 600 ms).

1. Build the server and the wasm again (`cargo build -p ti4-server --bin server`,
   `scripts/build-wasm.sh`), then run `npm test` and the e2e. List what fails before fixing.
2. **The wait in tests.** `playwright.config.ts` and the server of `transportContract.test.ts`
   set `TI4_RANDOM_BOT_DELAY_MS=0`, or the suites wait 600 ms for each decision of seven
   seats.
3. **The cards of `te`.** Play a game of each of the three sets: the action picker, the seat
   list of a strategic action and the draft of a secondary must name `te4construction`,
   `te6warfare` and `base4` from the catalogue, not by an id. Check `session/select/strategy.ts`
   and `names.ts`.
4. **The 24 factions.** A game of eight on several seeds: the symbol and the colour of a seat,
   the name, the rows that only some factions have (faction technologies, the flagship, the
   mech). Play each faction once in a test of the selectors, so that no screen shows an id.
   The seat of the viewer is found by the player, never by the place (check
   `select/players.ts` and the mock data for "seat 1 is Sol"). The Ghosts of Creuss have their
   home off the map.
5. **The speaker** is not the first seat: check the first screen of a game (who the game waits
   for) and the tests that expect the page to draft first.
6. **The local game.** The wasm test of the merge has one answer less (the game no longer asks
   "negotiate, or continue" there). Check that a saved game of before still plays, or say in
   the page that it is of another engine (the page has that notice). Bring the line of
   `AGENTS.md` ("Local play") up to date.
7. Record the fixtures again with their commands where the facts changed.

### Phase 1: the start screen, the lobby, and its connection (web2, a small part of the server)

The server gets the socket of a table and nothing else. The lobby has what the server can do
today, which is more than before the merge.

1. **The start screen** ("The start screen"): `/` is the choice; "Single player" loads the
   local game of today. The tests that open the local game at `/` get its address.
2. **Routing.** `?online` makes a table (`POST /api/games` with no `fill_seats`; the player
   count is the last one that this browser used, else four, as `web/`) and goes to
   `?online=<game_id>&seat=<player_id>`. `?online=<game_id>` asks the table: the phase `lobby`
   shows the lobby, `running` mounts the game as today, 404 is "The server no longer knows
   this game."
3. **The socket of a table** on the server ("The connection of a table"): `ws/table.rs`, the
   channel of a table in the registry, presence from the open socket.
   `tests/it/table_socket.rs`: a second connection is told when a player joins, leaves, picks
   a faction and when the map changes; a visitor gets the view with no credential in it; a
   closed socket is "not connected" after the grace time; the public server asks for the
   password.
4. **`session/table.ts`**: the `Table` seam and `serverTable`: the socket for the table as it
   is, the HTTP calls that the server has for the changes. Of two views the one with the
   higher `lobby_version` is kept. A failed call is a toast with the reason; the lobby stays
   as it was. A lost socket is "Offline · reconnecting", and the page tries again by itself.
5. **`session/select/lobby.ts`** makes a `LobbyView` (in `model/`): the rows of the players
   with their signs and their faction, the setup rows with `editable`, the footer (the main
   button, the note). The components decide nothing from names (as `ScoreRowView.open`).
6. **The screen**: the list of the players, the setup block (the player count and the set of
   strategy cards are shown and cannot be changed in this phase), the footer, the header with
   "Copy link" and "Start". The map is the preview of `GET …/lobby/map-preview`, read-only,
   asked again when `map_revision` changes.
   - Check first how much of `GameShell` is free of a `GameSession` (the header, the footer,
     the tab bar of a phone). Take the frame out as a component that both use; do not copy it.
   - Check how `GalaxyMap` can draw a list of `BoardTileView` with no game. A `BoardView` with
     the tiles only, made in `select/lobby.ts`, is the aim; the camera and "Fit board" then
     work as in the game.
7. **Seats that the server plays.** The row of an open seat has one button for each kind of
   `bot_kinds` that is played in the process ("Rule bot", "Random"; `mlp` is not offered,
   see "Later"), and under the list "Fill with rule bots" (`POST …/lobby/bots`). Such a seat
   has its sign and "×" (`remove-bot`).
8. **A seat for each tab**, as the section above.
9. **The nickname**: kept in `localStorage` (`ti4.nickname`), sent with `POST /api/games` and
   `join`. In this phase it is edited before the join only; phase 2 adds the rename.
10. **No "Ready"** (decision 6). Until phase 2 the server asks for it, so the page marks its
    player ready when it joins and shows nothing of it.
11. **The start**: when the table says `running`, every tab mounts the game with its player.
    (In this phase the seats are in the order of the join.)
12. `AGENTS.md`: a section "Lobby" with the rules of "The screen", and the changed lines of
    "Online play" and "Local play".

Tests: `select/lobby.test.ts` (rows and signs for the host, a player, a visitor; the reasons of
a blocked start); `table.test.ts` over a socket of the test and a fake `fetch` (an older view
is dropped, a failed call, a lost socket); a test of the keys (two players of one browser do
not share a draft; the old key is moved); `e2e/lobby.spec.ts` against the real server: **one
browser context with three pages** makes a table of four from the start screen, joins from two
more tabs, adds a rule bot, starts, and each page is another seat of the running game; a tab
sees a player who joins with no reload and no wait; a reload of a page keeps its seat. A phone
case: nothing is cut at the side, and the main button is in view.

### Phase 2: no Ready, the random order, the settings of a table (Rust, `web/`, web2)

The first half changes every table and is one commit with `web/` ("One table, two pages").
After this phase a draft can be tested alone: one or two tabs, the other seats played by the
server, with time to draft.

Server, for every table:

1. **No "Ready"** (decision 6): `start` needs every seat filled and nothing else. The call
   `ready`, `HumansNotReady` and the field go. `web/`, the smoke harness and
   `ti4-bot-agent` are changed with it.
2. **The seat order is drawn at Start** (decision 7): the occupants of the slots are shuffled
   with a stream of the seed of the game (a domain of its own in `GameRng`, as
   `seating:lobby-faction-draw:v1`), then the factions are drawn for the new order; a chosen
   faction stays with its player. `reorder` goes. `seat_order: "join"` of `POST /api/games`
   keeps the order of the join for tests and presets.
3. **The preview names no faction.** A home of the preview is a place of a seat: nobody knows
   before Start who sits there. `SeatPreview.faction` and `faction_name` go, and the home tile
   is sent as a place. Read `maps/build.rs` (`placeholder_homes`, `preview_seated`): whether a
   template builds must not depend on which faction sits where, or the preview and the start
   must use the same rule (the Ghosts of Creuss change the board). `web/` shows the place
   with its number only.

Server, new:

4. **`POST …/lobby/settings`** (host, phase `lobby`), each field optional:
   - `player_count` (2 to 8). More seats are added open. Fewer seats: the open seats go first,
     then the seats that the server plays (with their chosen factions); a seat of a person is
     never removed (`409` with the names that are in the way). The map of a table is for one
     count: the choice becomes the default map of the new count, and `map_revision` goes up.
     The host's own choice of a random board stays.
   - `strategy_card_set` (one of `card_set::OFFERED`). Today it is set when the table is made
     only.
   - `bot_delay_ms` (0 to 30 000), see item 6.
   - `start_from`, see item 7.
   - `POST …/lobby/nickname` renames the player of the credential.
5. `PlayerLobbyView` gets `bot_delay_ms`, `seat_order` and `start_from`. The record on disk
   gets the same fields with a default, so a table of today loads.
6. **The wait of a table** (decision 4). The mechanism is there (`RandomBotDecider::think`).
   What is missing:
   - the value of the table: the lobby record keeps it, `start` gives it to
     `SessionConfig.random_bot_delay_ms`, and `init.json` keeps it, so a recovered game has
     it. A table with no value uses the value of the process, as today;
   - a value that can change while the session runs (phase 7 sets it): the decider reads it
     for each decision, not once when the session starts;
   - **what a seat waits for.** It waits before every answer today. The owner asked that a
     reaction window that the seat only declines does not wait. Read `answer` and `pick_index`:
     if the decider cannot tell such a decision from the others, measure a round of eight with
     a wait of one second before choosing a rule;
   - the update must say who the game waits for while a seat waits (`bot_deciding` of
     `SessionShared` is there; check what `TurnStatus` says then): the page says "Waiting for
     Rule bot 3", as for a person;
   - tests: the same game with a wait of 0 and of 50 ms; an undo while a seat waits is not
     held up (the pause of a history change is there, `pause_bots`).
7. **"Start from"** (decision 15). `start_from` of `lobby/settings`: none ("Round 1"), or the
   id of a scenario of `ti4-view` (`scenario::find`, `scenario::apply`), the same list that
   the Dev sheet of a local game has. `GET /api/setup-options` lists them with what each
   holds.
   - A scenario is a game of its own: it has its seed, its number of seats, its board and
     its factions. So the table takes the player count of the scenario, and the map, the
     strategy cards and the factions are not asked: their rows show the values of the
     scenario with no control.
   - The seats are not drawn: a player sits in the order of the join, the host first. Open
     seats are filled with seats that the server plays, as in every table.
   - Read how the wasm host starts a scenario (`ti4_play_scenario`) and use the same function
     of `ti4-view`; the server keeps the first state in `init.json` as for every game, so a
     recovered game is the scenario. The scenarios of `dev/scenarios.rs` are not touched.
   - It is refused on the public server, as `start_preset` is.
   - It is a tool for testing and it is not hidden: the row is in the setup of every host,
     and the lobby says "Start from: Late game" to every player.

web2:

8. The setup rows get their controls for the host: the counter of the players ("− 6 +"; "−" is
   blocked with the reason when a person is in the way), the list of the sets of strategy
   cards (the name; the cards that differ from the default behind hover; the names come from
   `GET /api/setup-options`, the page writes none), the counter of the wait in steps of half a
   second ("Off" at zero).
9. The setup row "Start from" is a list: "Round 1" and the scenarios, with what a scenario
   holds behind "?", as in the Dev sheet.
10. The nickname is edited in the row of the viewer at any time before Start.
11. The page no longer sends `ready`. `?online&fill=random` sends `fill_seats` as today, and
    can name `wait=<seconds>`.

Tests: `tests/it/lobby_settings.rs` (grow and shrink; a person in the way; the map follows the
count; the card set; only the host), `tests/it/seat_order.rs` (the order is the same for a
seed and another for another seed; a chosen faction stays with its player; `join` keeps the
order; a start with a seat that is not ready is no longer a thing), the tests of the wait in
`tests/it/random_bots.rs`, `tests/it/table_scenario.rs` (a table of two people and bots starts
the late game; each person has the seat of the join; a recovered game is the same; refused on
the public server); `player_lobby_admission.rs` and the other tests that call `ready` or
`reorder` are brought up to date; the fixture test, the lobby e2e and the smoke harness of
`web/` are green. web2: the selectors, and in `e2e/lobby.spec.ts` the case of the owner: two
tabs and four rule bots with a wait of one second, the secondary of a card is drafted in one
tab while the bots wait, and the toolbar says who the game waits for. A second case starts
"Late game" with two tabs and opens a sheet of the late game in each.

### Phase 3: the choice of the faction (web2)

The server has all of it (`seat-faction`, `factions` of the view).

1. The editor of the row of the viewer, as "The choice of the faction": the list in two
   groups, "Random" first, a taken faction in its place with who has it.
2. The row of a seat that the server plays opens for the host, into the same list.
3. The sheet of a faction comes from the catalogue of the wasm (`Transport.catalog` has the
   abilities, the leaders and the technologies as printed); check what a faction sheet needs
   that the catalogue does not have (the starting fleet, the starting technologies, the home
   system, the commodities) and add it in `ti4-view`, from `ti4-content`.
4. The groups of the factions come from `GET /api/setup-options` (phase 2); until then from
   `PlayerLobbyView.factions`, as one list.

Tests: the selector of the editor (the viewer, the host over a bot, a taken faction, a player
who is not the host has no control over another row); `e2e/lobby.spec.ts`: two tabs pick two
factions, each tab sees the other's at once, a tab that picks a taken faction gets the toast,
a seat left at "Random" gets a faction that no other seat has, and each tab then plays the
faction that it chose. A shot with eight seats and the open editor at 1920×1080 with no
vertical scroll of the page, and on the phone.

### Phase 4: the map (web2)

1. The list of the maps on the board, for the host: one row for each map of
   `GET /api/maps?player_count=N` and "Random", with the name, the author and what
   `MapChoiceView` says (the number of systems, hyperlanes, "recommended"). The chosen one has
   "▸". A click saves it (`POST …/lobby/map`) and the board shows it. "↻" draws the open
   places again (the same call: each one draws a new seed). Read
   `web/src/presentation/mapPicker.ts` for the order and the words; do not import it.
2. **The list scrolls** (decision 17): it has a fixed height on the board and scrolls inside,
   the recommended maps first. This is an exception to pattern 9 until the owner has seen it
   with the fourteen maps of six players; groups ("Official", "Community") are the next thing
   to try.
3. A player who is not the host has no list: the name of the map is in the heading of the
   board. The map of the lobby is asked again when `map_revision` changes, and the change is a
   toast.
4. The board marks the home places of the seats. Before Start no player is on a place.
5. Looking at another map before choosing it (`GET /api/maps/{alias}/preview`) is not built:
   a choice applies at once and can be changed until Start, so the board is the preview.
6. The start preset of `web/` (dev, `combat`) is `&preset=` of the address that makes the
   table. No control. The public server refuses it.

Tests: the selector of the list (host and not host); `e2e/lobby.spec.ts`: the host chooses a
map, the other tab shows it and the toast, the started game has that map; a shot of the lobby
with eight seats and the long names at 1920×1080 with no vertical scroll of the page
(`e2e/shots.spec.ts`), a shot with six seats and the long list, and both on the phone.

### Phase 5: the setup in one place, and the bots in a crate of their own (Rust)

1. **`ti4-view/src/setup.rs`**, as "One setup, two hosts". The server moves `start`, the
   preview, the catalogue and `GET /api/setup-options` onto it; the draw of the order of
   phase 2 moves into `draw`. Guard: every fixture of web2 and of the server is the same byte
   for byte, and a table of a seed starts the same game as before the move.
2. **A crate for the seats that are played by code** (decision 16), `crates/ti4-bots`, over
   `ti4-model`, `ti4-content` and `ti4-engine`:
   - the rule bot (`smart_bot.rs`, `smart_bot_agents.rs`, `smart_bot_faction_plans.rs`,
     `smart_bot_objectives.rs`) and the random pick (`pick_index`, `weight`, `decision_rng`)
     move there, with their tests. The move changes no line of what a bot decides: the survey
     tests of the server (`smart_bots.rs`, `random_bots.rs`) are the guard;
   - `fleet_strength` is used from `ti4-policy` today. `ti4-policy` is not one of the web
     crates. Check that it builds for `wasm32`; if not, or if it is heavy, move the module
     into the new crate and let `ti4-policy` use it from there;
   - `RandomBotDecider` stays in the server: the wait, the pause of a history change and the
     counter are of a session;
   - the crate joins the aliases `c-web`, `l-web` and `t-web` (`.cargo/config.toml`) and the
     list of `AGENTS.md` ("Rust builds");
   - **Felix changes these files every day.** A move of 8 300 lines breaks every open branch
     of his. It is one commit with nothing else in it, made at a moment that he names.
3. **The wasm host** exports the functions of `setup.rs` (`ti4_setup_options`,
   `ti4_setup_preview`, `ti4_setup_draw`), starts a game from a setup (`ti4_play_setup`), and
   lets a seat that is not played in the page be played by the rule bot or by the random pick.
   `ti4_play(seed, players, mask)` stays as it is.
   - The same game on both hosts: a table of a seed, with the same seats played by the same
     kinds, is the same game in the wasm and on the server. A test plays one round of both.
   - Measure the size of `ti4.wasm` and the time of a decision of the rule bot in the browser
     before and after (`crates/ti4-wasm/README.md` has the numbers of today).

Tests: `ti4-view` (the same seed gives the same table; a chosen faction is kept; every seat has
another faction); `ti4-bots` (the tests that move); the test of both hosts.

### Phase 6: single player uses the same setup (web2, a small part of the wasm)

1. **`localTable`** in `session/`: a `Table` that lives in the page. Its players are "You" and
   seats that the page plays (the rule bot, or random); the host is the page. It has the
   player count, the map, the strategy cards, the factions, the wait time (the page waits
   before it lets a seat decide; check against "Other seats: Step" of the Dev sheet, which is
   the same idea). It has no link to copy, no nickname to edit and nobody to wait for.
2. **"Single player" of the start screen** opens the lobby screen over `localTable`, when
   this browser has no saved game of single player; with one it opens that game, as today
   (decision 13). "New game" of the Dev sheet opens the setup.
3. **The saved game** gets the setup: `SavedGame` format 2 has the setup beside the seed and
   the answers; format 1 is read as the game of today. The address of such a game names the
   save, not the setup (a setup does not fit an address). Export and Import keep working.
4. "Play as" and several seats in the page (the mask) stay tools of the Dev sheet.
5. The row "Multiplayer" of the Dev sheet stays the quick start.

Tests: the contract of `Table` runs against `serverTable` (a real server) and `localTable` (the
wasm), as `transportContract.test.ts` does for a game: the options, a change of the count, the
preview, a faction, a bot, the start. `savedGame.test.ts` (format 1 and 2).
`e2e/local.spec.ts`: from the start screen, a game of four on another map and the `pok` set
with three rule bots, a faction is chosen, the game is played for a turn, a reload continues
it.

### Phase 7: the table in the game (Rust + web2)

1. **Nicknames in the game.** The snapshot and the update get the seats of the table as a
   fact: the nickname and who plays the seat (a person, the rule bot, random), per player id.
   It is public. The page names a seat by its nickname everywhere it says "Seat PLAYER_1"
   today (the player table, the toolbar status, the seat list of a strategic action, the
   sheets), in the column of fixed width; a seat that the server plays has its sign.
2. **The sheet "Table"** in "Reference and more": the lobby screen, read-only (pattern 10),
   for the running game: who sits where, who is connected, the map by its name, the set of
   strategy cards. The question that it answers: "who is at this table, and how was it set
   up?" For the host it has the counter of the wait
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
  of its own, the advisor, libtorch; not on the public server). web2 does not offer it.
- A list of the saved games of single player.
- **A choice between two drawn factions**, as the first version of this plan had it, as a
  setting of a table.
- **More of the setup** (ideas, owner 2026-10-12: nice to have). Each is one more row of the
  setup block and one more field of `TableSetup`; none is checked against the engine yet.
  - The points that win (10, or 14 for a long game).
  - The speaker (`speaker_order` is there: `random` or the first seat).
  - The content of the game: with or without Thunder's Edge and the codices. The engine has
    the source sets `BASE`, `POK` and `FULL`; every host starts a game with `POK` today.
  - The factions that can be drawn (leave some out), Keleres.
  - House rules of the seats: whether the undo of a seat is allowed, whether a visitor may
    watch.

## Checks that every phase must pass

- Rust (phases 1, 2, 5, 7): `cargo c-web`, `cargo t-web`, `cargo fmt --all -- --check`,
  `cargo l-web`; server tests with `cargo test -p ti4-server --test it <file>::`. Do not build
  other crates, except `ti4-bot-agent` in phase 2 (it calls `ready`) and `ti4-policy` in
  phase 5 (`fleet_strength`).
- `PROTOCOL_VERSION`: a phase adds optional fields, calls and a socket. Phase 2 removes
  `ready` from the lobby view and two calls; the lobby view is HTTP and `web/` is changed in
  the same commit. Check `web/src/protocol/decode.ts` for a decoder that refuses a field that
  it does not know or misses.
- The records on disk (`PlayerLobbyRecord`, `PlayerGameInitRecord`) refuse unknown fields: a
  new field has a default, a removed field is still read, and a table of today loads.
- `web/` stays green: its unit tests, its lobby e2e and one run of the smoke harness after
  phase 2.
- Rebuild `session/ti4.wasm` (`scripts/build-wasm.sh`) in phase 0 and after phases 5 and 6.
- web2: `npm test`, `boundaries.test.ts` (the calls of a table are in `session/`; `game/` and
  `ui/` do not know the server), the e2e on desktop and phone. A changed screen is checked with
  eight seats and the two long names. Format only the files that changed (`npx oxfmt <files>`).
- The owner looks at the lobby on the phone after each phase.

## Risks

- **A change of every table breaks `web/` and Felix's branches.** Phase 2 (no Ready, the
  random order, the preview with no faction) and phase 5 (the move of the bots) touch code
  that he works on. Each is one commit that he knows of before it is made.
- **Hidden information before the game.** The seed of a table never leaves the server: it
  tells the seat order and the faction of every seat that is left at "Random". The seed is in
  the address only for development (`&seed=`), and the public server must refuse it there if
  web2 is served by it. A field of the lobby view is read by a visitor with no credential.
- **Tests that expect the order of the join.** After phase 2 the first player is not the
  first seat. `seat_order: "join"` is for them; a test that should have the real behaviour
  reads its seat from the view.
- **A wait that holds a lock** would stop the drafts that it is there to test. The decider
  waits with no lock today; the test "an undo while a seat waits" and the e2e of the secondary
  draft are the guard when the value becomes one of the table.
- **Two tabs, one player.** A duplicated tab is the same player twice, and both write the same
  draft key. The server allows it; the page does not guard against it. Say so in `AGENTS.md`.
- **Seeded games change** if the setup of today changes. The fixtures prove that it does not;
  `ti4_play`, the scenarios and `seat_order: "join"` keep the old games.
- **Twenty-four factions and three sets on every screen.** web2 was looked at with the six
  factions by seat and the cards of `pok`. Phase 0 plays each faction and each set once.
- **The rule bot in the browser.** It was written for the server: its size in the wasm and
  its time for a decision are not known. Phase 5 measures both before single player depends
  on it.

## Open questions (owner)

1. **Is web2 served by the public server?** Open (owner, 2026-10-12). If yes, web2 needs the
   password screen (`/api/access`), and the plan must refuse `&seed=` there. What the plan
   does in any case: the socket of a table is behind the password and the limits, and
   "Start from" and the preset are refused on the public server.
2. **Who changes `web/` in phase 2**, and when: Felix, or this branch with his agreement? The
   same for the move of the rule bot in phase 5.
3. **The list of the maps** (decision 17): the owner judges the scrolling list when phase 4
   shows it.

## Order and size

Phase 0 comes first: it says whether web2 works on the server of today. Phase 1 is next: every
other phase puts a control on its screen. Phase 2 gives the owner the tools to test drafts,
and it needs the agreement about `web/`; its second half (the settings) does not, and can be
built first if that agreement takes time. Phases 3 and 4 are web2 only and independent of each
other. Phase 5 is Rust only and can run beside them. Phase 6 needs 5. Phase 7 is last.

| Phase | Where | Size |
| --- | --- | --- |
| 0 web2 on the merged server | web2 | small to medium (not known before the tests are run) |
| 1 the start screen, the lobby, its socket, bots, a seat for each tab | web2, ti4-server (the socket of a table) | large (two new screens, the frame taken out of `GameShell`, the keys, the addresses of the tests) |
| 2 no Ready, random order, settings, the wait of a table, "Start from" | ti4-server, `web/`, ti4-bot-agent, web2 | large (many tests of the server and of `web/` change) |
| 3 the choice of the faction | web2, ti4-view (facts of a faction sheet) | medium |
| 4 the map | web2 | medium |
| 5 one setup, the crate of the bots, the wasm exports | ti4-view, ti4-bots (new), ti4-server, ti4-wasm | large (a big move, little new code) |
| 6 single player on the same setup | web2, ti4-wasm | medium |
| 7 the table in the game | ti4-server, ti4-view (a fact), web2 | medium |
