import { act } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PendingChoiceModal } from "./PendingChoiceModal.tsx";
import type { PendingChoiceDto } from "../protocol/types.ts";

const pool = (id: string) => ({ id, kind: "pool", label: id });
const gainChoice = (toPlace = 2): PendingChoiceDto => ({
  actor: "seat_1",
  nonce: "gain-1",
  prompt: "gain a command token into which pool",
  options: ["tactic_tokens", "fleet_tokens", "strategic_tokens"].map(pool),
  context: { subtype: "gain_command_token" } as PendingChoiceDto["context"],
  details: {
    kind: "command_tokens",
    mode: "gain",
    pools: { tactic: 3, fleet: 4, strategic: 2 },
    reinforcements: 7,
    tokens_to_place: toPlace,
  },
});
const redistributeChoice = (): PendingChoiceDto => ({
  actor: "seat_1",
  nonce: "redis-1",
  prompt: "redistribute your command tokens",
  options: ["3|4|2", "2|5|2", "2|4|3"].map((id) => ({ id, kind: "redistribute", label: id })),
  context: { subtype: "status_redistribute_tokens" } as PendingChoiceDto["context"],
  details: {
    kind: "command_tokens",
    mode: "redistribute",
    pools: { tactic: 3, fleet: 4, strategic: 2 },
    reinforcements: 7,
    total: 9,
  },
});
const click = async (id: string) =>
  act(async () => {
    fireEvent.click(screen.getByTestId(id));
  });

describe("command token panel (gain)", () => {
  it("shows pools, counts, the total, the remaining tokens and pips", async () => {
    render(<PendingChoiceModal choice={gainChoice()} onSubmit={vi.fn()} onSubmitBatch={vi.fn()} />);
    expect(screen.getByTestId("token-total")).toHaveTextContent("2");
    expect(screen.getByTestId("token-remaining")).toHaveTextContent("2");
    expect(screen.getByTestId("token-reinforcements")).toHaveTextContent("7");
    expect(screen.getByTestId("token-count-fleet")).toHaveTextContent("4");
    await click("token-plus-fleet");
    expect(screen.getByTestId("token-count-fleet")).toHaveTextContent("5");
    expect(screen.getByTestId("token-remaining")).toHaveTextContent("1");
    const pips = screen.getByTestId("token-pips-fleet");
    expect(pips).toHaveAttribute("data-kept", "4");
    expect(pips).toHaveAttribute("data-added", "1");
    expect(screen.queryByTestId("choice-option")).not.toBeInTheDocument();
  });

  it("keeps Confirm disabled until every token is assigned, then sends one tokens batch", async () => {
    const onSubmitBatch = vi.fn().mockResolvedValue(undefined);
    render(
      <PendingChoiceModal choice={gainChoice()} onSubmit={vi.fn()} onSubmitBatch={onSubmitBatch} />,
    );
    expect(screen.getByTestId("token-confirm")).toBeDisabled();
    await click("token-plus-strategic");
    expect(screen.getByTestId("token-confirm")).toBeDisabled();
    await click("token-plus-tactic");
    expect(screen.getByTestId("token-confirm")).toBeEnabled();
    expect(screen.getByTestId("token-plus-fleet")).toBeDisabled();
    await click("token-confirm");
    expect(onSubmitBatch).toHaveBeenCalledTimes(1);
    expect(onSubmitBatch).toHaveBeenCalledWith({
      kind: "tokens",
      steps: [
        { kind: "pool", pool: "tactic_tokens" },
        { kind: "pool", pool: "strategic_tokens" },
      ],
    });
  });

  it("minus and Reset take staged tokens back", async () => {
    render(<PendingChoiceModal choice={gainChoice()} onSubmit={vi.fn()} onSubmitBatch={vi.fn()} />);
    expect(screen.getByTestId("token-minus-tactic")).toBeDisabled();
    await click("token-plus-tactic");
    await click("token-minus-tactic");
    expect(screen.getByTestId("token-remaining")).toHaveTextContent("2");
    await click("token-plus-tactic");
    await click("token-plus-fleet");
    await click("token-reset");
    expect(screen.getByTestId("token-remaining")).toHaveTextContent("2");
    expect(screen.getByTestId("token-count-tactic")).toHaveTextContent("3");
  });

  it("re-syncs after a rejected or interrupted plan: error shown, staging cleared", async () => {
    const onSubmitBatch = vi.fn().mockRejectedValue(new Error("Batch rejected: moved on"));
    render(
      <PendingChoiceModal choice={gainChoice()} onSubmit={vi.fn()} onSubmitBatch={onSubmitBatch} />,
    );
    await click("token-plus-tactic");
    await click("token-plus-tactic");
    await click("token-confirm");
    expect(screen.getByTestId("token-error")).toHaveTextContent("moved on");
    expect(screen.getByTestId("token-remaining")).toHaveTextContent("2");
    expect(screen.getByTestId("token-confirm")).toBeDisabled();
  });

  it("falls back to the plain list when no batch submitter is supplied", () => {
    render(<PendingChoiceModal choice={gainChoice()} onSubmit={vi.fn()} />);
    expect(screen.queryByTestId("command-token-panel")).not.toBeInTheDocument();
    expect(screen.getAllByTestId("choice-option")).toHaveLength(3);
  });
});

describe("command token panel (redistribute)", () => {
  it("starts on the current arrangement and submits the matching option id", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<PendingChoiceModal choice={redistributeChoice()} onSubmit={onSubmit} />);
    expect(screen.getByTestId("token-remaining")).toHaveTextContent("0");
    expect(screen.getByTestId("token-confirm")).toBeEnabled();
    await click("token-minus-tactic");
    expect(screen.getByTestId("token-remaining")).toHaveTextContent("1");
    expect(screen.getByTestId("token-confirm")).toBeDisabled();
    expect(screen.getByTestId("token-pips-tactic")).toHaveAttribute("data-removed", "1");
    await click("token-plus-fleet");
    await click("token-confirm");
    expect(onSubmit).toHaveBeenCalledWith("2|5|2");
  });

  it("blocks an arrangement the engine does not offer", async () => {
    render(<PendingChoiceModal choice={redistributeChoice()} onSubmit={vi.fn()} />);
    await click("token-minus-tactic");
    await click("token-plus-tactic");
    await click("token-minus-strategic");
    await click("token-plus-strategic");
    await click("token-minus-fleet");
    await click("token-plus-tactic");
    expect(screen.getByTestId("token-confirm")).toBeDisabled();
    expect(screen.getByTestId("token-blocker")).toBeInTheDocument();
  });
});
