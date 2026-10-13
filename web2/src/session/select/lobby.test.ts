import { expect, it } from "vitest";
import type { TableState } from "../table";
import type { BoardTileView, Catalog, LobbySlot, LobbyState, SetupOptions } from "../wire";
import { type LobbyLocal, hostChange, nextNickname, selectLobby } from "./lobby";

const LONG = "Bartholomew Featherstonehaugh the Third of Mecatol";

const slot = (position: number, change: Partial<LobbySlot> = {}): LobbySlot => ({
  slot_id: `slot_${position}`,
  position,
  occupant: null,
  nickname: null,
  connected: false,
  can_take_over: false,
  ...change,
});
const person = (position: number, id: string, nickname: string, change: Partial<LobbySlot> = {}) =>
  slot(position, { occupant: id, nickname, connected: true, ...change });

/** A table of six: the host, a player with a long name, a rule bot, and three open seats. */
const lobby = (change: Partial<LobbyState> = {}): LobbyState => ({
  game_id: "g1",
  phase: "lobby",
  host_player_id: "ada",
  slots: [
    person(1, "ada", "Ada", { faction: "hacan" }),
    person(2, "bart", LONG),
    slot(3, {
      occupant: "bot1",
      nickname: "Rule bot 1",
      bot: "smart",
      faction: "sol",
    }),
    slot(4),
    slot(5),
    slot(6),
  ],
  lobby_version: 5,
  bot_kinds: ["random", "smart"],
  map: {
    kind: "template",
    alias: "std6",
    name: "Standard",
    systems: 37,
    hyperlanes: false,
    recommended: true,
  },
  map_revision: 1,
  strategy_card_set: "te",
  speaker_order: "random",
  factions: ["sol", "hacan"],
  seat_order: "random",
  bot_delay_ms: 600,
  ...change,
});
const card = (id: string, name: string, initiative: number) => ({ id, name, initiative });
/** What the server says a host can choose (`GET /api/setup-options`), cut to two cards a set. */
const OPTIONS: SetupOptions = {
  min_players: 2,
  max_players: 8,
  strategy_card_sets: [
    {
      id: "te",
      name: "Thunder's Edge",
      default: true,
      cards: [card("leadership", "Leadership", 1), card("te4construction", "Construction", 4)],
    },
    {
      id: "pok",
      name: "Prophecy of Kings",
      default: false,
      cards: [card("leadership", "Leadership", 1), card("pok4construction", "Construction", 4)],
    },
    {
      id: "base_game_codex1",
      name: "Base game with Codex I",
      default: false,
      cards: [card("leadership", "Leadership", 1), card("base4", "Construction", 4)],
    },
  ],
  faction_groups: [],
  bot_kinds: [
    { id: "smart", name: "Rule bot", description: "The server plays this seat by rules." },
    { id: "random", name: "Random", description: "The server plays this seat at random." },
  ],
  max_bot_delay_ms: 30_000,
  default_bot_delay_ms: 600,
  scenarios: [
    { id: "late-game", title: "Late game", description: "Round 4.", seed: 42, players: 8 },
  ],
};
const of = (
  state: LobbyState,
  viewer: string | null,
  options: SetupOptions | null = OPTIONS,
): TableState => ({ lobby: state, viewer, preview: null, options });
const table = (viewer: string | null, change: Partial<LobbyState> = {}): TableState =>
  of(lobby(change), viewer);
const local = (change: Partial<LobbyLocal> = {}): LobbyLocal => ({
  mine: [],
  inspected: null,
  offline: false,
  ...change,
});
const catalog = {
  components: {
    faction_sheets: { hacan: { name: "Hacan", title: "The Emirates of Hacan", commodities: 6 } },
  },
} as unknown as Catalog;

it("the host sees the rows with their signs, the kinds of an open seat and the removal", () => {
  const view = selectLobby(table("ada"), local({ mine: ["ada"] }), catalog);
  expect(view.title).toBe("Lobby · 6 players");
  expect(view.filled).toBe("3 of 6");
  expect(view.rows.map((row) => [row.sign?.kind ?? null, row.name, row.faction])).toEqual([
    ["host", "Ada", "Hacan"],
    [null, LONG, "Random"],
    // Without the catalogue entry a faction is its id in words, never the bare id of a seat.
    ["rule", "Rule bot 1", "Sol"],
    ["open", "Open", ""],
    ["open", "Open", ""],
    ["open", "Open", ""],
  ]);
  expect(view.rows[0].you).toBe(true);
  expect(view.rows[0].remove).toBeNull();
  expect(view.rows[1].remove?.intent).toEqual({ type: "remove", player: "bart" });
  expect(view.rows[2].remove?.label).toBe("Remove Rule bot 1");
  // An open seat has one button for each kind that the server plays, the rule bot first.
  expect(view.rows[3].buttons.map((button) => [button.label, button.intent])).toEqual([
    ["Rule bot", { type: "addBot", kind: "smart" }],
    ["Random", { type: "addBot", kind: "random" }],
    ["Join in a new tab", { type: "joinTab" }],
  ]);
  expect(view.rows[4].buttons.map((button) => button.label)).toEqual(["Rule bot", "Random"]);
  expect(view.fill?.intent).toEqual({ type: "fillBots", kind: "smart" });
  expect(view.help.map((item) => item.title)).toEqual(["Rule bot", "Random"]);
});

it("the start waits for the open seats, and says so", () => {
  const view = selectLobby(table("ada"), local(), null);
  expect(view.footer).toEqual({
    note: "Waiting for 3 open seats",
    main: { label: "Start game", disabled: true, intent: { type: "start" } },
    nickname: false,
  });
  const one = lobby();
  one.slots[3] = person(4, "c", "Cy");
  one.slots[4] = person(5, "d", "Di");
  expect(selectLobby(of(one, "ada"), local(), null).footer.note).toBe("Waiting for 1 open seat");
});

it("a full table starts: nobody says Ready", () => {
  const full = lobby();
  full.slots[3] = person(4, "c", "Cy");
  full.slots[4] = person(5, "d", "Di", { connected: false });
  full.slots[5] = slot(6, { occupant: "bot2", nickname: "Random bot 1", bot: "random" });
  const view = selectLobby(of(full, "ada"), local(), null);
  expect(view.footer).toEqual({
    note: "",
    main: { label: "Start game", disabled: false, intent: { type: "start" } },
    nickname: false,
  });
  expect(view.fill).toBeNull();
  expect(view.rows[5].sign).toEqual({ kind: "random", word: "Random: played by the server" });
});

it("the names of the kinds of a seat are those of the server: none is offered before it said them", () => {
  const view = selectLobby(of(lobby(), "ada", null), local(), null);
  expect(view.rows[3].buttons.map((button) => button.label)).toEqual(["Join in a new tab"]);
  expect(view.fill).toBeNull();
  expect(view.help).toEqual([]);
  expect(view.rows[2].sign).toEqual({ kind: "rule", word: "Played by the server" });
  // A kind that the server plays in another process is not offered.
  const mlp = selectLobby(table("ada", { bot_kinds: ["random", "smart", "mlp"] }), local(), null);
  expect(mlp.rows[4].buttons.map((button) => button.label)).toEqual(["Rule bot", "Random"]);
});

it("a player who is not the host has the same rows, no controls of the table and no main button", () => {
  const view = selectLobby(table("bart"), local({ mine: ["bart"] }), null);
  expect(view.rows.map((row) => row.name)).toEqual(
    selectLobby(table("ada"), local(), null).rows.map((row) => row.name),
  );
  expect(view.rows[1].you).toBe(true);
  expect(view.rows.every((row) => row.remove === null)).toBe(true);
  // Several seats of one browser: the first open seat can be joined from another tab.
  expect(view.rows.map((row) => row.buttons.map((button) => button.label))).toEqual([
    [],
    [],
    [],
    ["Join in a new tab"],
    [],
    [],
  ]);
  expect(view.fill).toBeNull();
  expect(view.help).toHaveLength(2);
  expect(view.setup.every((row) => row.control === null)).toBe(true);
  expect(view.rows.map((row) => row.rename)).toEqual([false, true, false, false, false, false]);
  expect(view.footer).toEqual({ note: "Waiting for 3 open seats", main: null, nickname: false });

  const full = lobby();
  for (const index of [3, 4, 5]) {
    full.slots[index] = person(index + 1, `p${index}`, `Player ${index}`);
  }
  expect(selectLobby(of(full, "bart"), local(), null).footer.note).toBe("Waiting for Ada to start");
});

it("a visitor joins with a name, while a seat is open", () => {
  const view = selectLobby(table(null), local(), null);
  expect(view.rows.every((row) => !row.you && row.buttons.length === 0 && !row.remove)).toBe(true);
  expect(view.footer).toEqual({
    note: "3 open seats",
    main: { label: "Join", disabled: false, intent: { type: "join" } },
    nickname: true,
  });
  const full = lobby();
  for (const index of [3, 4, 5]) {
    full.slots[index] = person(index + 1, `p${index}`, `Player ${index}`);
  }
  expect(selectLobby(of(full, null), local(), null).footer).toEqual({
    note: "Every seat is taken",
    main: { label: "Join", disabled: true, intent: { type: "join" } },
    nickname: false,
  });
});

it("a tab with no seat marks the players of this browser, and a row makes the tab that player", () => {
  const view = selectLobby(table(null), local({ mine: ["ada", "bart"] }), null);
  expect(view.rows.map((row) => row.become)).toEqual([
    { type: "become", player: "ada" },
    { type: "become", player: "bart" },
    null,
    null,
    null,
    null,
  ]);
  // The row of the tab itself is not such a control.
  const own = selectLobby(table("ada"), local({ mine: ["ada", "bart"] }), null);
  expect(own.rows.map((row) => row.become?.type ?? null).slice(0, 2)).toEqual([null, "become"]);
});

it("a player whose socket is closed has the sign of it, and the viewer never has", () => {
  const away = lobby();
  away.slots[0] = person(1, "ada", "Ada", { connected: false });
  away.slots[1] = person(2, "bart", LONG, { connected: false });
  const seen = selectLobby(of(away, "bart"), local(), null);
  expect(seen.rows[0].sign).toEqual({ kind: "away", word: "Host · not connected" });
  expect(seen.rows[1].sign).toBeNull();
  const visitor = selectLobby(of(away, null), local(), null);
  expect(visitor.rows[1].sign).toEqual({ kind: "away", word: "Not connected" });
});

it("the setup says the players, the map, the strategy cards, the wait and where the game starts", () => {
  const view = selectLobby(table("bart"), local(), null);
  expect(view.setup.map((row) => [row.label, row.value])).toEqual([
    ["Players", "6"],
    ["Map", "Standard · 37 systems"],
    ["Strategy cards", "Thunder's Edge"],
    ["Bots wait", "0.6 s"],
    ["Start from", "Round 1"],
  ]);
  const random = selectLobby(
    table("bart", {
      map: { kind: "random", systems: 1, hyperlanes: true, recommended: false },
      strategy_card_set: "base_game_codex1",
      bot_delay_ms: 0,
    }),
    local(),
    null,
  );
  expect(random.setup.map((row) => row.value)).toEqual([
    "6",
    "Random · 1 system · hyperlanes",
    "Base game with Codex I",
    "Off",
    "Round 1",
  ]);
  // Before the server said the names, a set is its id in words; a server with no scenario has no row for it.
  const bare = selectLobby(of(lobby(), "ada", null), local(), null);
  expect(bare.setup.map((row) => [row.label, row.value])).toEqual([
    ["Players", "6"],
    ["Map", "Standard · 37 systems"],
    ["Strategy cards", "Te"],
    ["Bots wait", "0.6 s"],
  ]);
  expect(bare.setup.find((row) => row.id === "cards")?.control).toBeNull();
});

const row = (state: TableState, id: string) =>
  selectLobby(state, local(), null).setup.find((item) => item.id === id)!;
const steps = (state: TableState, id: string) => {
  const control = row(state, id).control;
  return control?.kind === "counter"
    ? [control.less, control.more].map((step) => step.intent ?? step.reason)
    : null;
};

it("the host changes the players in steps of one, unless a person is in the way", () => {
  expect(steps(table("ada"), "players")).toEqual([
    { type: "set", change: { players: 5 } },
    { type: "set", change: { players: 7 } },
  ]);
  // Two people and a bot: the bot gives its seat, a person does not.
  const three = lobby();
  three.slots.length = 3;
  expect(steps(of(three, "ada"), "players")?.[0]).toEqual({ type: "set", change: { players: 2 } });
  three.slots[2] = person(3, "c", "Cy");
  expect(steps(of(three, "ada"), "players")?.[0]).toBe("A person is in every seat");
  const two = lobby();
  two.slots.length = 2;
  expect(steps(of(two, "ada"), "players")?.[0]).toBe("A table has 2 seats at least");
  const eight = lobby();
  eight.slots.push(slot(7), slot(8));
  expect(steps(of(eight, "ada"), "players")?.[1]).toBe("A table has 8 seats at most");
});

it("the host chooses the set of strategy cards from the list of the server", () => {
  const cards = row(table("ada", { strategy_card_set: "pok" }), "cards");
  expect(cards.control).toEqual({
    kind: "list",
    chosen: "pok",
    options: [
      { id: "te", label: "Thunder's Edge", intent: { type: "set", change: { cards: "te" } } },
      { id: "pok", label: "Prophecy of Kings", intent: { type: "set", change: { cards: "pok" } } },
      {
        id: "base_game_codex1",
        label: "Base game with Codex I",
        intent: { type: "set", change: { cards: "base_game_codex1" } },
      },
    ],
  });
  // Behind "?": the cards of a set that the usual set does not have.
  expect(cards.help).toEqual([
    { title: "Prophecy of Kings", text: "4 Construction" },
    { title: "Base game with Codex I", text: "4 Construction" },
  ]);
});

it("the host changes the wait in steps of half a second", () => {
  expect(steps(table("ada", { bot_delay_ms: 1000 }), "wait")).toEqual([
    { type: "set", change: { waitMs: 500 } },
    { type: "set", change: { waitMs: 1500 } },
  ]);
  // The value of the server is between two steps: a step goes to the next one.
  expect(steps(table("ada"), "wait")).toEqual([
    { type: "set", change: { waitMs: 500 } },
    { type: "set", change: { waitMs: 1000 } },
  ]);
  expect(steps(table("ada", { bot_delay_ms: 0 }), "wait")?.[0]).toBe("The seats answer at once");
  expect(steps(table("ada", { bot_delay_ms: 30_000 }), "wait")?.[1]).toBe("30 s at most");
  expect(row(table("ada", { bot_delay_ms: 1500 }), "wait").value).toBe("1.5 s");
});

it("a scenario is a game of its own: its players and its cards are not chosen", () => {
  const start = row(table("ada"), "start");
  expect(start.control).toEqual({
    kind: "list",
    chosen: "",
    options: [
      { id: "", label: "Round 1", intent: { type: "set", change: { startFrom: null } } },
      {
        id: "late-game",
        label: "Late game",
        intent: { type: "set", change: { startFrom: "late-game" } },
      },
    ],
  });
  expect(start.help).toEqual([{ title: "Late game", text: "Round 4." }]);

  const late = table("ada", { start_from: "late-game", strategy_card_set: "pok" });
  expect(row(late, "start").value).toBe("Late game");
  expect(row(late, "players").control).toBeNull();
  expect(row(late, "cards").control).toBeNull();
  expect(row(late, "cards").value).toBe("Prophecy of Kings");
  // The wait is of the table, not of the game that starts.
  expect(row(late, "wait").control?.kind).toBe("counter");
  // Every player is told where the game starts.
  expect(row(table("bart", { start_from: "late-game" }), "start")).toMatchObject({
    value: "Late game",
    control: null,
  });
});

it("a change of the host is one sentence for the other players", () => {
  const before = lobby();
  expect(hostChange(before, lobby())).toBeNull();
  expect(hostChange(before, lobby({ lobby_version: 6 }))).toBeNull();
  expect(hostChange(before, lobby({ map_revision: 2 }))).toBe("The host changed the map");
  expect(hostChange(before, lobby({ strategy_card_set: "pok" }))).toBe(
    "The host changed the strategy cards",
  );
  expect(hostChange(before, lobby({ bot_delay_ms: 1000 }))).toBe(
    "The host set the wait of the bots to 1 s",
  );
  expect(hostChange(before, lobby({ bot_delay_ms: 0 }))).toBe(
    "The host set the wait of the bots to off",
  );
  // A new count has another map: the count is what the host changed.
  const five = lobby({ map_revision: 2 });
  five.slots.length = 5;
  expect(hostChange(before, five)).toBe("The host set the players to 5");
  expect(hostChange(before, lobby({ start_from: "late-game", map_revision: 2 }))).toBe(
    "The host changed where the game starts",
  );
});

it("a lost connection has one signal", () => {
  expect(selectLobby(table("ada"), local(), null).status).toBe("");
  expect(selectLobby(table("ada"), local({ offline: true }), null).status).toBe(
    "Offline · reconnecting",
  );
});

it("the board is the tiles of the preview, and a home is the place of a seat", () => {
  const tile = (
    system_id: string,
    q: number,
    change: Partial<BoardTileView> = {},
  ): BoardTileView => ({
    system_id,
    label: `System ${system_id}`,
    q,
    r: 0,
    ...change,
  });
  const state: TableState = {
    ...table("ada"),
    preview: {
      player_count: 2,
      tiles: [
        tile("18", 0, {
          planets: [{ id: "mecatol", label: "Mecatol Rex", resources: 1, influence: 6 }],
        }),
        tile("seat_1", 1),
        tile("26", 2, { wormholes: ["alpha"] }),
        tile("39", 3, { wormholes: ["alpha"] }),
        tile("nexus", 9, { special_area: "nexus" }),
      ],
      seats: [{ seat: 1, home_system_id: "seat_1" }],
    },
  };
  const view = selectLobby(state, local({ inspected: "18" }), null);
  expect(view.board.tiles.map((item) => [item.id, item.home, item.planets.length])).toEqual([
    ["18", null, 1],
    // Nobody sits there yet: the home is a place, with no planets of a faction.
    ["seat_1", "place", 0],
    ["26", null, 0],
    ["39", null, 0],
  ]);
  expect(view.seats.place).toBeDefined();
  expect(view.board.wormholes).toEqual([["26", "39"]]);
  expect(view.board.inspector?.title).toBe("System 18 · 18");
  expect(view.board.task).toBeNull();
  // Before the server said the board, the lobby has an empty one.
  expect(selectLobby(table("ada"), local(), null).board.tiles).toEqual([]);
});

it("another tab of a player is that name with the next free number", () => {
  expect(nextNickname(lobby(), "ada")).toBe("Ada 2");
  const more = lobby();
  more.slots[3] = person(4, "ada2", "Ada 2");
  expect(nextNickname(more, "ada")).toBe("Ada 3");
  // From the second tab the number goes on: it is not "Ada 2 2".
  expect(nextNickname(more, "ada2")).toBe("Ada 3");
  expect(nextNickname(lobby(), "bart").length).toBeLessThanOrEqual(64);
});
