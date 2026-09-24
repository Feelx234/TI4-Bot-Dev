import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { PlayerSheet } from "./PlayerSheet.tsx";
import { CardDetails } from "./CardDetails.tsx";
import { PlayerView } from "../protocol/types.ts";

const mockPlayers: PlayerView[] = [
  {
    id: "p1",
    faction: "Federation of Sol",
    victory_points: 3,
    trade_goods: 2,
    commodities: 4,
    tactic_tokens: 3,
    fleet_tokens: 3,
    strategic_tokens: 2,
    passed: false,
    strategy_cards: ["pok1leadership", "pok6warfare"],
    exhausted_strategy_cards: [],
    technologies: [],
    exhausted_technologies: [],
    relics: [],
    exhausted_relics: [],
    action_cards_count: 2,
    secret_objectives_count: 1,
    held_action_cards: ["direct_hit"],
    held_secret_objectives: ["faa"],
    leaders: {},
  },
  {
    id: "p2",
    faction: "Barony of Letnev",
    victory_points: 2,
    trade_goods: 1,
    commodities: 2,
    tactic_tokens: 2,
    fleet_tokens: 4,
    strategic_tokens: 1,
    passed: false,
    strategy_cards: ["pok2diplomacy"],
    exhausted_strategy_cards: [],
    technologies: [],
    exhausted_technologies: [],
    relics: [],
    exhausted_relics: [],
    action_cards_count: 3,
    secret_objectives_count: 1,
    held_action_cards: [], // Redacted by server
    held_secret_objectives: [],
    leaders: {},
  },
];

describe("PlayerSheet Component & Human Readable Metadata", () => {
  it("resolves cards to accessible click targets for a reusable detail panel", () => {
    const onInspectCard = vi.fn();
    render(<PlayerSheet players={mockPlayers} userSeat="p1" onInspectCard={onInspectCard} />);

    // Strategy cards pok1leadership and pok6warfare mapped to readable names
    const scLeadership = screen.getByTestId("strategy-card-badge-pok1leadership");
    expect(scLeadership).toHaveTextContent("1. Leadership");
    fireEvent.click(scLeadership);
    expect(onInspectCard).toHaveBeenCalledWith({ kind: "strategy", id: "pok1leadership" });

    const scWarfare = screen.getByTestId("strategy-card-badge-pok6warfare");
    expect(scWarfare).toHaveTextContent("6. Warfare");
    expect(scWarfare).toHaveAttribute("type", "button");

    // Secret objective "faa" mapped to Forge an Alliance with description and points
    const soFaa = screen.getByTestId("secret-objective-item-faa");
    expect(soFaa).toHaveTextContent("Forge an Alliance");
    expect(soFaa).toHaveTextContent("Control 4 cultural planets.");
    expect(soFaa).toHaveTextContent("1 VP");
    fireEvent.click(soFaa.querySelector("button")!);
    expect(onInspectCard).toHaveBeenCalledWith({ kind: "secretObjective", id: "faa" });

    // Action card direct_hit mapped to Direct Hit
    const acDirectHit = screen.getByTestId("action-card-item-direct_hit");
    expect(acDirectHit).toHaveTextContent("Direct Hit");
  });

  it("renders private cards exclusively for the viewer seat", () => {
    // Viewer is p1
    render(<PlayerSheet players={mockPlayers} userSeat="p1" />);

    // p1 sees own private cards
    expect(screen.getByTestId("secret-objective-item-faa")).toBeInTheDocument();
    expect(screen.getByTestId("action-card-item-direct_hit")).toBeInTheDocument();

    const privateCards = screen.getAllByTestId("player-card");
    expect(privateCards).toHaveLength(2);

    // Invariant: p2 must not have private-hand-section
    const p2Card = privateCards[1];
    expect(p2Card.querySelector('[data-testid="private-hand-section"]')).toBeNull();
  });

  it("renders zero private cards when viewer is spectator", () => {
    // Viewer is spectator (no seat)
    const { container } = render(<PlayerSheet players={mockPlayers} />);

    // Invariant: no private card elements exist anywhere in DOM
    const privateElements = container.querySelectorAll('[data-private-card="true"]');
    expect(privateElements).toHaveLength(0);

    // Only public counts are rendered
    expect(screen.getAllByText(/Action Cards:/i)).toHaveLength(2);
  });

  it("opens public objectives and only the owner’s private cards", () => {
    const onInspectCard = vi.fn();
    render(
      <PlayerSheet
        players={mockPlayers}
        userSeat="p1"
        revealedObjectives={["corner"]}
        onInspectCard={onInspectCard}
      />,
    );
    fireEvent.click(screen.getByTestId("public-objective-corner"));
    expect(onInspectCard).toHaveBeenCalledWith({ kind: "publicObjective", id: "corner" });
    fireEvent.click(screen.getByTestId("action-card-item-direct_hit").querySelector("button")!);
    expect(onInspectCard).toHaveBeenCalledWith({ kind: "action", id: "direct_hit" });
    expect(screen.queryByTestId("action-card-item-nonexistent")).not.toBeInTheDocument();
    render(<CardDetails subject={{ kind: "strategy", id: "pok1leadership" }} onClose={vi.fn()} />);
    expect(screen.getByTestId("detail-panel")).toHaveTextContent("Gain 3 command tokens");
  });
});
