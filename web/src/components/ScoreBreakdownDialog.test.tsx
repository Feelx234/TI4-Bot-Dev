import { describe, expect, it } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { PlayerSheet } from "./PlayerSheet.tsx";
import { PassOrderProvider } from "../presentation/passOrder.tsx";
import { PublicStateProvider } from "../presentation/PublicStateContext.tsx";
import type { PlayerView, ScoreFacts, TableView } from "../protocol/types.ts";

const seat = (id: string, faction: string, extra: Partial<PlayerView> = {}): PlayerView => ({
  id, faction, victory_points: 0, trade_goods: 0, commodities: 0, tactic_tokens: 3, fleet_tokens: 3,
  strategic_tokens: 2, passed: false, strategy_cards: [], exhausted_strategy_cards: [], technologies: [],
  exhausted_technologies: [], relics: [], exhausted_relics: [], action_cards_count: 0,
  secret_objectives_count: 0, leaders: {}, ...extra,
});
const players = [
  seat("p1", "sol", { victory_points: 4, scored_secret_objectives: ["faa"] }),
  seat("p2", "hacan", { victory_points: 5, passed: true }),
];
const table = {
  revealed_objectives: ["expand_borders", "conquer"],
  scored_objectives: { p1: ["expand_borders", "conquer"], p2: [] },
} as unknown as TableView;
const score: ScoreFacts = {
  target: 10,
  public: [
    { id: "expand_borders", name: "Expand Borders", points: 1, text: "", scored: ["p1"] },
    { id: "conquer", name: "Conquer the Weak", points: 2, text: "", scored: ["p1"] },
  ],
  sources: [
    { kind: "secrets", name: "", points: { p1: 1 } },
    { kind: "custodians", name: "", points: { p2: 1 } },
    { kind: "imperial", name: "", points: { p2: 2 } },
    { kind: "support", name: "", points: { p2: 1 }, from: { p2: ["p1"] } },
    { kind: "relic", name: "Shard of the Throne", points: { p2: 1 } },
  ],
};

const open = (phase = "action", round = 2, withScore: ScoreFacts | null = score) =>
  render(
    <PublicStateProvider state={{}} score={withScore}>
      <PassOrderProvider players={players} round={round} phase={phase}>
        <PlayerSheet players={players} userSeat="p1" table={table} />
      </PassOrderProvider>
    </PublicStateProvider>,
  );

describe("VP breakdown dialog", () => {
  it("opens from the VP chip with the engine total and the points by source", () => {
    open();
    fireEvent.click(screen.getAllByTestId("player-vp")[0]);
    const dialog = screen.getByTestId("score-dialog");
    expect(dialog).toHaveAttribute("role", "dialog");
    expect(screen.getByTestId("score-total")).toHaveTextContent("4 / 10 VP");
    expect(within(screen.getByTestId("score-group-public")).getByText("Conquer the Weak")).toBeInTheDocument();
    expect(screen.getByTestId("score-row-secret:faa")).toHaveTextContent("Forge an Alliance");
    expect(screen.getByTestId("score-sum")).toHaveAttribute("data-matches", "true");
    expect(screen.getByTestId("score-open")).toHaveTextContent("None revealed, or all scored");
  });

  it("switches seats in one tap and marks the leader and the pass", () => {
    open();
    fireEvent.click(screen.getAllByTestId("player-vp")[0]);
    expect(screen.queryByTestId("score-leader")).toBeNull();
    fireEvent.click(screen.getByTestId("score-tab-p2"));
    expect(screen.getByTestId("score-leader")).toBeInTheDocument();
    expect(screen.getByTestId("score-passed")).toHaveTextContent("Passed");
    expect(screen.getByTestId("score-row-support:")).toHaveTextContent("Note from");
    expect(screen.getByTestId("score-row-relic:Shard of the Throne")).toHaveTextContent("+1");
    expect(screen.getByTestId("score-open-conquer")).toHaveTextContent("scored by");
    expect(screen.getByTestId("score-summary")).toHaveTextContent("Leading");
    expect(screen.getByTestId("score-sum")).toHaveAttribute("data-matches", "true");
  });

  it("falls back to the view when the server sends no score facts", () => {
    open("action", 2, null);
    fireEvent.click(screen.getAllByTestId("player-vp")[0]);
    expect(screen.getByTestId("score-sum")).toHaveAttribute("data-matches", "true");
    expect(screen.getByTestId("score-row-public:conquer")).toBeInTheDocument();
  });

  it("closes with the Close button", () => {
    open();
    fireEvent.click(screen.getAllByTestId("player-vp")[0]);
    fireEvent.click(screen.getByTestId("score-close"));
    expect(screen.queryByTestId("score-dialog")).toBeNull();
  });
});

describe("Passed badge on the player mat", () => {
  it("marks a seat that passed in the action phase", () => {
    open();
    const badges = screen.getAllByTestId("player-passed");
    expect(badges).toHaveLength(1);
    const card = badges[0].closest('[data-testid="player-card"]')!;
    expect(card).toHaveAttribute("data-is-self", "false");
    expect(badges[0]).toHaveTextContent(/^Passed/);
  });
  it("shows nothing in another phase", () => {
    open("status");
    expect(screen.queryByTestId("player-passed")).toBeNull();
  });
});
