import { describe, expect, it } from "vitest";
import type { ChoiceOption, PendingChoiceDto } from "../protocol/types.ts";
import { buildCosts } from "./productionPayment.ts";
import {
  agentDiscount,
  draftResourceCost,
  optionUnit,
  planBuilds,
  produceStep,
  productionTotal,
  stepUnitKey,
} from "./productionDraft.ts";

const build: ChoiceOption = {
  id: "build|carrier|1",
  kind: "produce",
  label: "produce 1x carrier for 3",
  payload: { unit: "carrier", count: 1, cost: 3, printed_cost: 3, discount: 0, production_spent: 1, available_resources: 6 },
};
const exchange: ChoiceOption = {
  id: "exchange|carrier",
  kind: "produce",
  label: "produce 1x carrier by returning a captured carrier",
  payload: { unit: "carrier", count: 1, cost: 0, printed_cost: 3, discount: 0, production_spent: 1, available_resources: 6, exchange: true },
};
const choice = {
  actor: "a",
  nonce: "n",
  prompt: "produce",
  context: { subtype: "produce_unit", target: { System: "14" }, outstanding: [{ amount: 3, paid: 0 }] },
  options: [build, exchange, { id: "done_producing", kind: "decline", label: "done", payload: {} }],
} as unknown as PendingChoiceDto;

describe("exchange vs paid build of one unit type", () => {
  it("keys them differently and sends the flag only for the exchange", () => {
    expect(optionUnit(build)).toBe("carrier");
    expect(optionUnit(exchange)).toBe("exchange|carrier");
    expect(produceStep(build)).toEqual({ kind: "produce", unit: "carrier", count: 1 });
    expect(produceStep(exchange)).toEqual({ kind: "produce", unit: "carrier", count: 1, exchange: true });
    expect(stepUnitKey(produceStep(exchange))).toBe("exchange|carrier");
    expect(stepUnitKey(produceStep(build))).toBe("carrier");
  });
  it("plans a prepared build of either kind without calling it ambiguous", () => {
    const paid = planBuilds(choice, ["carrier"]);
    expect(paid.ok && paid.draft).toEqual({ "build|carrier|1": 1 });
    const swapped = planBuilds(choice, ["exchange|carrier"]);
    expect(swapped.ok && swapped.steps).toEqual([{ kind: "produce", unit: "carrier", count: 1, exchange: true }]);
  });
  it("costs an exchange as nothing", () => {
    expect(buildCosts(choice, ["carrier", "exchange|carrier"])).toEqual([3, 0]);
  });
});

describe("Berekar Berekon's discount in the draft", () => {
  const agent: ChoiceOption = {
    id: "exhaust_agent|winnuagent",
    kind: "production_agent",
    label: "Exhaust Berekar Berekon: -2 cost",
    payload: { leader: "winnuagent", discount_offered: 2 },
  };
  it("takes 2 off the combined bill, never below zero", () => {
    expect(draftResourceCost([build], { [build.id]: 1 })).toBe(3);
    expect(draftResourceCost([build], { [build.id]: 1 }, 2)).toBe(1);
    expect(draftResourceCost([build], { [build.id]: 2 }, 2)).toBe(4);
    const cheap = { ...build, id: "build|fighter|1", payload: { ...build.payload, cost: 1, printed_cost: 1 } };
    expect(draftResourceCost([cheap], { [cheap.id]: 1 }, 2)).toBe(0);
  });
  it("adds to a discount the use already has and shows it as the cost reduction", () => {
    const sarween = { ...build, payload: { ...build.payload, cost: 2, discount: 1 } };
    expect(draftResourceCost([sarween], { [sarween.id]: 1 }, 2)).toBe(0);
    const total = productionTotal([build], { [build.id]: 1 }, 2);
    expect(total.total).toBe(1);
    expect(total.discount).toBe(2);
  });
  it("reads what the option offers and is not a build", () => {
    expect(agentDiscount({ ...choice, options: [build, agent] } as PendingChoiceDto)).toBe(2);
    expect(agentDiscount(choice as PendingChoiceDto)).toBe(0);
    const planned = planBuilds({ ...choice, options: [build, agent] } as PendingChoiceDto, ["carrier"]);
    expect(planned.ok).toBe(true);
  });
});
