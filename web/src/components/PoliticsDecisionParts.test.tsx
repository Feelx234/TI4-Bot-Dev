import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { PendingChoiceModal } from "./PendingChoiceModal.tsx";
import { DecisionTableProvider } from "./PoliticsDecisionParts.tsx";
import type { PendingChoiceDto } from "../protocol/types.ts";
import type { DecisionTable } from "../presentation/politicsDecision.ts";

const table = {
  players: [
    { id: "a", faction: "sol", victory_points: 3, commodities: 1 },
    { id: "b", faction: "hacan", victory_points: 5, commodities: 0 },
  ],
  seating_order: ["a", "b"],
  speaker: "a",
} as unknown as DecisionTable;

const base = (subtype: string, extra: Partial<PendingChoiceDto>): PendingChoiceDto => ({
  prompt: "q",
  actor: "a",
  nonce: "n1",
  context: { subtype } as never,
  options: [],
  ...extra,
});

describe("Politics, Trade and Hacan agent decisions", () => {
  it("shows the agenda and keeps the options and submit button", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <PendingChoiceModal
        onSubmit={onSubmit}
        choice={base("politics_place_agenda", {
          prompt: "place mutiny where",
          details: { agenda: { name: "Mutiny", type: "Law", target: "For/Against", text1: "Printed text.", text2: "" } },
          options: [
            { id: "top", label: "on top of the deck" },
            { id: "bottom", label: "on the bottom" },
          ],
        })}
      />,
    );
    expect(screen.getByTestId("agenda-card-name")).toHaveTextContent("Mutiny");
    expect(screen.getByTestId("agenda-card-text")).toHaveTextContent("Printed text.");
    const rows = screen.getAllByTestId("choice-option");
    expect(rows.map((row) => row.getAttribute("data-option-id"))).toEqual(["top", "bottom"]);
    fireEvent.click(screen.getAllByRole("radio")[1]);
    fireEvent.click(screen.getByTestId("submit-choice-button"));
    await vi.waitFor(() => expect(onSubmit).toHaveBeenCalledWith("bottom"));
  });

  it("describes speaker candidates and submits the same option id", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <DecisionTableProvider table={table}>
        <PendingChoiceModal
          onSubmit={onSubmit}
          choice={base("politics_choose_speaker", {
            prompt: "who becomes speaker",
            details: { seats: { Hacan: "b" } },
            options: [{ id: "Hacan", label: "Hacan becomes speaker" }],
          })}
        />
      </DecisionTableProvider>,
    );
    expect(screen.getByTestId("choice-option-note")).toHaveTextContent(/Hacan.*5 VP.*2nd in speaker order/);
    fireEvent.click(screen.getByTestId("submit-choice-button"));
    await vi.waitFor(() => expect(onSubmit).toHaveBeenCalledWith("Hacan"));
  });

  it("shows commodities for replenish and a reason for the Hacan agent", () => {
    const { unmount } = render(
      <DecisionTableProvider table={table}>
        <PendingChoiceModal
          onSubmit={vi.fn()}
          choice={base("trade_choose_replenish", {
            details: { seats: { Hacan: "b" } },
            options: [
              { id: "Hacan", label: "Hacan replenishes commodities", kind: "replenish" },
              { id: "done", label: "nobody else replenishes", kind: "decline" },
            ],
          })}
        />
      </DecisionTableProvider>,
    );
    expect(screen.getAllByTestId("choice-option-note")[0]).toHaveTextContent("0 commodities held");
    expect(screen.getAllByTestId("choice-option")).toHaveLength(2);
    unmount();
    render(
      <DecisionTableProvider table={table}>
        <PendingChoiceModal
          onSubmit={vi.fn()}
          choice={base("leader_hacanagent_branch", {
            options: [
              { id: "self", label: "gain 2 commodities" },
              { id: "b", label: "replenish b" },
            ],
          })}
        />
      </DecisionTableProvider>,
    );
    expect(screen.getByTestId("hacan-reason")).toHaveTextContent(/holds 0/);
    expect(screen.getAllByTestId("choice-option-note")[1]).toHaveTextContent(/Hacan/);
    expect(screen.getByTestId("submit-choice-button")).toBeInTheDocument();
  });
});
