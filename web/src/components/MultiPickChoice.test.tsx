import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import type { PendingChoiceDto, PlayerView } from "../protocol/types.ts";
import { partialContext } from "../test/wire.ts";
import { PendingChoiceModal } from "./PendingChoiceModal.tsx";
import { PaymentDrawer } from "./PaymentDrawer.tsx";
import { deriveChoiceRendererModel } from "../presentation/choiceModel.ts";
import { isPaymentChoice } from "../presentation/paymentDraft.ts";

const option = (id: string, label: string, payload: Record<string, unknown> = {}, kind = "card") => ({
  id,
  kind,
  label,
  payload,
});

const discard: PendingChoiceDto = {
  prompt: "discard 2 action cards",
  actor: "p1",
  nonce: "discard-1",
  context: partialContext({ subtype: "discard_over_hand_limit" }),
  options: [
    option("sabotage", "Sabotage"),
    option("direct_hit", "Direct Hit"),
    option("skilled_retreat", "Skilled Retreat"),
  ],
  multi_pick: { kind: "discard", owed: 2, repeat: { sabotage: 2 } },
};

const toggle = (id: string) =>
  act(() => {
    fireEvent.click(within(screen.getByTestId("pending-choice-dialog")).getByDisplayValue(id));
  });

describe("a multi-pick in the decision modal", () => {
  it("builds a discard as one '+'-joined answer, checking the count as you pick", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<PendingChoiceModal choice={discard} onSubmit={onSubmit} />);
    const tally = screen.getByTestId("multi-pick-tally");
    expect(tally).toHaveAttribute("data-rule", "discard");
    expect(screen.getByTestId("multi-pick-total")).toHaveTextContent("0 of 2");
    expect(screen.getByTestId("submit-choice-button")).toBeDisabled();
    toggle("direct_hit");
    expect(screen.getByTestId("multi-pick-problem")).toHaveTextContent("Pick 1 more");
    // A card held twice steps up to its copies.
    await act(async () => {
      fireEvent.click(screen.getByTestId("multi-pick-plus-sabotage"));
    });
    expect(screen.getByTestId("multi-pick-count-sabotage")).toHaveTextContent("1 / 2");
    expect(screen.getByTestId("multi-pick-total")).toHaveTextContent("2 of 2");
    expect(screen.getByTestId("submit-choice-button")).toHaveTextContent("Discard 2");
    await act(async () => {
      fireEvent.click(screen.getByTestId("multi-pick-plus-sabotage"));
    });
    expect(screen.getByTestId("multi-pick-problem")).toHaveTextContent("take 1 out");
    await act(async () => {
      fireEvent.click(screen.getByTestId("multi-pick-minus-sabotage"));
    });
    await act(async () => {
      fireEvent.click(screen.getByTestId("submit-choice-button"));
    });
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith("sabotage+direct_hit");
  });

  it("lets the player pick freely for a kind it does not know and leaves the check to the engine", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const remove: PendingChoiceDto = {
      prompt: "remove units over capacity",
      actor: "p1",
      nonce: "remove-1",
      context: partialContext({ subtype: "remove_over_capacity" }),
      options: [option("fighter|a", "a fighter", {}, "remove"), option("infantry|a", "an infantry", {}, "remove")],
      multi_pick: { kind: "something_new", owed: 0 },
    };
    render(<PendingChoiceModal choice={remove} onSubmit={onSubmit} />);
    expect(screen.getByTestId("multi-pick-problem")).toHaveTextContent("Pick at least one");
    toggle("fighter|a");
    toggle("infantry|a");
    expect(screen.getByTestId("multi-pick-engine-note")).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(screen.getByTestId("submit-choice-button"));
    });
    expect(onSubmit).toHaveBeenCalledWith("fighter|a+infantry|a");
  });

  it("removes units over the fleet supply: a stepper per unit kind up to the excess, one answer", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const remove: PendingChoiceDto = {
      prompt: "remove ships over the fleet supply",
      actor: "p1",
      nonce: "remove-2",
      context: partialContext({ subtype: "remove_over_fleet_supply" }),
      options: [option("remove|0", "remove destroyer", {}, "remove"), option("remove|1", "remove cruiser", {}, "remove")],
      multi_pick: { kind: "remove", owed: 2 },
    };
    render(<PendingChoiceModal choice={remove} onSubmit={onSubmit} />);
    expect(screen.getByTestId("multi-pick-total")).toHaveTextContent("0 · over by 2");
    await act(async () => {
      fireEvent.click(screen.getByTestId("multi-pick-plus-remove|0"));
    });
    expect(screen.getByTestId("multi-pick-engine-note")).toHaveTextContent("the game removes more");
    await act(async () => {
      fireEvent.click(screen.getByTestId("multi-pick-plus-remove|0"));
    });
    expect(screen.getByTestId("multi-pick-plus-remove|0")).toBeDisabled();
    await act(async () => {
      fireEvent.click(screen.getByTestId("submit-choice-button"));
    });
    expect(onSubmit).toHaveBeenCalledWith("remove|0+remove|0");
  });

  it("runs a payment's rule as you type: short, more than needed, then a legal set", async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const pay: PendingChoiceDto = {
      prompt: "pay 5 more resources",
      actor: "p1",
      nonce: "pay-generic",
      context: partialContext({ subtype: "some_payment" }),
      options: [
        option("exhaust|jord", "exhaust jord for 4 resources", { worth: 4, kind: "resources" }, "pay_other"),
        option("exhaust|abyz", "exhaust abyz for 3 resources", { worth: 3, kind: "resources" }, "pay_other"),
        option("trade_good", "spend a trade good", { worth: 1, kind: "resources" }, "pay_other"),
      ],
      multi_pick: { kind: "payment", owed: 5, repeat: { trade_good: 3 } },
    };
    render(<PendingChoiceModal choice={pay} onSubmit={onSubmit} />);
    toggle("exhaust|jord");
    expect(screen.getByTestId("multi-pick-problem")).toHaveTextContent("Short by 1 resources");
    toggle("exhaust|abyz");
    expect(screen.queryByTestId("multi-pick-problem")).not.toBeInTheDocument();
    // 4 + 3 + 1 = 8 for 5 is still a set the engine takes (without the 4, 4 is short); a second good is not.
    for (let i = 0; i < 2; i += 1) {
      await act(async () => {
        fireEvent.click(screen.getByTestId("multi-pick-plus-trade_good"));
      });
    }
    expect(screen.getByTestId("multi-pick-problem")).toHaveTextContent("More than needed");
    for (let i = 0; i < 2; i += 1) {
      await act(async () => {
        fireEvent.click(screen.getByTestId("multi-pick-minus-trade_good"));
      });
    }
    await act(async () => {
      fireEvent.click(screen.getByTestId("submit-choice-button"));
    });
    expect(onSubmit).toHaveBeenCalledWith("exhaust|jord+exhaust|abyz");
  });
});

const player = { id: "p1", faction: "sol", trade_goods: 3 } as unknown as PlayerView;
const payResources: PendingChoiceDto = {
  prompt: "pay 4 more resources",
  actor: "p1",
  nonce: "pay-1",
  context: partialContext({ subtype: "pay_resources", outstanding: [{ kind: "Resources", amount: 4, paid: 0 }] }),
  options: [
    { id: "exhaust|jord", kind: "pay", label: "exhaust jord for 4 resources", payload: { worth: 4, kind: "resources", planet_name: "Jord" } },
    { id: "exhaust|arinam", kind: "pay", label: "exhaust arinam for 1 resources", payload: { worth: 1, kind: "resources", planet_name: "Arinam" } },
    { id: "trade_good", kind: "pay", label: "spend a trade good", payload: { worth: 1, kind: "resources" } },
  ],
  multi_pick: { kind: "payment", owed: 4, repeat: { trade_good: 3 } },
};

describe("a multi-pick payment in the payment drawer", () => {
  const renderDrawer = (onSubmit = vi.fn().mockResolvedValue(undefined), onSubmitBatch = vi.fn()) => {
    render(
      <PaymentDrawer
        choice={payResources}
        player={player}
        viewerSeat="p1"
        onSubmit={onSubmit}
        onSubmitBatch={onSubmitBatch}
        isOpen
        onClose={vi.fn()}
      />,
    );
    return { onSubmit, onSubmitBatch };
  };
  const pick = (id: string) =>
    act(() => {
      fireEvent.click(screen.getByTestId(`planet-card-${id}`).querySelector("input")!);
    });

  it("sends the whole set as one id, not a batch", async () => {
    const { onSubmit, onSubmitBatch } = renderDrawer();
    pick("exhaust|arinam");
    await act(async () => {
      fireEvent.click(screen.getByTestId("tg-increment-btn"));
    });
    await act(async () => {
      fireEvent.click(screen.getByTestId("tg-increment-btn"));
    });
    await act(async () => {
      fireEvent.click(screen.getByTestId("tg-increment-btn"));
    });
    expect(screen.getByTestId("confirm-payment-btn")).toBeEnabled();
    await act(async () => {
      fireEvent.click(screen.getByTestId("confirm-payment-btn"));
    });
    expect(onSubmitBatch).not.toHaveBeenCalled();
    expect(onSubmit).toHaveBeenCalledWith("exhaust|arinam+trade_good+trade_good+trade_good");
  });

  it("blocks a set with an item the bill did not need, and says which", async () => {
    renderDrawer();
    // Jord 4 + Arinam 1 + 3 goods = 8 for 4: without Jord the rest (4) still covers it.
    pick("exhaust|jord");
    pick("exhaust|arinam");
    for (let i = 0; i < 3; i += 1) {
      await act(async () => {
        fireEvent.click(screen.getByTestId("tg-increment-btn"));
      });
    }
    expect(screen.getByTestId("payment-problem")).toHaveTextContent("More than needed");
    expect(screen.getByTestId("payment-problem")).toHaveTextContent("without Jord");
    expect(screen.getByTestId("confirm-payment-btn")).toBeDisabled();
  });

  it("keeps the payment workflow for payments and leaves Leadership's purchase to the token panel", () => {
    expect(deriveChoiceRendererModel(payResources, "p1")?.workflow).toBe("payment");
    const purchase: PendingChoiceDto = {
      ...payResources,
      context: partialContext({ subtype: "buy_token_with_influence" }),
      multi_pick: { kind: "purchase", owed: 3, buy: [0, 1] },
    };
    expect(deriveChoiceRendererModel(purchase, "p1")?.workflow).not.toBe("payment");
    expect(isPaymentChoice(purchase)).toBe(false);
    expect(isPaymentChoice(payResources)).toBe(true);
  });
});
