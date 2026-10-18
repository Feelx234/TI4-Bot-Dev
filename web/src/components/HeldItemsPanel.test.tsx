import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { PlayerSheet } from "./PlayerSheet.tsx";
import { PublicStateProvider } from "../presentation/PublicStateContext.tsx";
import type { PlayerView } from "../protocol/types.ts";

const seat = (id: string, faction: string, extra: Partial<PlayerView> = {}): PlayerView => ({
  id, faction, victory_points: 0, trade_goods: 0, commodities: 0, tactic_tokens: 3, fleet_tokens: 3,
  strategic_tokens: 2, passed: false, strategy_cards: [], exhausted_strategy_cards: [], technologies: [],
  exhausted_technologies: [], relics: [], exhausted_relics: [], action_cards_count: 0,
  secret_objectives_count: 0, leaders: {}, ...extra,
});
const players = [
  seat("p1", "hacan", { relics: ["heartofixth"], exhausted_relics: ["heartofixth"], technologies: ["aida"] }),
  seat("p2", "sol", { relics: ["thetriad"] }),
];
const state = { promissory_notes: { "cf:sol": "p1", "ps:hacan": "p2" }, promissory_faceup: [] };

const open = () =>
  render(
    <PublicStateProvider state={state}>
      <PlayerSheet players={players} userSeat="p1" />
    </PublicStateProvider>,
  );

describe("Held items entry and panel", () => {
  it("shows a count badge on each seat and opens a labelled panel", () => {
    open();
    expect(screen.getByTestId("held-items-button-0")).toHaveAccessibleName("Held items: 3");
    fireEvent.click(screen.getByTestId("held-items-button-0"));
    const panel = screen.getByTestId("held-items-panel");
    expect(panel).toHaveAttribute("role", "dialog");
    expect(panel).toHaveAttribute("data-own", "true");
    expect(screen.getByTestId("held-item-relic-heartofixth")).toHaveTextContent("exhausted");
    expect(screen.getByTestId("held-item-note-cf:sol")).toHaveTextContent("in hand");
  });

  it("switches to another seat read-only without its hand", () => {
    open();
    fireEvent.click(screen.getByTestId("held-items-button-0"));
    fireEvent.click(screen.getByTestId("held-items-seat-1"));
    expect(screen.getByTestId("held-items-panel")).toHaveAttribute("data-own", "false");
    expect(screen.getByTestId("held-item-relic-thetriad")).toBeInTheDocument();
    expect(screen.queryByText(/Political Secret/)).toBeNull();
    expect(screen.getByTestId("held-items-mode")).toHaveTextContent("public information only");
  });

  it("closes with Escape and the Close button", () => {
    open();
    fireEvent.click(screen.getByTestId("held-items-button-1"));
    expect(screen.getByTestId("held-items-mode")).toHaveTextContent("Read-only");
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByTestId("held-items-panel")).toBeNull();
    fireEvent.click(screen.getByTestId("held-items-button-0"));
    fireEvent.click(screen.getByTestId("held-items-close"));
    expect(screen.queryByTestId("held-items-panel")).toBeNull();
  });
});
