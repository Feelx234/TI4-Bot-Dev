import { describe, expect, it } from "vitest";
import { createGameBody, presetFromEnv } from "../../e2e/smokePreset";

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
});
