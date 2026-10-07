import React, { useState } from "react";
import { act } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Board } from "./Board.tsx";
import { PendingChoiceModal } from "./PendingChoiceModal.tsx";
import type { BoardView, PendingChoiceDto } from "../protocol/types.ts";
import {
  CommandTokenDraftProvider,
  useCommandTokenDraft,
} from "../presentation/CommandTokenDraftContext.tsx";

const board: BoardView = {
  systems: {
    "34": {
      system_id: "34",
      coordinate: "<1, 0, -1>",
      tile_type: "normal",
      planets: {
        abyz: { planet_id: "abyz", controlled_by: "p1", exhausted: false, attachments: [] },
        fria: { planet_id: "fria", controlled_by: "p1", exhausted: false, attachments: [] },
        arnor: { planet_id: "arnor", controlled_by: "p2", exhausted: false, attachments: [] },
      },
      units: [],
      command_tokens: [],
    },
  },
};

const choice: PendingChoiceDto = {
  actor: "p1",
  nonce: "lead-map-1",
  prompt: "gain a command token into which pool",
  options: ["tactic_tokens", "fleet_tokens", "strategic_tokens"].map((id) => ({
    id,
    kind: "pool",
    label: id,
  })),
  context: { subtype: "gain_command_token" } as PendingChoiceDto["context"],
  details: {
    kind: "command_tokens",
    mode: "gain",
    pools: { tactic: 3, fleet: 4, strategic: 2 },
    reinforcements: 9,
    tokens_to_place: 1,
    purchase: {
      cost: 3,
      influence_available: 7,
      max: 2,
      trade_goods: 2,
      trade_good_worth: 1,
      // abyz pays 3, fria 2: one token is covered by abyz alone.
      planets: [
        { id: "abyz", worth: 3 },
        { id: "fria", worth: 2 },
      ],
    },
  },
};

/** Wires the map, the modal and the shared draft the way App does. */
const Harness: React.FC<{ onSubmitBatch: (plan: unknown) => Promise<void> }> = ({ onSubmitBatch }) => {
  const tokenDraft = useCommandTokenDraft(choice);
  const [minimized, setMinimized] = useState(false);
  return (
    <CommandTokenDraftProvider value={tokenDraft}>
      <Board
        board={board}
        seatingOrder={["p1", "p2"]}
        pendingChoice={choice}
        viewerSeat="p1"
        onSelectTarget={(_system, planetId) => {
          if (
            tokenDraft.mapPayment &&
            planetId &&
            tokenDraft.mapPayment.offer.planets.some((p) => p.planetId === planetId)
          ) {
            tokenDraft.togglePlanet(planetId);
          }
        }}
      />
      <PendingChoiceModal
        choice={choice}
        onSubmit={vi.fn()}
        onSubmitBatch={onSubmitBatch as never}
        isMinimized={minimized}
        onMinimizedChange={setMinimized}
      />
    </CommandTokenDraftProvider>
  );
};

const click = async (id: string) =>
  act(async () => {
    fireEvent.click(screen.getByTestId(id));
  });

const buyOneAndAssign = async () => {
  await click("token-buy-plus");
  await click("token-plus-tactic");
  await click("token-plus-tactic");
};

describe("Leadership purchase paid on the map", () => {
  it("has no payment targets until tokens are bought", () => {
    render(<Harness onSubmitBatch={vi.fn()} />);
    expect(screen.queryByTestId("payment-mark-abyz")).not.toBeInTheDocument();
  });

  it("rings the planets that can pay, with Auto-pay's pick staged, and not the others", async () => {
    render(<Harness onSubmitBatch={vi.fn()} />);
    await click("token-buy-plus");
    expect(screen.getByTestId("payment-mark-abyz")).toHaveTextContent("✓ 3I");
    expect(screen.getByTestId("payment-mark-fria")).toHaveTextContent("2I");
    expect(screen.getByTestId("planet-abyz")).toHaveAttribute("data-payment-staged", "true");
    expect(screen.getByTestId("planet-fria")).toHaveAttribute("data-payment-staged", "false");
    expect(screen.queryByTestId("payment-mark-arnor")).not.toBeInTheDocument();
  });

  it("toggles a planet on and off from the map and keeps the panel's list in sync", async () => {
    render(<Harness onSubmitBatch={vi.fn()} />);
    await buyOneAndAssign();
    // Auto-pay: abyz alone. Add fria: 5 paid against 3 is more than the bill needs.
    await click("planet-fria");
    expect(screen.getByTestId("planet-fria")).toHaveAttribute("data-payment-staged", "true");
    expect(screen.getByTestId("token-payment-planet-fria")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByTestId("token-payment-account")).toHaveTextContent("Paid 5 · owed 3");
    expect(screen.getByTestId("token-payment-problem")).toHaveTextContent("More than needed");
    expect(screen.getByTestId("token-confirm")).toBeDisabled();
    // Take abyz out again: 2 of 3, short.
    await click("planet-abyz");
    expect(screen.getByTestId("planet-abyz")).toHaveAttribute("data-payment-staged", "false");
    expect(screen.getByTestId("token-payment-planet-abyz")).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByTestId("token-payment-problem")).toHaveTextContent("Short by 1");
    expect(screen.getByTestId("token-confirm")).toBeDisabled();
    // The list controls work too and the map follows.
    await click("token-payment-planet-abyz");
    await click("token-payment-planet-fria");
    expect(screen.getByTestId("planet-abyz")).toHaveAttribute("data-payment-staged", "true");
    expect(screen.getByTestId("planet-fria")).toHaveAttribute("data-payment-staged", "false");
    expect(screen.getByTestId("token-confirm")).toBeEnabled();
  });

  it("shows paid against the bill in the bar while minimised, and confirms only when covered", async () => {
    const onSubmitBatch = vi.fn().mockResolvedValue(undefined);
    render(<Harness onSubmitBatch={onSubmitBatch} />);
    await buyOneAndAssign();
    await click("token-pay-on-map");
    expect(screen.getByTestId("token-payment-bar")).toBeInTheDocument();
    expect(screen.getByTestId("token-bar-paid")).toHaveTextContent("3");
    expect(screen.getByTestId("token-bar-remaining")).toHaveTextContent("Covered");
    expect(screen.getByTestId("token-bar-confirm")).toBeEnabled();

    // Insufficient: abyz off, fria on (2 of 3).
    await click("planet-abyz");
    await click("planet-fria");
    expect(screen.getByTestId("token-bar-paid")).toHaveTextContent("2");
    expect(screen.getByTestId("token-bar-remaining")).toHaveTextContent("1 remaining");
    expect(screen.getByTestId("token-bar-confirm")).toBeDisabled();
    // A trade good covers the gap.
    await click("token-bar-goods-plus");
    expect(screen.getByTestId("token-bar-remaining")).toHaveTextContent("Covered");
    expect(screen.getByTestId("token-bar-confirm")).toBeEnabled();
    // Over-paying is blocked like in the panel: abyz on top of fria + a trade good.
    await click("planet-abyz");
    expect(screen.getByTestId("token-bar-remaining")).toHaveTextContent("Overpaying by 3");
    expect(screen.getByTestId("token-bar-confirm")).toBeDisabled();
    await click("planet-fria");
    await click("token-bar-goods-minus");

    expect(onSubmitBatch).not.toHaveBeenCalled();
    await click("token-bar-confirm");
    expect(onSubmitBatch).toHaveBeenCalledTimes(1);
    const steps = onSubmitBatch.mock.calls[0][0].steps as Array<{ kind: string; planet?: string }>;
    expect(steps.filter((s) => s.kind === "exhaust")).toEqual([{ kind: "exhaust", planet: "abyz" }]);
  });

  it("keeps the staged purchase and payment when the panel is minimised and reopened", async () => {
    render(<Harness onSubmitBatch={vi.fn()} />);
    await buyOneAndAssign();
    await click("planet-fria");
    await click("planet-abyz");
    await click("token-pay-on-map");
    expect(screen.queryByTestId("command-token-panel")).not.toBeInTheDocument();
    await click("resume-choice-button");
    expect(screen.getByTestId("token-buy-count")).toHaveTextContent("1");
    expect(screen.getByTestId("token-count-tactic")).toHaveTextContent("5");
    expect(screen.getByTestId("token-payment-planet-fria")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByTestId("token-payment-planet-abyz")).toHaveAttribute("aria-pressed", "false");
  });

  it("does not offer the planets of another player", async () => {
    render(<Harness onSubmitBatch={vi.fn()} />);
    await click("token-buy-plus");
    expect(screen.getByTestId("planet-arnor")).not.toHaveAttribute("data-payment-staged");
  });
});
