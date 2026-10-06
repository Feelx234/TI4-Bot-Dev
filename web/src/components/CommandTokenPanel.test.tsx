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

const purchase = {
  cost: 3,
  influence_available: 9,
  max: 3,
  trade_goods: 7,
  trade_good_worth: 1,
  planets: [{ id: "jord", worth: 2 }],
};
const gainBuyChoice = (): PendingChoiceDto => {
  const choice = gainChoice(3);
  return { ...choice, nonce: "gain-buy-1", details: { ...choice.details, purchase } };
};

describe("command token panel (gain and buy)", () => {
  it("plans the free tokens, the purchases and every pool on one screen", async () => {
    const onSubmitBatch = vi.fn().mockResolvedValue(undefined);
    render(<PendingChoiceModal choice={gainBuyChoice()} onSubmit={vi.fn()} onSubmitBatch={onSubmitBatch} />);
    expect(screen.getByTestId("token-total")).toHaveTextContent("3");
    expect(screen.getByTestId("token-buy")).toHaveTextContent("3 influence each");
    expect(screen.getByTestId("token-influence")).toHaveTextContent("Influence available 9");
    expect(screen.getByTestId("token-buy-minus")).toBeDisabled();

    await click("token-buy-plus");
    await click("token-buy-plus");
    expect(screen.getByTestId("token-total")).toHaveTextContent("5");
    expect(screen.getByTestId("token-split")).toHaveTextContent("3 free + 2 bought");
    expect(screen.getByTestId("token-influence-spent")).toHaveTextContent("6");
    expect(screen.getByTestId("token-payment")).toHaveTextContent("jord (2) + 4 trade goods");
    expect(screen.getByTestId("token-remaining")).toHaveTextContent("5");
    expect(screen.getByTestId("token-confirm")).toBeDisabled();

    for (const pool of ["tactic", "tactic", "fleet", "fleet", "strategic"]) {
      await click(`token-plus-${pool}`);
    }
    expect(screen.getByTestId("token-remaining")).toHaveTextContent("0");
    await click("token-confirm");
    const sent = onSubmitBatch.mock.calls[0][0];
    expect(sent.kind).toBe("tokens");
    expect(sent.steps.filter((step: { kind: string }) => step.kind === "purchase")).toEqual([
      { kind: "purchase", buy: true },
      { kind: "purchase", buy: true },
      { kind: "purchase", buy: false },
    ]);
    expect(sent.steps).toHaveLength(3 + 2 * 2 + 5 + 1);
  });

  it("stops the stepper at what is affordable and drops staged tokens when buying fewer", async () => {
    render(<PendingChoiceModal choice={gainBuyChoice()} onSubmit={vi.fn()} onSubmitBatch={vi.fn()} />);
    for (let i = 0; i < 5; i += 1) await click("token-buy-plus");
    expect(screen.getByTestId("token-buy-count")).toHaveTextContent("3");
    expect(screen.getByTestId("token-buy-plus")).toBeDisabled();
    expect(screen.getByTestId("token-influence-spent")).toHaveTextContent("9");
    for (let i = 0; i < 6; i += 1) await click("token-plus-fleet");
    await click("token-buy-minus");
    expect(screen.getByTestId("token-remaining")).toHaveTextContent("0");
    expect(screen.getByTestId("token-pips-fleet")).toHaveAttribute("data-added", "5");
    await click("token-reset");
    expect(screen.getByTestId("token-buy-count")).toHaveTextContent("0");
    expect(screen.getByTestId("token-total")).toHaveTextContent("3");
  });

  it("is a plain list again without a batch submitter", () => {
    render(<PendingChoiceModal choice={gainBuyChoice()} onSubmit={vi.fn()} />);
    expect(screen.queryByTestId("command-token-panel")).not.toBeInTheDocument();
  });

  it("shows the secondary window's purchase on the same panel and answers it as one plan", async () => {
    const onSubmitBatch = vi.fn().mockResolvedValue(undefined);
    const window: PendingChoiceDto = {
      actor: "seat_1",
      nonce: "secondary-1",
      prompt: "spend 3 influence for a command token",
      options: [
        { id: "no", kind: "strategy", label: "spend nothing further" },
        { id: "yes", kind: "strategy", label: "spend 3 influence" },
      ],
      context: { subtype: "buy_token_with_influence" } as PendingChoiceDto["context"],
      details: {
        kind: "strategy_secondary",
        card: "pok1leadership",
        played_by: "seat_2",
        mode: "buy",
        pools: { tactic: 3, fleet: 4, strategic: 2 },
        reinforcements: 7,
        tokens_to_place: 0,
        purchase,
      },
    };
    render(<PendingChoiceModal choice={window} onSubmit={vi.fn()} onSubmitBatch={onSubmitBatch} />);
    expect(screen.getByTestId("token-confirm")).toHaveTextContent("No purchase");
    await click("token-buy-plus");
    await click("token-plus-strategic");
    expect(screen.getByTestId("token-confirm")).toHaveTextContent("Confirm tokens and purchase");
    await click("token-confirm");
    expect(onSubmitBatch).toHaveBeenCalledWith({
      kind: "tokens",
      steps: [
        { kind: "purchase", buy: true },
        { kind: "exhaust", planet: "jord" },
        { kind: "trade_good" },
        { kind: "pool", pool: "strategic_tokens" },
        { kind: "purchase", buy: false },
      ],
    });
  });
});
