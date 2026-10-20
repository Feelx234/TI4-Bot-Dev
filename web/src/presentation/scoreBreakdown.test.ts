import { describe, expect, it } from "vitest";
import type { PlayerView, ScoreFacts, TableView } from "../protocol/types.ts";
import { buildScoreModel, fallbackScoreFacts } from "./scoreBreakdown.ts";

const player = (id: string, vp: number, extra: Partial<PlayerView> = {}) =>
  ({ id, faction: "sol", victory_points: vp, secret_objectives_count: 0, scored_secret_objectives: [], ...extra }) as unknown as PlayerView;

const facts: ScoreFacts = {
  target: 10,
  public: [
    { id: "expand_borders", name: "Expand Borders", points: 1, text: "", scored: ["a", "b"] },
    { id: "conquer", name: "Conquer the Weak", points: 2, text: "", scored: ["a"] },
    { id: "golden_age", name: "Found a Golden Age", points: 2, text: "", scored: [] },
  ],
  sources: [
    { kind: "secrets", name: "", points: { a: 1 } },
    { kind: "custodians", name: "", points: { b: 1 } },
    { kind: "imperial", name: "", points: {} },
    { kind: "support", name: "", points: { a: 1 }, from: { a: ["b"] } },
    { kind: "agenda", name: "Mutiny", points: { b: 1, a: -1 } },
  ],
  unexplained: {},
};
const label = (id: string) => id.toUpperCase();

describe("buildScoreModel", () => {
  const players = [player("a", 4, { scored_secret_objectives: ["faa"] }), player("b", 3), player("c", 0)];
  const model = buildScoreModel({ players, score: facts, label });

  it("lists each source of a seat and the rows add up to the engine total", () => {
    const a = model.seats[0];
    expect(a.rows.map((r) => [r.group, r.label, r.points])).toEqual([
      ["public", "Expand Borders", 1],
      ["public", "Conquer the Weak", 2],
      ["secret", "Forge an Alliance", 1],
      ["support", "Support for the Throne", 1],
      ["agenda", "Mutiny", -1],
    ]);
    for (const seat of model.seats) expect(seat.rows.reduce((n, r) => n + r.points, 0)).toBe(seat.total);
    expect(a.rows.find((r) => r.group === "support")?.detail).toBe("Note from B");
  });

  it("marks the leader, the gap and the points to win", () => {
    expect(model.seats.map((s) => [s.seat, s.isLeader, s.behind, s.toWin])).toEqual([
      ["a", true, 0, 6],
      ["b", false, 1, 7],
      ["c", false, 4, 10],
    ]);
  });

  it("lists the revealed public objectives a seat has not scored", () => {
    expect(model.seats[1].open.map((o) => o.id)).toEqual(["conquer", "golden_age"]);
    expect(model.seats[1].open[0].scoredBy).toEqual(["a"]);
    expect(model.seats[2].open).toHaveLength(3);
  });

  it("nobody leads at zero points and ties share the lead", () => {
    const none = buildScoreModel({ players: [player("a", 0), player("b", 0)], score: { ...facts, public: [], sources: [] }, label });
    expect(none.seats.some((s) => s.isLeader)).toBe(false);
    const tie = buildScoreModel({ players: [player("a", 2), player("b", 2)], score: { ...facts, public: [], sources: [] }, label });
    expect(tie.seats.every((s) => s.isLeader)).toBe(true);
  });

  it("shows points the game does not name as other adjustments", () => {
    const m = buildScoreModel({ players: [player("a", 5)], score: { ...facts, public: [], sources: [], unexplained: { a: 5 } }, label });
    expect(m.seats[0].rows).toEqual([expect.objectContaining({ key: "unexplained", points: 5 })]);
  });
});

describe("fallback without server score facts", () => {
  const table = {
    revealed_objectives: ["expand_borders", "conquer"],
    scored_objectives: { a: ["expand_borders", "conquer"] },
  } as unknown as TableView;
  it("rebuilds public and secret rows and keeps the total honest", () => {
    const players = [player("a", 5, { scored_secret_objectives: ["faa"] }), player("b", 0)];
    const f = fallbackScoreFacts(players, table);
    expect(f.target).toBe(10);
    const model = buildScoreModel({ players, table, label });
    expect(model.seats[0].rows.reduce((n, r) => n + r.points, 0)).toBe(5);
    expect(model.seats[0].unexplained).toBe(1);
    expect(model.seats[1].open.map((o) => o.id)).toEqual(["expand_borders", "conquer"]);
  });
});
