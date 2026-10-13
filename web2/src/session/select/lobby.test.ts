import { expect, it } from "vitest";
import type { TableState } from "../table";
import type { BoardTileView, Catalog, LobbySlot, LobbyState } from "../wire";
import { type LobbyLocal, nextNickname, selectLobby } from "./lobby";

const LONG = "Bartholomew Featherstonehaugh the Third of Mecatol";

const slot = (position: number, change: Partial<LobbySlot> = {}): LobbySlot => ({
  slot_id: `slot_${position}`,
  position,
  occupant: null,
  nickname: null,
  ready: false,
  connected: false,
  can_take_over: false,
  ...change,
});
const person = (position: number, id: string, nickname: string, change: Partial<LobbySlot> = {}) =>
  slot(position, { occupant: id, nickname, ready: true, connected: true, ...change });

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
      ready: true,
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
  ...change,
});
const table = (viewer: string | null, change: Partial<LobbyState> = {}): TableState => ({
  lobby: lobby(change),
  viewer,
  preview: null,
});
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
  expect(selectLobby({ lobby: one, viewer: "ada", preview: null }, local(), null).footer.note).toBe(
    "Waiting for 1 open seat",
  );
});

it("a full table starts, unless a player of another page is not ready", () => {
  const full = lobby();
  full.slots[3] = person(4, "c", "Cy");
  full.slots[4] = person(5, "d", "Di", { ready: false });
  full.slots[5] = slot(6, {
    occupant: "bot2",
    nickname: "Random bot 1",
    ready: true,
    bot: "random",
  });
  const waits = selectLobby({ lobby: full, viewer: "ada", preview: null }, local(), null);
  expect(waits.footer.note).toBe("Waiting for Di to be ready");
  expect(waits.footer.main?.disabled).toBe(true);
  expect(waits.fill).toBeNull();
  expect(waits.rows[5].sign).toEqual({ kind: "random", word: "Random: played by the server" });

  full.slots[4] = person(5, "d", "Di");
  const ready = selectLobby({ lobby: full, viewer: "ada", preview: null }, local(), null);
  expect(ready.footer).toEqual({
    note: "",
    main: { label: "Start game", disabled: false, intent: { type: "start" } },
    nickname: false,
  });
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
  expect(view.setup.every((row) => !row.editable)).toBe(true);
  expect(view.footer).toEqual({ note: "Waiting for 3 open seats", main: null, nickname: false });

  const full = lobby();
  for (const index of [3, 4, 5]) {
    full.slots[index] = person(index + 1, `p${index}`, `Player ${index}`);
  }
  expect(
    selectLobby({ lobby: full, viewer: "bart", preview: null }, local(), null).footer.note,
  ).toBe("Waiting for Ada to start");
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
  expect(selectLobby({ lobby: full, viewer: null, preview: null }, local(), null).footer).toEqual({
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
  const seen = selectLobby({ lobby: away, viewer: "bart", preview: null }, local(), null);
  expect(seen.rows[0].sign).toEqual({ kind: "away", word: "Host · not connected" });
  expect(seen.rows[1].sign).toBeNull();
  const visitor = selectLobby({ lobby: away, viewer: null, preview: null }, local(), null);
  expect(visitor.rows[1].sign).toEqual({ kind: "away", word: "Not connected" });
});

it("the setup says the players, the map and the set of strategy cards", () => {
  const view = selectLobby(table("ada"), local(), null);
  expect(view.setup.map((row) => [row.label, row.value])).toEqual([
    ["Players", "6"],
    ["Map", "Standard · 37 systems"],
    ["Strategy cards", "Thunder's Edge"],
  ]);
  const random = selectLobby(
    table("ada", {
      map: { kind: "random", systems: 1, hyperlanes: true, recommended: false },
      strategy_card_set: "base_game_codex1",
    }),
    local(),
    null,
  );
  expect(random.setup.map((row) => row.value)).toEqual([
    "6",
    "Random · 1 system · hyperlanes",
    "Base game with Codex I",
  ]);
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
        tile("01", 1, { planets: [{ id: "jord", label: "Jord", resources: 4, influence: 2 }] }),
        tile("26", 2, { wormholes: ["alpha"] }),
        tile("39", 3, { wormholes: ["alpha"] }),
        tile("nexus", 9, { special_area: "nexus" }),
      ],
      seats: [{ seat: 1, faction: "sol", faction_name: "Sol", home_system_id: "01" }],
    },
  };
  const view = selectLobby(state, local({ inspected: "18" }), null);
  expect(view.board.tiles.map((item) => [item.id, item.home, item.planets.length])).toEqual([
    ["18", null, 1],
    // Nobody sits there yet: the home has no planets of a faction.
    ["01", "place", 0],
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
