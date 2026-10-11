import { expect, it } from "vitest";
import menuUpdate from "../fixtures/update-menu.json";
import type { ScoreFacts, SessionUpdate } from "../wire";
import { selectScore } from "./score";

// An update of the real engine, viewed by seat a, with the points written by hand.
const menu = menuUpdate as unknown as SessionUpdate;
const seats = menu.view.players.map((player) => player.id);
const facts: ScoreFacts = {
  target: 10,
  public: [
    {
      id: "expand_borders",
      name: "Expand Borders",
      points: 1,
      text: "Control *6* planets.",
      scored: ["a", "c"],
    },
    {
      id: "master_science",
      name: "Master the Sciences",
      points: 2,
      text: "Own 2 technologies.",
      scored: ["b"],
    },
  ],
  sources: [
    { kind: "secrets", name: "", points: { a: 2, d: 1 } },
    { kind: "custodians", name: "", points: { b: 1 } },
    { kind: "imperial", name: "", points: {} },
    { kind: "support", name: "", points: { a: 1 }, from: { a: ["c"] } },
    { kind: "agenda", name: "Mutiny", points: { d: -1 } },
    { kind: "relic", name: "Shard of the Throne", points: { c: 1 } },
    { kind: "other", name: "a reason of tomorrow", points: { c: 1 } },
  ],
};
const score = (score: ScoreFacts = facts) => selectScore({ ...menu, score });
const cells = (view: ReturnType<typeof score>, name: string) => {
  const row = view?.groups.flatMap((group) => group.rows).find((item) => item.name === name);
  return Object.fromEntries(
    (view?.seats ?? []).map((seat, index) => [seat.seat, row?.cells[index]]),
  );
};

it("has no sheet when the game does not say where the points come from", () => {
  expect(selectScore(menu)).toBeNull();
});

it("has the viewer first, then the other seats by their points", () => {
  const players = menu.view.players.map((player, index) => ({
    ...player,
    victory_points: index % 4,
  }));
  const view = selectScore({ ...menu, view: { ...menu.view, players }, score: facts });
  expect(view?.target).toBe(10);
  expect(view?.seats.map((seat) => seat.seat).sort()).toEqual([...seats].sort());
  expect(view?.seats.map((seat) => seat.isMe)).toEqual([true, ...seats.slice(1).map(() => false)]);
  expect(view?.seats[0].seat).toBe("a");
  const points = view?.seats.slice(1).map((seat) => seat.points) ?? [];
  expect(points).toEqual([...points].sort((a, b) => b - a));
  for (const seat of view?.seats ?? []) {
    expect(seat.points).toBe(players.find((player) => player.id === seat.seat)?.victory_points);
  }
});

it("puts the public objectives in their stage, with the seats that scored", () => {
  const view = score();
  expect(view?.groups.map((group) => [group.title, group.rows.map((row) => row.name)])).toEqual([
    ["Public objectives · stage I", ["Expand Borders"]],
    ["Public objectives · stage II", ["Master the Sciences"]],
    [
      "Other points",
      [
        "Secret objectives",
        "Custodians of Mecatol Rex",
        "Imperial · Mecatol Rex",
        "Support for the Throne",
        "Agenda · Mutiny",
        "Relic · Shard of the Throne",
        "a reason of tomorrow",
      ],
    ],
  ]);
  const borders = view?.groups[0].rows[0];
  expect(borders).toMatchObject({ worth: 1, text: "Control 6 planets." });
  expect(cells(view, "Expand Borders")).toMatchObject({
    a: { kind: "scored" },
    b: { kind: "none" },
    c: { kind: "scored" },
  });
  expect(view?.groups[1].rows[0].worth).toBe(2);
});

it("has one cell for each seat in every row", () => {
  for (const row of score()?.groups.flatMap((group) => group.rows) ?? []) {
    expect(row.cells).toHaveLength(seats.length);
  }
});

it("says the points of another source, a loss, and whose Support a seat has", () => {
  const view = score();
  expect(cells(view, "Secret objectives")).toMatchObject({
    a: { kind: "points", value: 2, from: [] },
    b: { kind: "none" },
    d: { kind: "points", value: 1 },
  });
  expect(cells(view, "Imperial · Mecatol Rex").a).toEqual({ kind: "none" });
  expect(cells(view, "Support for the Throne").a).toEqual({
    kind: "points",
    value: 1,
    from: ["c"],
  });
  expect(cells(view, "Agenda · Mutiny").d).toEqual({ kind: "points", value: -1, from: [] });
});

it("has a row for the points that no source explains", () => {
  const view = score({ ...facts, unexplained: { b: 2 } });
  expect(view?.groups[2].rows.at(-1)?.name).toBe("Other");
  expect(cells(view, "Other").b).toEqual({ kind: "points", value: 2, from: [] });
});
