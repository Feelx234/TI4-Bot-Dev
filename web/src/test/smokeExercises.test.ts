import { describe, expect, it } from "vitest";
import { shotCapFromEnv } from "../../e2e/smokeExercises";
import { SecondaryPrepExercise, prepConfigFromEnv } from "../../e2e/smokePrep";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

describe("smoke exercise knobs", () => {
  it("caps screenshots at 20 by default and accepts a number or 'all'", () => {
    expect(shotCapFromEnv(undefined)).toBe(20);
    expect(shotCapFromEnv("all")).toBe(20);
    expect(shotCapFromEnv("4")).toBe(4);
    expect(shotCapFromEnv("0")).toBe(0);
    expect(shotCapFromEnv("nonsense")).toBe(20);
  });

  it("plans half of the secondaries, 90% of them in Auto, unless switched off", () => {
    expect(prepConfigFromEnv({})).toEqual({ enabled: true, probability: 0.5, autoProbability: 0.9 });
    expect(prepConfigFromEnv({ TI4_SMOKE_PREP: "0" }).enabled).toBe(false);
    expect(prepConfigFromEnv({ TI4_SMOKE_PREP_PROBABILITY: "1", TI4_SMOKE_PREP_AUTO_PROBABILITY: "0" })).toEqual({
      enabled: true,
      probability: 1,
      autoProbability: 0,
    });
    expect(prepConfigFromEnv({ TI4_SMOKE_PREP_PROBABILITY: "7" }).probability).toBe(1);
  });
});

describe("smoke prep exercise: decisions after the plan's own window", () => {
  const make = () => {
    const findings: string[] = [];
    const exercise = new SecondaryPrepExercise({
      pages: [],
      rng: () => 0.5,
      config: { enabled: true, probability: 1, autoProbability: 1 },
      log: () => {},
      finding: (text: string) => findings.push(text),
      shot: async () => {},
      collect: async () => [],
      pickOne: (cands: unknown[]) => cands[0],
      label: "test",
    } as never);
    // An Auto plan whose own window was answered (Politics has no follow-up step).
    (exercise as unknown as { plans: Map<number, unknown> }).plans.set(1, {
      seat: 1, round: 2, card: "Politics", mode: "auto", summary: "Follow Politics",
      started: true, answered: true, line: 0,
    });
    (exercise as unknown as { report: { cases: string[] } }).report.cases[0] = "seat 2 r2 Politics auto: plan \"Follow Politics\" -> auto-played (toast seen)";
    return { exercise, findings };
  };

  it("does not treat a reaction window as an Auto follow-up (s2-05 2026-10-20)", async () => {
    const { exercise, findings } = make();
    const result = await exercise.handle(1, { prompt: "after TRADE_GOODS_GAINED" }, 2, 10, async () => 10, "action", "reaction_after_TRADE_GOODS_GAINED");
    expect(result).toBe("normal");
    expect(findings).toEqual([]);
  });

  it("does not treat a status-phase decision as an Auto follow-up (s2-10, s1-01 2026-10-20)", async () => {
    const { exercise, findings } = make();
    const result = await exercise.handle(1, { prompt: "gain a command token into which pool" }, 2, 10, async () => 10, "status", "gain_command_token");
    expect(result).toBe("normal");
    expect(findings).toEqual([]);
  });
});

describe("secondary prep toast placement", () => {
  const css = readFileSync(resolve(__dirname, "../components/SecondaryPrep.css"), "utf8");
  it("docks the answer bars and toasts above the turn action bar (s1-08 2026-10-20)", () => {
    const block = (selector: string) => css.slice(css.indexOf(selector), css.indexOf("}", css.indexOf(selector)));
    expect(block(".secondary-prep:not(.secondary-prep--preparing) {")).toContain("var(--bottom-bar-offset");
    expect(css).toMatch(/bottom: calc\(var\(--bottom-bar-offset, 0px\) \+ 72px\)/);
  });
});
