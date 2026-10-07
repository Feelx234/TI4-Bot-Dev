import { describe, expect, it } from "vitest";
import { createGameBody, missingExpected, parseExpect, presetFromEnv } from "../../e2e/smokePreset";

describe("smoke start preset", () => {
  it("treats an unset or empty TI4_SMOKE_PRESET as a normal opening", () => {
    expect(presetFromEnv(undefined)).toBeUndefined();
    expect(presetFromEnv("")).toBeUndefined();
    expect(presetFromEnv("  ")).toBeUndefined();
  });

  it("accepts the combat preset and rejects an unknown one", () => {
    expect(presetFromEnv("combat")).toBe("combat");
    expect(() => presetFromEnv("nope")).toThrow(/unknown TI4_SMOKE_PRESET/);
  });

  it("sends start_preset only when one was asked for", () => {
    expect(createGameBody(3, 7)).toEqual({ player_count: 3, seed: 7, nickname: "E2E Host" });
    expect(createGameBody(3, 7, "combat")).toEqual({
      player_count: 3,
      seed: 7,
      nickname: "E2E Host",
      start_preset: "combat",
    });
  });

  it("parses expectations: plain, any-of and at-least-N", () => {
    expect(parseExpect(undefined)).toEqual([]);
    expect(parseExpect("a, b|c, d|e|f>=2")).toEqual([
      { subtypes: ["a"], atLeast: 1 },
      { subtypes: ["b", "c"], atLeast: 1 },
      { subtypes: ["d", "e", "f"], atLeast: 2 },
    ]);
  });

  it("reports only the expectations the run missed", () => {
    const seen = { a: 3, c: 1, d: 1 };
    const wanted = parseExpect("a,b|c,d|e|f>=2,x");
    expect(missingExpected(seen, wanted)).toEqual(["d|e|f (need 2, saw 1)", "x (need 1, saw 0)"]);
  });
});
