import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { PendingChoiceModal } from "./PendingChoiceModal.tsx";
import { PendingChoiceDto } from "../protocol/types.ts";

const mockChoice: PendingChoiceDto = {
  prompt: "Select a Strategy Card",
  actor: "p1",
  nonce: "1234567890abcdef",
  options: [
    { id: "strat_leadership", label: "Leadership (1)", description: "Gain 3 command tokens" },
    { id: "strat_diplomacy", label: "Diplomacy (2)", description: "Ready 2 planets" },
  ],
  context: {
    kind: "strategy_draft",
    subtype: "draft_pick",
  },
};

describe("PendingChoiceModal Component", () => {
  it("shows the printed primary and secondary text for strategy draft cards", () => {
    const draft: PendingChoiceDto = {
      ...mockChoice,
      context: { subtype: "draft_strategy_card" },
      options: [{ id: "pok1leadership", kind: "strategy_card", label: "1. Leadership" }],
    };
    render(<PendingChoiceModal choice={draft} onSubmit={vi.fn()} />);
    expect(screen.getByTestId("choice-option")).toHaveClass("strategy-draft-card");
    expect(screen.getByText(/Gain 3 command tokens/)).toBeInTheDocument();
    expect(screen.getByText("Secondary")).toBeInTheDocument();
  });
  it("does not render when choice is null", () => {
    const { container } = render(<PendingChoiceModal choice={null} onSubmit={vi.fn()} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders prompt and options when choice is provided", () => {
    render(<PendingChoiceModal choice={mockChoice} onSubmit={vi.fn()} />);

    expect(screen.getByTestId("pending-choice-dialog")).toBeInTheDocument();
    expect(screen.getByTestId("choice-prompt")).toHaveTextContent("Select a Strategy Card");
    expect(screen.getAllByTestId("choice-option")).toHaveLength(2);
    expect(screen.getByText("Leadership (1)")).toBeInTheDocument();
    expect(screen.getByText("Diplomacy (2)")).toBeInTheDocument();
  });

  it("does not infer metadata from an opaque option ID", () => {
    const choiceWithoutDescription: PendingChoiceDto = {
      ...mockChoice,
      options: [{ id: "pok1leadership", label: "Take this option" }],
    };

    render(<PendingChoiceModal choice={choiceWithoutDescription} onSubmit={vi.fn()} />);

    expect(screen.getByText("Take this option")).toBeInTheDocument();
    expect(screen.queryByText(/Gain 3 command tokens/)).toBeNull();
  });

  it("submits selected option on confirm", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<PendingChoiceModal choice={mockChoice} onSubmit={onSubmit} />);

    // Select second option
    const secondOption = screen.getByLabelText(/Diplomacy \(2\)/i);
    fireEvent.click(secondOption);

    const submitBtn = screen.getByTestId("submit-choice-button");
    await act(async () => {
      fireEvent.click(submitBtn);
    });

    expect(onSubmit).toHaveBeenCalledWith("strat_diplomacy");
  });

  it("submits once until the server response resolves the submission", async () => {
    let resolveSubmission!: () => void;
    const onSubmit = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveSubmission = resolve;
        }),
    );
    render(<PendingChoiceModal choice={mockChoice} onSubmit={onSubmit} />);

    const submitButton = screen.getByTestId("submit-choice-button");
    fireEvent.click(submitButton);
    fireEvent.click(submitButton);
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(submitButton).toBeDisabled();

    await act(async () => resolveSubmission());
    expect(submitButton).not.toBeDisabled();
  });

  it("shows a refused submission and restores the submit button for retry", async () => {
    const onSubmit = vi
      .fn()
      .mockRejectedValueOnce(new Error("Rejected: Stale decision nonce"))
      .mockResolvedValueOnce(undefined);
    render(<PendingChoiceModal choice={mockChoice} onSubmit={onSubmit} />);
    await act(async () => {
      fireEvent.click(screen.getByTestId("submit-choice-button"));
    });
    expect(screen.getByRole("alert")).toHaveTextContent("Rejected: Stale decision nonce");
    expect(screen.getByTestId("submit-choice-button")).toBeEnabled();
    await act(async () => {
      fireEvent.click(screen.getByTestId("submit-choice-button"));
    });
    expect(onSubmit).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("moves focus into the dialog and minimizes on Escape", () => {
    render(<PendingChoiceModal choice={mockChoice} onSubmit={vi.fn()} />);
    const dialog = screen.getByTestId("pending-choice-dialog");
    expect(dialog).toHaveFocus();
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(screen.getByTestId("minimized-choice-banner")).toBeInTheDocument();
  });

  it("displays error banner when lastError is set", () => {
    render(
      <PendingChoiceModal
        choice={mockChoice}
        onSubmit={vi.fn()}
        lastError="Stale version mismatch"
      />,
    );

    const errorBanner = screen.getByTestId("choice-error-banner");
    expect(errorBanner).toBeInTheDocument();
    expect(errorBanner).toHaveTextContent("Stale version mismatch");
  });

  it("allows minimizing to inspect the map and reopening the dialog", () => {
    render(<PendingChoiceModal choice={mockChoice} onSubmit={vi.fn()} />);

    // Full modal is initially open
    expect(screen.getByTestId("pending-choice-dialog")).toBeInTheDocument();
    expect(screen.queryByTestId("minimized-choice-banner")).toBeNull();

    // Click minimize / inspect map button
    const minimizeBtn = screen.getByTestId("minimize-choice-button");
    fireEvent.click(minimizeBtn);

    // Full dialog is hidden, non-blocking floating banner is visible
    expect(screen.queryByTestId("pending-choice-dialog")).toBeNull();
    expect(screen.getByTestId("minimized-choice-banner")).toBeInTheDocument();
    expect(screen.queryByText(/Unknown participant/)).not.toBeInTheDocument();

    // Click resume button to restore dialog
    const resumeBtn = screen.getByTestId("resume-choice-button");
    fireEvent.click(resumeBtn);

    // Dialog is restored
    expect(screen.getByTestId("pending-choice-dialog")).toBeInTheDocument();
    expect(screen.queryByTestId("minimized-choice-banner")).toBeNull();
  });

  it("handles bounded multi-selection constraints with checkboxes and counter", async () => {
    const multiChoice: PendingChoiceDto = {
      prompt: "Select 2 technologies",
      actor: "p1",
      nonce: "nonce_multi",
      options: [
        { id: "tech_a", label: "Tech A" },
        { id: "tech_b", label: "Tech B" },
        { id: "tech_c", label: "Tech C" },
      ],
      context: {
        subtype: "research_technology",
        outstanding: [{ min_selection: 2, max_selection: 2 }],
      },
    };

    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const { rerender } = render(<PendingChoiceModal choice={multiChoice} onSubmit={onSubmit} />);

    // Renders checkboxes instead of radios
    const checkboxes = screen.getAllByRole("checkbox");
    expect(checkboxes).toHaveLength(3);

    const badge = screen.getByTestId("multi-selection-badge");
    expect(badge).toHaveTextContent("Selected: 0 of 2 (Minimum: 2)");

    const submitBtn = screen.getByTestId("submit-choice-button");
    expect(submitBtn).toBeDisabled();

    // Check first option
    fireEvent.click(checkboxes[0]);
    expect(badge).toHaveTextContent("Selected: 1 of 2 (Minimum: 2)");
    expect(submitBtn).toBeDisabled();

    // Check second option -> reaches required 2
    fireEvent.click(checkboxes[1]);
    expect(badge).toHaveTextContent("Selected: 2 of 2 (Minimum: 2)");
    expect(submitBtn).not.toBeDisabled();

    // Third option should be disabled since max is reached
    expect(checkboxes[2]).toBeDisabled();

    // Submit step 1
    await act(async () => {
      fireEvent.click(submitBtn);
    });
    expect(onSubmit).toHaveBeenNthCalledWith(1, "tech_a");

    // Simulate choice update from server after step 1 with new nonce and remaining options
    const secondChoice: PendingChoiceDto = {
      ...multiChoice,
      nonce: "nonce_multi_step_2",
      options: [
        { id: "tech_b", label: "Tech B" },
        { id: "tech_c", label: "Tech C" },
      ],
    };

    await act(async () => {
      rerender(<PendingChoiceModal choice={secondChoice} onSubmit={onSubmit} />);
    });
    expect(onSubmit).toHaveBeenNthCalledWith(2, "tech_b");
  });

  it("renders search filter when options >= 6 and filters list", () => {
    const largeChoice: PendingChoiceDto = {
      prompt: "Select an Action Card",
      actor: "p1",
      nonce: "nonce_large",
      options: [
        { id: "card_1", label: "Morale Boost", description: "Combat roll bonus" },
        { id: "card_2", label: "Shields Holding", description: "Cancel hits" },
        { id: "card_3", label: "Direct Hit", description: "Destroy sustained unit" },
        { id: "card_4", label: "Sabotage", description: "Cancel action card" },
        { id: "card_5", label: "Fighter Prototype", description: "Fighter bonus" },
        { id: "card_6", label: "Skilled Retreat", description: "Retreat ship" },
      ],
    };

    render(<PendingChoiceModal choice={largeChoice} onSubmit={vi.fn()} />);

    const searchInput = screen.getByTestId("choice-search-input");
    expect(searchInput).toBeInTheDocument();

    // Filter for "Direct"
    fireEvent.change(searchInput, { target: { value: "Direct" } });
    expect(screen.getByText("Direct Hit")).toBeInTheDocument();
    expect(screen.queryByText("Morale Boost")).toBeNull();
  });
});

describe("PendingChoiceModal context header", () => {
  it("shows what the decision is about and when it was asked", () => {
    render(
      <PendingChoiceModal
        choice={{
          actor: "seat_1",
          nonce: "ctx-1",
          prompt: "gain a command token into which pool",
          context: {
            subtype: "gain_command_token",
            source: { Rule: "52.4" },
            phase: "Status",
            round: 4,
          },
          options: [
            { id: "tactic_tokens", label: "tactic pool", kind: "pool" },
            { id: "fleet_tokens", label: "fleet pool", kind: "pool" },
          ],
        }}
        onSubmit={vi.fn()}
      />,
    );
    expect(screen.getByTestId("decision-eyebrow")).toHaveTextContent("Command tokens");
    expect(screen.getByTestId("decision-context-strip")).toHaveTextContent("Status phase · Round 4");
    expect(screen.getByTestId("decision-context-strip")).toHaveTextContent("Rule 52.4");
  });

  it("shows no context strip when the server sent no context", () => {
    render(
      <PendingChoiceModal
        choice={{
          actor: "seat_1",
          nonce: "ctx-2",
          prompt: "spend a strategy token to draw two action cards",
          options: [{ id: "yes", label: "draw" }],
        }}
        onSubmit={vi.fn()}
      />,
    );
    expect(screen.queryByTestId("decision-context-strip")).not.toBeInTheDocument();
  });
});

describe("PendingChoiceModal hand decisions", () => {
  it("shows the name, text and phase of each card offered for discard", () => {
    render(
      <PendingChoiceModal
        choice={{
          actor: "seat_1",
          nonce: "hand-1",
          prompt: "over the hand limit — discard one of 8",
          context: { subtype: "discard_over_hand_limit", source: { Rule: "2.4" } },
          options: [
            { id: "4", label: "Ancient Burial Sites", kind: "discard" },
            { id: "3", label: "Direct Hit", kind: "discard" },
          ],
        }}
        onSubmit={vi.fn()}
      />,
    );
    const texts = screen.getAllByTestId("card-option-text");
    expect(texts[0]).toHaveTextContent("Exhaust each cultural planet");
    expect(screen.getAllByTestId("card-option-badge")[0]).toHaveTextContent("Agenda phase");
    expect(screen.getByTestId("decision-eyebrow")).toHaveTextContent("Action card hand limit");
    expect(screen.getByText(/You hold 8 action cards/)).toBeInTheDocument();
    expect(screen.getByTestId("submit-choice-button")).toHaveTextContent("Discard card");
  });

  it("names secret objectives instead of showing their alias", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <PendingChoiceModal
        choice={{
          actor: "seat_1",
          nonce: "hand-2",
          prompt: "return a secret objective to the deck",
          context: { subtype: "return_over_secret_hand_limit", source: { Rule: "45.4" } },
          options: [
            { id: "baf", label: "return baf", kind: "return" },
            { id: "ans", label: "return ans", kind: "return" },
          ],
        }}
        onSubmit={onSubmit}
      />,
    );
    expect(screen.getByText("Betray a Friend")).toBeInTheDocument();
    expect(screen.queryByText("return baf")).not.toBeInTheDocument();
    expect(screen.getAllByTestId("card-option-badge")[0]).toHaveTextContent("1 VP");
    await act(async () => {
      fireEvent.click(screen.getByTestId("submit-choice-button"));
    });
    expect(onSubmit).toHaveBeenCalledWith("baf");
  });
});
