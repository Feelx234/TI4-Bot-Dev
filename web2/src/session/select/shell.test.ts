import { expect, it } from "vitest";
import menuUpdate from "../fixtures/update-menu.json";
import systemsUpdate from "../fixtures/update-load.json";
import type { SessionUpdate } from "../wire";
import type { LocalState } from "./action";
import { selectShell } from "./shell";

// The fixtures are updates of a real game, written by the engine:
//   cargo run -p ti4-wasm --example record -- 3 8 1 20 > web2/src/session/fixtures/update-menu.json
//   cargo run -p ti4-wasm --example record -- 3 8 1 6 pok8imperial no tactical 23 'move|01|0' > web2/src/session/fixtures/update-load.json
const menu = menuUpdate as unknown as SessionUpdate;
const systems = systemsUpdate as unknown as SessionUpdate;
const local = (change: Partial<LocalState> = {}): LocalState => ({
  inspected: null,
  staged: null,
  sent: null,
  error: null,
  replaying: null,
  canUndo: false,
  movement: {},
  landing: {},
  landingKind: null,
  handled: [],
  step: null,
  remaining: null,
  planNote: null,
  stepping: null,
  draftLocked: null,
  draft: null,
  ...change,
});
const flow = (update: SessionUpdate, state: LocalState) => {
  const { action } = selectShell(update, state);
  if (action.kind !== "flow") {
    throw new Error("local play has only the flow panel");
  }
  return action;
};
const rows = (update: SessionUpdate, state: LocalState) =>
  flow(update, state).blocks.flatMap((block) => (block.kind === "menu" ? block.rows : []));

it("shows eight seats with different marks, in seat order", () => {
  const view = selectShell(menu, local());
  expect(view.players.rows.map((row) => row.seat)).toEqual(menu.view.seating_order);
  expect(view.players.rows).toHaveLength(8);
  const marks = Object.values(view.seats).map((seat) => seat.symbol + seat.color);
  expect(new Set(marks).size).toBe(8);
  expect(view.seats.a.faction).toBe("Sol");
  expect(view.players.rows.filter((row) => row.isMe).map((row) => row.seat)).toEqual(["a"]);
  expect(view.players.rows.filter((row) => row.turn === "now").map((row) => row.seat)).toEqual([
    "a",
  ]);
});

it("counts what the planets of a seat give, ready and total", () => {
  const row = selectShell(menu, local()).players.rows[0];
  const values = new Map(
    (menu.view.board.map_tiles ?? []).flatMap((tile) =>
      (tile.planets ?? []).map((planet) => [planet.id, planet] as const),
    ),
  );
  const own = Object.values(menu.view.board.systems)
    .flatMap((system) => Object.values(system.planets))
    .filter((planet) => planet.controlled_by === "a");
  const sum = (ready: boolean) =>
    own
      .filter((planet) => !ready || !planet.exhausted)
      .reduce((total, planet) => total + values.get(planet.planet_id)!.resources, 0);
  expect(own.length).toBeGreaterThan(0);
  expect(row.planets).toBe(own.length);
  expect(row.resources).toEqual([sum(true), sum(false)]);
});

it("puts every system of the map on the board, and none from outside it", () => {
  const { board } = selectShell(menu, local());
  const onMap = (menu.view.board.map_tiles ?? []).filter((tile) => !tile.special_area);
  expect(board.tiles.map((tile) => tile.id)).toEqual(onMap.map((tile) => tile.system_id));
  const home = board.tiles.find((tile) => tile.id === "01")!;
  expect(home.planets.map((planet) => [planet.name, planet.owner])).toEqual([["Jord", "a"]]);
  expect(home.fleets.map((fleet) => fleet.seat)).toEqual(["a"]);
  const ships = menu.view.board.systems["01"].units.filter(
    (unit) => unit.planet === null && !/pds|spacedock/.test(unit.unit_type),
  );
  expect(home.fleets[0].ships).toBe(ships.length);
});

it("shows a choice as one list, and stages a row without sending", () => {
  const choice = menu.pending_choice!.choice;
  expect(rows(menu, local()).map((row) => row.intent)).toEqual(
    choice.options.map((option) => ({ type: "chooseOption", option: option.id })),
  );
  expect(flow(menu, local()).footer.actions.at(-1)).toMatchObject({
    label: "Send",
    disabled: true,
  });
  const staged = local({ staged: choice.options[1].id });
  expect(rows(menu, staged).map((row) => row.selected)).toEqual(
    choice.options.map((_, index) => index === 1),
  );
  expect(flow(menu, staged).badge).toEqual({ tone: "draft", label: "Not sent" });
  expect(flow(menu, staged).footer.actions.at(-1)).toMatchObject({
    label: "Send",
    disabled: false,
  });
});

it("has no choice after the answer is sent, until the next update", () => {
  const sent = local({ sent: menu.pending_choice!.nonce });
  expect(rows(menu, sent)).toEqual([]);
  expect(flow(menu, sent).footer.actions).toEqual([]);
  expect(selectShell(menu, sent).toolbar.status).toBe("Sent · the game goes on");
});

it("shows a decision without a screen of its own as a list, also inside a tactical action", () => {
  const choice = systems.pending_choice!.choice;
  expect(choice.context?.subtype).toBe("load_cargo");
  expect(rows(systems, local()).map((row) => row.intent)).toEqual(
    choice.options.map((option) => ({ type: "chooseOption", option: option.id })),
  );
  expect(selectShell(systems, local()).board.task).toBeNull();
});

it("shows the forces of an inspected system", () => {
  const { inspector } = selectShell(menu, local({ inspected: "01" })).board;
  expect(inspector?.title).toBe("Jord - Sol · 01");
  expect(inspector?.rows[0].seat).toBe("a");
  expect(inspector?.rows.at(-1)?.planet?.name).toBe("Jord");
});

it("says why the game stopped", () => {
  const view = selectShell(menu, local({ error: "run: the engine failed" }));
  expect(view.toolbar.status).toBe("The game stopped");
});

it("offers undo once there is an answer to take back", () => {
  const labels = (state: LocalState) => flow(menu, state).footer.actions.map((item) => item.label);
  expect(labels(local())).toEqual(["Send"]);
  expect(labels(local({ canUndo: true }))).toEqual(["Undo", "Send"]);
  expect(flow(menu, local({ canUndo: true })).footer.actions[0].intent).toEqual({ type: "undo" });
});

it("has no choice while a saved game is played again: the update is out of date", () => {
  const replaying = local({ canUndo: true, replaying: { done: 12, total: 40 } });
  expect(rows(menu, replaying)).toEqual([]);
  expect(flow(menu, replaying).footer.actions).toEqual([]);
  expect(selectShell(menu, replaying).toolbar.status).toBe("Replaying 12 of 40");
});
