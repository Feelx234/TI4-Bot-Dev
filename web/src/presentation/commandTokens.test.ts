import { describe, expect, it } from "vitest";
import {
  addToken,
  arrangementId,
  canConfirmTokens,
  confirmBlocker,
  describeCommandTokens,
  initialStaging,
  poolPips,
  removeToken,
  resultingCount,
  tokenOutcome,
  tokenPlan,
  tokensRemaining,
} from "./commandTokens.ts";

const poolOptions = ["tactic_tokens", "fleet_tokens", "strategic_tokens"].map((id) => ({
  id,
  kind: "pool",
  label: id,
}));
const gain = (toPlace = 2) => ({
  options: poolOptions,
  details: {
    kind: "command_tokens",
    mode: "gain",
    pools: { tactic: 3, fleet: 4, strategic: 2 },
    reinforcements: 7,
    tokens_to_place: toPlace,
  },
});
const redistribute = (arrangements = ["3|4|2", "2|5|2", "4|4|1", "9|0|0"]) => ({
  options: arrangements.map((id) => ({ id, kind: "redistribute", label: id })),
  details: {
    kind: "command_tokens",
    mode: "redistribute",
    pools: { tactic: 3, fleet: 4, strategic: 2 },
    reinforcements: 7,
    total: 9,
  },
});

describe("describeCommandTokens", () => {
  it("reads a gain: pools, reinforcements and how many to place", () => {
    const view = describeCommandTokens(gain(), true)!;
    expect(view.mode).toBe("gain");
    expect(view.current).toEqual({ tactic: 3, fleet: 4, strategic: 2 });
    expect(view.reinforcements).toBe(7);
    expect(view.total).toBe(2);
  });

  it("offers a gain only with a batch submitter, so a plain list remains the fallback", () => {
    expect(describeCommandTokens(gain(), false)).toBeNull();
  });

  it("offers a redistribution without a batch submitter", () => {
    const view = describeCommandTokens(redistribute(), false)!;
    expect(view.mode).toBe("redistribute");
    expect(view.total).toBe(9);
    expect(view.arrangements.has("2|5|2")).toBe(true);
  });

  it("ignores other decisions and incomplete details", () => {
    expect(describeCommandTokens({ options: poolOptions }, true)).toBeNull();
    expect(
      describeCommandTokens({ options: poolOptions, details: { kind: "strategy_secondary" } }, true),
    ).toBeNull();
    const noPools = { ...gain(), details: { kind: "command_tokens", mode: "gain" } };
    expect(describeCommandTokens(noPools, true)).toBeNull();
    expect(describeCommandTokens({ ...gain(), options: poolOptions.slice(0, 2) }, true)).toBeNull();
  });
});

describe("gain staging", () => {
  const view = describeCommandTokens(gain(2), true)!;

  it("counts down the tokens left and stops adding at zero", () => {
    let staging = initialStaging(view);
    expect(tokensRemaining(view, staging)).toBe(2);
    staging = addToken(view, staging, "fleet");
    staging = addToken(view, staging, "fleet");
    expect(tokensRemaining(view, staging)).toBe(0);
    expect(addToken(view, staging, "tactic")).toEqual(staging);
    expect(resultingCount(view, staging, "fleet")).toBe(6);
  });

  it("confirms only when every token is assigned", () => {
    let staging = addToken(view, initialStaging(view), "tactic");
    expect(canConfirmTokens(view, staging)).toBe(false);
    expect(confirmBlocker(view, staging)).toContain("1 more token");
    expect(tokenOutcome(view, staging)).toBeNull();
    staging = addToken(view, staging, "strategic");
    expect(canConfirmTokens(view, staging)).toBe(true);
  });

  it("removes only what was staged, never below zero", () => {
    const staging = addToken(view, initialStaging(view), "tactic");
    expect(removeToken(staging, "fleet")).toEqual(staging);
    expect(removeToken(staging, "tactic")).toEqual(initialStaging(view));
  });

  it("plans one pool step per token in pool order", () => {
    let staging = initialStaging(view);
    staging = addToken(view, staging, "strategic");
    staging = addToken(view, staging, "tactic");
    expect(tokenPlan(staging)).toEqual([
      { kind: "pool", pool: "tactic_tokens" },
      { kind: "pool", pool: "strategic_tokens" },
    ]);
    expect(tokenOutcome(view, staging)).toEqual({ kind: "plan", steps: tokenPlan(staging) });
  });

  it("shows existing pips apart from new ones", () => {
    const staging = addToken(view, initialStaging(view), "tactic");
    expect(poolPips(view, staging, "tactic")).toEqual({ kept: 3, added: 1, removed: 0 });
    expect(poolPips(view, staging, "fleet")).toEqual({ kept: 4, added: 0, removed: 0 });
  });
});

describe("redistribution staging", () => {
  const view = describeCommandTokens(redistribute(), false)!;

  it("starts as the current arrangement with nothing left to assign", () => {
    const staging = initialStaging(view);
    expect(staging).toEqual({ tactic: 3, fleet: 4, strategic: 2 });
    expect(tokensRemaining(view, staging)).toBe(0);
    expect(canConfirmTokens(view, staging)).toBe(true);
  });

  it("frees a token by removing it and must reassign it before confirming", () => {
    let staging = removeToken(initialStaging(view), "strategic");
    expect(tokensRemaining(view, staging)).toBe(1);
    expect(canConfirmTokens(view, staging)).toBe(false);
    staging = addToken(view, staging, "fleet");
    // 3|5|1 is not among the offered arrangements.
    expect(arrangementId(view, staging)).toBeNull();
    expect(canConfirmTokens(view, staging)).toBe(false);
    expect(tokenOutcome(view, { tactic: 2, fleet: 5, strategic: 2 })).toEqual({
      kind: "option",
      optionId: "2|5|2",
    });
  });

  it("refuses an arrangement the engine does not offer, with a reason", () => {
    const bad = { tactic: 8, fleet: 1, strategic: 0 };
    expect(arrangementId(view, bad)).toBeNull();
    expect(confirmBlocker(view, bad)).toContain("fleet pool");
    expect(tokenOutcome(view, bad)).toBeNull();
  });

  it("shows tokens moved out as removed pips and tokens moved in as new ones", () => {
    const staging = { tactic: 2, fleet: 5, strategic: 2 };
    expect(poolPips(view, staging, "tactic")).toEqual({ kept: 2, added: 0, removed: 1 });
    expect(poolPips(view, staging, "fleet")).toEqual({ kept: 4, added: 1, removed: 0 });
    expect(resultingCount(view, staging, "fleet")).toBe(5);
  });
});
