import { describe, expect, it } from "vitest";
import { isColourObjective, techColourCounts, techColourDisplay } from "./techColourProgress.ts";

describe("tech colour progress", () => {
  it("counts owned technologies per colour and ignores unit upgrades", () => {
    expect(techColourCounts(["amd", "det", "nm", "st", "cr2", "dn2"])).toEqual({
      PROPULSION: 2,
      BIOTIC: 1,
      CYBERNETIC: 1,
      WARFARE: 0,
    });
  });

  it("counts a faction technology under its colour", () => {
    // Aerie Hololattice (Argent) is a yellow faction technology; Advanced Carrier II is colourless.
    expect(techColourCounts(["ah", "ac2"])).toEqual({ PROPULSION: 0, BIOTIC: 0, CYBERNETIC: 1, WARFARE: 0 });
  });

  it("does not count a technology twice", () => {
    expect(techColourCounts(["amd", "amd"]).PROPULSION).toBe(1);
  });

  it("describes Diversify Research with a chip per colour and the engine's count", () => {
    const model = techColourDisplay("diversify", ["amd", "det", "nm"], {
      have: 1,
      threshold: 2,
      satisfied: false,
    })!;
    expect(model.chips.map((c) => [c.colour, c.count, c.needed, c.met])).toEqual([
      ["Blue", 2, 2, true],
      ["Green", 1, 2, false],
      ["Yellow", 0, 2, false],
      ["Red", 0, 2, false],
    ]);
    expect(model.summary).toBe("1 of 2 colors with 2+ · need 2");
    expect(model.aria).toContain("Green (biotic) 1 of 2");
  });

  it("falls back to its own count without progress, and knows Master the Sciences", () => {
    const model = techColourDisplay("master_science", ["amd", "det", "nm", "pa"], undefined)!;
    expect(model.have).toBe(2);
    expect(model.threshold).toBe(4);
    expect(model.summary).toBe("2 of 4 colors with 2+ · need 4");
  });

  it("is null for other objectives and unknown technologies", () => {
    expect(techColourDisplay("corner", ["amd"], undefined)).toBeNull();
    expect(techColourDisplay("diversify", undefined, undefined)).toBeNull();
    expect(isColourObjective("diversify")).toBe(true);
    expect(isColourObjective("corner")).toBe(false);
  });
});
