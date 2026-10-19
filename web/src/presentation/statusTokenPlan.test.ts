import { describe, expect, it } from "vitest";
import type { PendingChoiceDto } from "../protocol/types.ts";
import { partialContext } from "../test/wire.ts";
import {
  openParts,
  parseStoredStatusPlan,
  partUsed,
  redistributionProblem,
  resolveStatusTokens,
  serializeStoredStatusPlan,
  statusTokenPart,
  type StoredStatusPlan,
} from "./statusTokenPlan.ts";

const SEAT = "p1";
const pools = { tactic: 3, fleet: 3, strategic: 2 };

const gainChoice = (tokens: number, extra: Record<string, unknown> = {}, phase: "status" | "action" | "agenda" | "strategy" = "status"): PendingChoiceDto => ({
  prompt: "gain a command token into which pool",
  actor: SEAT,
  nonce: "n-gain",
  context: partialContext({ subtype: "gain_command_token", phase, actor: SEAT }),
  options: ["tactic", "fleet", "strategic"].map((pool) => ({
    id: `${pool}_tokens`,
    kind: "pool",
    label: `${pool} pool`,
    payload: {},
  })),
  details: { kind: "command_tokens", mode: "gain", pools, reinforcements: 8, tokens_to_place: tokens, ...extra },
});

/** The arrangements the engine offers for `total` tokens with a fleet floor. */
const arrangements = (total: number, floor: number) => {
  const options = [];
  const min = total >= floor ? floor : 0;
  for (let tactic = 0; tactic <= total; tactic++)
    for (let fleet = min; fleet <= total - tactic; fleet++) {
      const id = `${tactic}|${fleet}|${total - tactic - fleet}`;
      options.push({ id, kind: "redistribute", label: id, payload: {} });
    }
  return options;
};

const redistributeChoice = (held: typeof pools, floor = 2): PendingChoiceDto => {
  const total = held.tactic + held.fleet + held.strategic;
  return {
    prompt: "redistribute your command tokens",
    actor: SEAT,
    nonce: "n-redis",
    context: partialContext({ subtype: "status_redistribute_tokens", phase: "status", actor: SEAT }),
    options: arrangements(total, floor),
    details: { kind: "command_tokens", mode: "redistribute", pools: held, reinforcements: 6, total },
  };
};

describe("which decision is planned", () => {
  it("plans only the status phase's gain and redistribution", () => {
    expect(statusTokenPart(gainChoice(2))).toBe("gain");
    expect(statusTokenPart(redistributeChoice(pools))).toBe("redistribute");
    // Leadership, action cards and exploration use the same gain question outside the status phase.
    expect(statusTokenPart(gainChoice(3, {}, "strategy"))).toBeNull();
    expect(statusTokenPart(gainChoice(1, {}, "action"))).toBeNull();
    expect(statusTokenPart(null)).toBeNull();
  });
});

describe("stored status plans", () => {
  const stored: StoredStatusPlan = {
    v: 1,
    round: 3,
    plan: { gain: { tactic: 2, fleet: 0, strategic: 0 }, redistribute: { tactic: 4, fleet: 4, strategic: 2 } },
  };

  it("round-trips by meaning, without option ids", () => {
    const raw = serializeStoredStatusPlan(stored);
    expect(raw).not.toMatch(/_tokens|optionId/);
    expect(parseStoredStatusPlan(raw)).toEqual(stored);
  });

  it("drops garbage and malformed pools instead of throwing", () => {
    expect(parseStoredStatusPlan(null)).toBeNull();
    expect(parseStoredStatusPlan("{nope")).toBeNull();
    expect(parseStoredStatusPlan(JSON.stringify({ v: 2, round: 1, plan: {} }))).toBeNull();
    const bad = JSON.stringify({ v: 1, round: 1, plan: { gain: { tactic: -1, fleet: 0, strategic: 0 }, redistribute: { tactic: 99, fleet: 0, strategic: 0 } } });
    expect(parseStoredStatusPlan(bad)).toBeNull();
    const half = JSON.stringify({ v: 1, round: 1, plan: { gain: { tactic: 1, fleet: 1, strategic: 0 }, redistribute: { tactic: 1 } } });
    expect(parseStoredStatusPlan(half)?.plan).toEqual({ gain: { tactic: 1, fleet: 1, strategic: 0 } });
  });

  it("counts a part as used only while the game is past its answer (undo makes it usable again)", () => {
    const used = { ...stored, applied: { gain: 10 } };
    expect(partUsed(used, "gain", 11)).toBe(true);
    expect(partUsed(used, "gain", 10)).toBe(false);
    expect(partUsed(used, "gain", 9)).toBe(false);
    expect(openParts(used, 11)).toEqual({ redistribute: stored.plan.redistribute });
    expect(openParts(used, 9)).toEqual(stored.plan);
    expect(openParts(null, 0)).toEqual({});
  });
});

describe("resolving the gain", () => {
  const plan = { gain: { tactic: 1, fleet: 1, strategic: 0 } };

  it("answers exactly the planned pools as one batch", () => {
    const { resolution, part } = resolveStatusTokens(plan, gainChoice(2), SEAT);
    expect(part).toBe("gain");
    expect(resolution.kind).toBe("tokens");
    if (resolution.kind !== "tokens") return;
    expect(resolution.steps).toEqual([
      { kind: "pool", pool: "tactic_tokens" },
      { kind: "pool", pool: "fleet_tokens" },
    ]);
    expect(resolution.text).toBe("Gain 2 command tokens: 1 tactic, 1 fleet");
  });

  it("needs review when a different number of tokens is gained (never a half plan)", () => {
    for (const tokens of [1, 3]) {
      const { resolution } = resolveStatusTokens(plan, gainChoice(tokens), SEAT);
      expect(resolution.kind).toBe("review");
      if (resolution.kind === "review") expect(resolution.reason).toContain(`gain ${tokens}`);
    }
  });

  it("needs review when the reinforcements cannot hold the plan, or a purchase is offered", () => {
    expect(resolveStatusTokens(plan, gainChoice(2, { reinforcements: 1 }), SEAT).resolution.kind).toBe("review");
    const purchase = { cost: 3, influence_available: 3, max: 1, trade_goods: 0, trade_good_worth: 1, planets: [] };
    expect(resolveStatusTokens(plan, gainChoice(2, { purchase }), SEAT).resolution.kind).toBe("review");
  });

  it("has nothing to say about other seats, other gains or an empty plan", () => {
    expect(resolveStatusTokens(plan, gainChoice(2), "p2").resolution.kind).toBe("none");
    expect(resolveStatusTokens(plan, gainChoice(2, {}, "strategy"), SEAT).resolution.kind).toBe("none");
    expect(resolveStatusTokens({}, gainChoice(2), SEAT).resolution.kind).toBe("none");
    expect(resolveStatusTokens(null, gainChoice(2), SEAT).resolution.kind).toBe("none");
  });
});

describe("resolving the redistribution", () => {
  it("answers the planned arrangement with the engine's option id", () => {
    const { resolution } = resolveStatusTokens({ redistribute: { tactic: 5, fleet: 3, strategic: 2 } }, redistributeChoice({ tactic: 4, fleet: 4, strategic: 2 }), SEAT);
    expect(resolution).toMatchObject({ kind: "option", optionId: "5|3|2" });
  });

  it("needs review when the tokens held changed (a different gain than planned)", () => {
    const { resolution, prefill } = resolveStatusTokens(
      { redistribute: { tactic: 5, fleet: 3, strategic: 2 } },
      redistributeChoice({ tactic: 4, fleet: 4, strategic: 3 }),
      SEAT,
    );
    expect(resolution.kind).toBe("review");
    if (resolution.kind === "review") expect(resolution.reason).toContain("hold 11");
    expect(prefill).toBeUndefined();
  });

  it("needs review (with the plan as a suggestion) when the fleet floor forbids it", () => {
    const { resolution, prefill } = resolveStatusTokens(
      { redistribute: { tactic: 6, fleet: 1, strategic: 3 } },
      redistributeChoice({ tactic: 4, fleet: 4, strategic: 2 }, 2),
      SEAT,
    );
    expect(resolution.kind).toBe("review");
    if (resolution.kind === "review") expect(resolution.reason).toContain("at least 2");
    expect(prefill?.pools).toEqual({ tactic: 6, fleet: 1, strategic: 3 });
  });

  it("applies a Letnev-style plan below two fleet tokens when the game offers it", () => {
    const { resolution } = resolveStatusTokens(
      { redistribute: { tactic: 6, fleet: 1, strategic: 3 } },
      redistributeChoice({ tactic: 4, fleet: 4, strategic: 2 }, 0),
      SEAT,
    );
    expect(resolution).toMatchObject({ kind: "option", optionId: "6|1|3" });
  });
});

describe("planning a redistribution", () => {
  it("must arrange exactly the tokens expected after the gain and respect the fleet floor", () => {
    expect(redistributionProblem({ tactic: 5, fleet: 3, strategic: 2 }, 10, "hacan")).toBeNull();
    expect(redistributionProblem({ tactic: 5, fleet: 3, strategic: 1 }, 10, "hacan")).toMatch(/Assign 1 more/);
    expect(redistributionProblem({ tactic: 5, fleet: 3, strategic: 3 }, 10, "hacan")).toMatch(/Remove 1/);
    expect(redistributionProblem({ tactic: 6, fleet: 1, strategic: 3 }, 10, "hacan")).toMatch(/at least 2/);
    expect(redistributionProblem({ tactic: 6, fleet: 1, strategic: 3 }, 10, "letnev")).toBeNull();
  });
});
