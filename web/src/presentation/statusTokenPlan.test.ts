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
  it("plans only the status phase's redistribution: the engine asks no status gain", () => {
    expect(statusTokenPart(redistributeChoice(pools))).toBe("redistribute");
    // Leadership, action cards and exploration ask gains; none is planned here.
    expect(statusTokenPart(gainChoice(2))).toBeNull();
    expect(statusTokenPart(gainChoice(3, {}, "strategy"))).toBeNull();
    expect(statusTokenPart(null)).toBeNull();
  });
});

describe("stored status plans", () => {
  const stored: StoredStatusPlan = {
    v: 1,
    round: 3,
    plan: { redistribute: { tactic: 4, fleet: 4, strategic: 2 } },
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
    const bad = JSON.stringify({ v: 1, round: 1, plan: { redistribute: { tactic: 99, fleet: 0, strategic: 0 } } });
    expect(parseStoredStatusPlan(bad)).toBeNull();
  });

  it("keeps only the redistribution of an older plan that also named a gain", () => {
    const old = JSON.stringify({
      v: 1,
      round: 2,
      plan: { gain: { tactic: 1, fleet: 1, strategic: 0 }, redistribute: { tactic: 4, fleet: 4, strategic: 2 } },
      applied: { gain: 7 },
    });
    expect(parseStoredStatusPlan(old)).toEqual({ v: 1, round: 2, plan: { redistribute: { tactic: 4, fleet: 4, strategic: 2 } }, applied: {} });
    const gainOnly = JSON.stringify({ v: 1, round: 2, plan: { gain: { tactic: 2, fleet: 0, strategic: 0 } } });
    expect(parseStoredStatusPlan(gainOnly)).toBeNull();
  });

  it("counts a part as used only while the game is past its answer (undo makes it usable again)", () => {
    const used = { ...stored, applied: { redistribute: 10 } };
    expect(partUsed(used, "redistribute", 11)).toBe(true);
    expect(partUsed(used, "redistribute", 10)).toBe(false);
    expect(partUsed(used, "redistribute", 9)).toBe(false);
    expect(openParts(used, 11)).toEqual({});
    expect(openParts(used, 9)).toEqual(stored.plan);
    expect(openParts(null, 0)).toEqual({});
  });
});

describe("a gain question", () => {
  it("is never answered from the plan", () => {
    const plan = { redistribute: { tactic: 5, fleet: 3, strategic: 2 } };
    expect(resolveStatusTokens(plan, gainChoice(2), SEAT).resolution.kind).toBe("none");
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
