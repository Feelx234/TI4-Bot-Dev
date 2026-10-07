import { describe, expect, it } from "vitest";
import {
  describePlan,
  hasTradeWarning,
  normalizePlan,
  ownPlanets,
  parseStoredPlan,
  researchableEstimate,
  resolveStep,
  serializeStoredPlan,
  TRADE_WARNING,
  type SecondaryPlan,
  type StoredPlan,
} from "./secondaryPlan.ts";
import { boardWith, option, player, secondaryChoice, stepChoice } from "../test/secondaryPrepFixtures.ts";

const stored = (plan: SecondaryPlan): StoredPlan => ({ v: 1, actionKey: "2:action_5:a:x", generation: 3, plan });

describe("stored plans", () => {
  it("round-trips semantically, without option ids", () => {
    const plan: SecondaryPlan = { card: "pok7technology", follow: true, tech: "amd" };
    const raw = serializeStoredPlan(stored(plan));
    expect(raw).not.toMatch(/optionId|"yes"/);
    expect(parseStoredPlan(raw)).toEqual(stored(plan));
  });

  it("drops garbage instead of throwing", () => {
    expect(parseStoredPlan(null)).toBeNull();
    expect(parseStoredPlan("{nope")).toBeNull();
    expect(parseStoredPlan(JSON.stringify({ v: 2 }))).toBeNull();
    expect(parseStoredPlan(JSON.stringify({ v: 1, actionKey: "k", generation: 0, plan: { card: "x" } }))).toBeNull();
  });

  it("keeps only well-formed details and at most two planets", () => {
    const raw = JSON.stringify({
      v: 1,
      actionKey: "k",
      generation: 0,
      plan: {
        card: "pok2diplomacy",
        follow: true,
        planets: ["a", "b", "c", 4],
        structure: { unit: "castle", planet: "x" },
        leadership: { pools: { tactic: 99, fleet: 0, strategic: 0 } },
      },
    });
    expect(parseStoredPlan(raw)?.plan).toEqual({ card: "pok2diplomacy", follow: true, planets: ["a", "b"] });
  });

  it("normalizes away details that do not belong to the card or to a skip", () => {
    expect(normalizePlan({ card: "pok3politics", follow: true, tech: "amd" })).toEqual({
      card: "pok3politics",
      follow: true,
    });
    expect(normalizePlan({ card: "pok7technology", follow: false, tech: "amd" })).toEqual({
      card: "pok7technology",
      follow: false,
    });
  });
});

describe("resolveStep: follow or skip", () => {
  it("answers follow with the plain yes and skip with no, for every card", () => {
    for (const card of ["pok3politics", "pok5trade", "pok8imperial", "pok6warfare", "pok7technology"]) {
      const follow = resolveStep({ card, follow: true }, secondaryChoice(card), "b");
      expect(follow).toMatchObject({ kind: "option", optionId: "yes" });
      const skip = resolveStep({ card, follow: false }, secondaryChoice(card), "b");
      expect(skip).toMatchObject({ kind: "option", optionId: "no" });
    }
  });

  it("uses the decline/follow pair of cards without a card-specific prompt", () => {
    const choice = secondaryChoice("x", {}, [option("decline", "decline"), option("follow")]);
    expect(resolveStep({ card: "x", follow: true }, choice, "b")).toMatchObject({ optionId: "follow" });
    expect(resolveStep({ card: "x", follow: false }, choice, "b")).toMatchObject({ optionId: "decline" });
  });

  it("faction waiver: a tokenless follower's single waiver replaces the plain yes", () => {
    const choice = secondaryChoice("pok7technology", {}, [
      option("no"),
      option("follow|waived|0|free_research", "strategy", "follow without a token"),
    ]);
    const result = resolveStep({ card: "pok7technology", follow: true }, choice, "b");
    expect(result).toMatchObject({ kind: "option", optionId: "follow|waived|0|free_research" });
  });

  it("prefers the paid yes when a waiver is offered beside it, and asks when waivers are ambiguous", () => {
    const both = secondaryChoice("pok7technology", {}, [option("no"), option("yes"), option("follow|waived|0|w")]);
    expect(resolveStep({ card: "pok7technology", follow: true }, both, "b")).toMatchObject({ optionId: "yes" });
    const many = secondaryChoice("pok7technology", {}, [
      option("no"),
      option("follow|waived|0|w"),
      option("follow|waived|1|v"),
    ]);
    expect(resolveStep({ card: "pok7technology", follow: true }, many, "b").kind).toBe("review");
  });

  it("needs review when following is gone, the card differs, or it is not the viewer's decision", () => {
    expect(
      resolveStep({ card: "pok3politics", follow: true }, secondaryChoice("pok3politics", {}, [option("no")]), "b").kind,
    ).toBe("review");
    expect(resolveStep({ card: "pok3politics", follow: true }, secondaryChoice("pok5trade"), "b").kind).toBe("review");
    expect(resolveStep({ card: "pok3politics", follow: true }, secondaryChoice("pok3politics"), "c")).toEqual({
      kind: "none",
    });
    expect(resolveStep(null, secondaryChoice("pok3politics"), "b")).toEqual({ kind: "none" });
    expect(resolveStep({ card: "pok3politics", follow: true }, null, "b")).toEqual({ kind: "none" });
  });
});

describe("resolveStep: second-level choices", () => {
  const tech: SecondaryPlan = { card: "pok7technology", follow: true, tech: "amd" };

  it("technology: matches the research option, flags a missing one, ignores other prompts", () => {
    const offered = stepChoice("research_technology", [option("amd", "research"), option("decline", "decline")]);
    expect(resolveStep(tech, offered, "b")).toMatchObject({ kind: "option", optionId: "amd" });
    const gone = stepChoice("research_technology", [option("nm", "research"), option("decline", "decline")]);
    expect(resolveStep(tech, gone, "b").kind).toBe("review");
    expect(resolveStep(tech, stepChoice("specialist_compounds", [option("yes"), option("no")]), "b")).toEqual({
      kind: "none",
    });
    expect(resolveStep({ ...tech, follow: false }, offered, "b")).toEqual({ kind: "none" });
  });

  it("diplomacy: readies prepared planets in turn and flags when none is offered", () => {
    const plan: SecondaryPlan = { card: "pok2diplomacy", follow: true, planets: ["jord", "lodor"] };
    const first = stepChoice("ready_planet", [option("jord", "ready"), option("lodor", "ready")]);
    expect(resolveStep(plan, first, "b")).toMatchObject({ optionId: "jord" });
    const second = stepChoice("ready_planet", [option("lodor", "ready"), option("xxcha", "ready")]);
    expect(resolveStep(plan, second, "b")).toMatchObject({ optionId: "lodor" });
    expect(resolveStep(plan, stepChoice("ready_planet", [option("xxcha", "ready")]), "b").kind).toBe("review");
  });

  it("construction: matches unit and planet whatever the system part is", () => {
    const plan: SecondaryPlan = { card: "pok4construction", follow: true, structure: { unit: "pds", planet: "jord" } };
    const offered = stepChoice("place_structure", [
      option("pds|18|jord", "build"),
      option("spacedock|18|jord", "build"),
      option("decline", "decline"),
    ]);
    expect(resolveStep(plan, offered, "b")).toMatchObject({ optionId: "pds|18|jord" });
    const noPds = stepChoice("place_structure", [option("spacedock|18|jord", "build")]);
    expect(resolveStep(plan, noPds, "b").kind).toBe("review");
  });
});

describe("resolveStep: Leadership purchase plan", () => {
  const leadership = (max: number, planets = [{ id: "jord", worth: 2 }, { id: "lodor", worth: 1 }]) =>
    secondaryChoice("pok1leadership", {
      details: {
        kind: "strategy_secondary",
        card: "pok1leadership",
        played_by: "a",
        tokens_left: 0,
        costs_token: false,
        mode: "buy",
        pools: { tactic: 3, fleet: 3, strategic: 2 },
        reinforcements: 10,
        purchase: {
          cost: 3,
          influence_available: 3,
          max,
          trade_goods: 0,
          trade_good_worth: 1,
          planets,
        },
      },
    });

  it("plans the purchase, the payment and the pool as one token batch", () => {
    const plan: SecondaryPlan = { card: "pok1leadership", follow: true, leadership: { pools: { tactic: 0, fleet: 1, strategic: 0 } } };
    const result = resolveStep(plan, leadership(1), "b");
    expect(result.kind).toBe("tokens");
    if (result.kind !== "tokens") return;
    expect(result.steps[0]).toEqual({ kind: "purchase", buy: true });
    expect(result.steps).toContainEqual({ kind: "pool", pool: "fleet_tokens" });
    expect(result.steps.filter((s) => s.kind === "exhaust")).toHaveLength(2);
  });

  it("needs review when the influence no longer pays for the prepared tokens", () => {
    const plan: SecondaryPlan = { card: "pok1leadership", follow: true, leadership: { pools: { tactic: 2, fleet: 0, strategic: 0 } } };
    const result = resolveStep(plan, leadership(1), "b");
    expect(result.kind).toBe("review");
    expect(resolveStep(plan, leadership(1, []), "b").kind).toBe("review");
  });

  it("skipping Leadership answers no", () => {
    expect(resolveStep({ card: "pok1leadership", follow: false }, leadership(1), "b")).toMatchObject({
      optionId: "no",
    });
  });
});

describe("panel data and wording", () => {
  it("lists only exhausted own planets for Diplomacy and every own planet for Construction", () => {
    const board = boardWith([
      { system: "18", planet: "jord", owner: "b", exhausted: true },
      { system: "18", planet: "other", owner: "c", exhausted: true },
      { system: "26", planet: "lodor", owner: "b" },
    ]);
    expect(ownPlanets(board, "b").map((p) => p.planet).sort()).toEqual(["jord", "lodor"]);
    expect(ownPlanets(board, "b").filter((p) => p.exhausted).map((p) => p.planet)).toEqual(["jord"]);
    expect(ownPlanets(board, null)).toEqual([]);
  });

  it("estimates researchable technologies from owned ones", () => {
    const fresh = researchableEstimate(player("b"));
    expect(fresh).toContain("amd");
    expect(fresh).not.toContain("lwd");
    const owned = researchableEstimate(player("b", { technologies: ["amd"] }));
    expect(owned).not.toContain("amd");
  });

  it("describes a plan and flags Trade", () => {
    expect(describePlan({ card: "pok7technology", follow: true, tech: "amd" })).toMatch(/research/i);
    expect(describePlan({ card: "pok3politics", follow: false })).toBe("Skip Politics");
    expect(hasTradeWarning("pok5trade")).toBe(true);
    expect(hasTradeWarning("pok3politics")).toBe(false);
    expect(TRADE_WARNING).toMatch(/replenishes you/);
  });
});
