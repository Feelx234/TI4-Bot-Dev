import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { ObjectivesModal } from "./ObjectivesModal.tsx";
import type { PlayerView, ObjectiveProgressView } from "../protocol/types.ts";

const mockPlayers: PlayerView[] = [
  {
    id: "p1",
    faction: "sol",
    victory_points: 3,
    trade_goods: 4,
    commodities: 2,
    tactic_tokens: 3,
    fleet_tokens: 4,
    strategic_tokens: 2,
    passed: false,
    strategy_cards: ["leadership"],
    exhausted_strategy_cards: [],
    technologies: ["sarween_tools"],
    exhausted_technologies: [],
    relics: [],
    exhausted_relics: [],
    action_cards_count: 2,
    secret_objectives_count: 1,
    leaders: {},
  },
  {
    id: "p2",
    faction: "hacan",
    victory_points: 2,
    trade_goods: 8,
    commodities: 6,
    tactic_tokens: 2,
    fleet_tokens: 3,
    strategic_tokens: 3,
    passed: false,
    strategy_cards: ["trade"],
    exhausted_strategy_cards: [],
    technologies: [],
    exhausted_technologies: [],
    relics: [],
    exhausted_relics: [],
    action_cards_count: 3,
    secret_objectives_count: 2,
    leaders: {},
  },
];

describe("ObjectivesModal", () => {
  it("renders empty state when no public objectives are revealed", () => {
    render(
      <ObjectivesModal
        isOpen={true}
        onClose={vi.fn()}
        revealedObjectives={[]}
        players={mockPlayers}
      />,
    );

    expect(screen.getByTestId("objectives-empty")).toHaveTextContent(
      "No public objectives revealed yet.",
    );
  });

  it("renders matrix table with Stage I and Stage II objectives", () => {
    render(
      <ObjectivesModal
        isOpen={true}
        onClose={vi.fn()}
        revealedObjectives={["corner", "unify_colonies"]}
        players={mockPlayers}
        viewerSeat="p1"
      />,
    );

    expect(screen.getByTestId("objectives-matrix-table")).toBeInTheDocument();
    expect(screen.getByText("Stage I Objectives (1 VP)")).toBeInTheDocument();
    expect(screen.getByText("Stage II Objectives (2 VP)")).toBeInTheDocument();

    expect(screen.getByTestId("objective-row-corner")).toBeInTheDocument();
    expect(screen.getByTestId("objective-row-unify_colonies")).toBeInTheDocument();
  });

  it("highlights the viewer player column with a 'You' tag", () => {
    render(
      <ObjectivesModal
        isOpen={true}
        onClose={vi.fn()}
        revealedObjectives={["corner"]}
        players={mockPlayers}
        viewerSeat="p1"
      />,
    );

    const p1Col = screen.getByTestId("player-col-header-p1");
    expect(p1Col).toHaveClass("objectives-matrix__player-col--self");
    expect(p1Col).toHaveTextContent("You");

    const p2Col = screen.getByTestId("player-col-header-p2");
    expect(p2Col).not.toHaveClass("objectives-matrix__player-col--self");
    expect(p2Col).not.toHaveTextContent("You");
  });

  it("renders Scored status, Ready status, and In Progress status correctly", () => {
    const scoredObjectives: Record<string, string[]> = {
      p1: ["corner"],
    };

    const objectiveProgress: Record<string, Record<string, ObjectiveProgressView>> = {
      p1: {
        unify_colonies: { have: 6, threshold: 6, satisfied: true },
      },
      p2: {
        corner: { have: 3, threshold: 4, satisfied: false },
        unify_colonies: { have: 2, threshold: 6, satisfied: false },
      },
    };

    render(
      <ObjectivesModal
        isOpen={true}
        onClose={vi.fn()}
        revealedObjectives={["corner", "unify_colonies"]}
        scoredObjectives={scoredObjectives}
        objectiveProgress={objectiveProgress}
        players={mockPlayers}
        viewerSeat="p1"
      />,
    );

    // p1 scored "corner"
    expect(screen.getByTestId("status-scored-corner-p1")).toBeInTheDocument();
    expect(screen.getByTestId("status-scored-corner-p1")).toHaveTextContent("Scored");

    // p2 in progress on "corner" (3/4)
    expect(screen.getByTestId("status-progress-corner-p2")).toBeInTheDocument();
    expect(screen.getByTestId("status-progress-corner-p2")).toHaveTextContent("3 / 4");

    // p1 is Ready on "unify_colonies" (6/6)
    expect(screen.getByTestId("status-ready-unify_colonies-p1")).toBeInTheDocument();
    expect(screen.getByTestId("status-ready-unify_colonies-p1")).toHaveTextContent("Ready (6/6)");

    // p2 in progress on "unify_colonies" (2/6)
    expect(screen.getByTestId("status-progress-unify_colonies-p2")).toBeInTheDocument();
    expect(screen.getByTestId("status-progress-unify_colonies-p2")).toHaveTextContent("2 / 6");
  });

  it("calls onInspectCard when an objective name button is clicked", () => {
    const onInspectCard = vi.fn();
    render(
      <ObjectivesModal
        isOpen={true}
        onClose={vi.fn()}
        revealedObjectives={["corner"]}
        players={mockPlayers}
        onInspectCard={onInspectCard}
      />,
    );

    fireEvent.click(screen.getByTestId("inspect-objective-corner"));
    expect(onInspectCard).toHaveBeenCalledWith({
      kind: "publicObjective",
      id: "corner",
    });
  });

  it("renders in scoring mode with selectable options, decline, and submit button", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();

    render(
      <ObjectivesModal
        isOpen={true}
        onClose={onClose}
        revealedObjectives={["lead", "trade_routes", "corner"]}
        players={mockPlayers}
        viewerSeat="p1"
        choice={{
          actor: "p1",
          nonce: "nonce-1",
          prompt: "Score a public objective",
          context: { subtype: "score_objective" },
          options: [
            { id: "lead", label: "Lead From the Front" },
            { id: "trade_routes", label: "Negotiate Trade Routes" },
            { id: "decline", label: "Decline" },
          ],
        }}
        onSubmit={onSubmit}
        isScoringMode={true}
      />,
    );

    // Scoring title and badge
    expect(screen.getByText("Score Public Objective")).toBeInTheDocument();
    expect(screen.getByText("Scoring Window")).toBeInTheDocument();
    expect(screen.getByTestId("objectives-scoring-footer")).toBeInTheDocument();

    // Check minimize button
    const minimizeBtn = screen.getByTestId("minimize-choice-button");
    expect(minimizeBtn).toBeInTheDocument();
    fireEvent.click(minimizeBtn);
    expect(onClose).toHaveBeenCalled();

    // Scoreable options are actionable
    const options = screen.getAllByTestId("choice-option");
    expect(options.length).toBe(3); // lead, trade_routes, decline

    // Select trade_routes
    const tradeOption = document.querySelector(
      '[data-testid="choice-option"][data-option-id="trade_routes"]',
    )!;
    expect(tradeOption).not.toBeNull();
    fireEvent.click(tradeOption);

    // Footer summary updates
    const footer = screen.getByTestId("objectives-scoring-footer");
    expect(footer).toHaveTextContent("Negotiate Trade Routes");

    // Submit
    const submitBtn = screen.getByTestId("submit-choice-button");
    expect(submitBtn).toBeEnabled();
    await act(async () => {
      fireEvent.click(submitBtn);
    });

    expect(onSubmit).toHaveBeenCalledWith("trade_routes");
  });

  it("preserves selection when props/snapshot updates arrive with the same choice nonce", () => {
    const choice = {
      actor: "p1",
      nonce: "nonce-same",
      prompt: "Score a public objective",
      context: { subtype: "score_objective" },
      options: [
        { id: "lead", label: "Lead From the Front" },
        { id: "trade_routes", label: "Negotiate Trade Routes" },
      ],
    };

    const { rerender } = render(
      <ObjectivesModal
        isOpen={true}
        onClose={vi.fn()}
        revealedObjectives={["lead", "trade_routes"]}
        players={mockPlayers}
        viewerSeat="p1"
        choice={choice}
        isScoringMode={true}
      />,
    );

    // Select second objective: trade_routes
    const tradeOption = document.querySelector(
      '[data-testid="choice-option"][data-option-id="trade_routes"]',
    )!;
    fireEvent.click(tradeOption);

    const footer = screen.getByTestId("objectives-scoring-footer");
    expect(footer).toHaveTextContent("Negotiate Trade Routes");

    // Simulate snapshot update arriving after 1-2 seconds with fresh object references
    rerender(
      <ObjectivesModal
        isOpen={true}
        onClose={vi.fn()}
        revealedObjectives={["lead", "trade_routes"]}
        players={[...mockPlayers]}
        viewerSeat="p1"
        choice={{
          ...choice,
          options: [
            { id: "lead", label: "Lead From the Front" },
            { id: "trade_routes", label: "Negotiate Trade Routes" },
          ],
        }}
        isScoringMode={true}
      />,
    );

    // Selection MUST remain Negotiate Trade Routes, NOT reset to Lead From the Front!
    expect(footer).toHaveTextContent("Negotiate Trade Routes");
  });

  describe("Imperial", () => {
    const imperial = (controlsMecatol: boolean) => ({
      actor: "p1",
      nonce: "imperial-1",
      prompt: "score a public objective with Imperial",
      context: { subtype: "imperial_score_objective" },
      options: [
        { id: "lead", label: "lead" },
        { id: "decline", kind: "decline", label: "decline" },
      ],
      details: {
        kind: "imperial",
        controls_mecatol: controlsMecatol,
        secrets_held: 2,
        secrets_max: 3,
      },
    });
    const open = (controlsMecatol: boolean, onSubmit = vi.fn().mockResolvedValue(undefined)) =>
      render(
        <ObjectivesModal
          isOpen={true}
          onClose={vi.fn()}
          revealedObjectives={["lead"]}
          players={mockPlayers}
          viewerSeat="p1"
          choice={imperial(controlsMecatol)}
          onSubmit={onSubmit}
        />,
      );

    it("always shows the Mecatol point when the seat holds Mecatol Rex", () => {
      open(true);
      expect(screen.getByTestId("imperial-outcome")).toHaveAttribute("data-variant", "mecatol");
      expect(screen.getByTestId("imperial-outcome-headline")).toHaveTextContent(
        "+1 VP (you hold Mecatol Rex)",
      );
    });

    it("shows the secret draw with the hand when it does not", () => {
      open(false);
      expect(screen.getByTestId("imperial-outcome")).toHaveAttribute("data-variant", "secret");
      expect(screen.getByTestId("imperial-outcome-headline")).toHaveTextContent(
        "Draw a secret objective (2/3 held)",
      );
    });

    it("repeats the outcome in the confirm summary, scored or skipped", () => {
      open(true);
      fireEvent.click(document.querySelector('[data-option-id="decline"]')!);
      expect(screen.getByTestId("objectives-scoring-footer")).toHaveTextContent(
        "Do not score (Decline) · +1 VP for Mecatol Rex",
      );
    });

    it("shows no outcome card for an ordinary scoring window", () => {
      render(
        <ObjectivesModal
          isOpen={true}
          onClose={vi.fn()}
          revealedObjectives={["lead"]}
          players={mockPlayers}
          viewerSeat="p1"
          choice={{ ...imperial(true), context: { subtype: "score_objective" } }}
          isScoringMode={true}
        />,
      );
      expect(screen.queryByTestId("imperial-outcome")).toBeNull();
    });
  });

  describe("bought objective progress", () => {
    const render1 = (props: Partial<React.ComponentProps<typeof ObjectivesModal>> = {}) =>
      render(
        <ObjectivesModal
          isOpen={true}
          onClose={vi.fn()}
          revealedObjectives={["monument", "sway_council", "corner"]}
          players={mockPlayers}
          viewerSeat="p1"
          objectiveProgress={{
            p1: {
              monument: { have: 5, threshold: 8, satisfied: false },
              sway_council: { have: 0, threshold: 8, satisfied: false },
              corner: { have: 3, threshold: 4, satisfied: false },
            },
          }}
          {...props}
        />,
      );

    it("labels spending capacity, unit-aware, and marks it as paid at scoring", () => {
      render1();
      const monument = screen.getByTestId("bought-progress-monument-p1");
      expect(monument).toHaveTextContent("can pay 5 / 8 resources now · pay at scoring");
      expect(monument).toHaveAttribute("title", expect.stringContaining("paid for when you score them"));
      expect(monument.getAttribute("aria-label")).toContain("ready planets and trade goods");
      expect(screen.getByTestId("bought-progress-sway_council-p1")).toHaveTextContent(
        "can pay 0 / 8 influence now · pay at scoring",
      );
    });

    it("leaves counting objectives as a plain N / M", () => {
      render1();
      expect(screen.queryByTestId("bought-progress-corner-p1")).toBeNull();
      expect(screen.getByTestId("status-progress-corner-p1")).toHaveTextContent("3 / 4");
    });

    it("drops the suffix for the seat whose scoring window is open", () => {
      render1({
        choice: {
          actor: "p1",
          nonce: "n",
          prompt: "Score",
          context: { subtype: "score_objective" },
          options: [{ id: "corner", label: "Corner" }],
        },
      });
      expect(screen.getByTestId("bought-progress-monument-p1")).toHaveTextContent(/^can pay 5 \/ 8 resources now$/);
    });

    it("keeps Ready when satisfied and Scored over progress", () => {
      render1({
        scoredObjectives: { p1: ["sway_council"] },
        objectiveProgress: {
          p1: {
            monument: { have: 8, threshold: 8, satisfied: true },
            sway_council: { have: 8, threshold: 8, satisfied: true },
          },
        },
      });
      expect(screen.getByTestId("status-ready-monument-p1")).toHaveTextContent("Ready (8/8)");
      expect(screen.getByTestId("status-scored-sway_council-p1")).toBeInTheDocument();
      expect(screen.queryByTestId("bought-progress-monument-p1")).toBeNull();
    });
  });
});
