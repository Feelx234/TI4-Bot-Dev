import { describe, expect, it } from "vitest";
import { hydrateTech } from "./technologyData.ts";
import { matchesTechFilter, techColor, isTechFilterActive, NO_TECH_FILTER } from "./techFilter.ts";

describe("technology filter", () => {
  it("names the colour of standard, unit upgrade and faction technologies", () => {
    expect(techColor(hydrateTech("amd"))).toBe("PROPULSION");
    expect(techColor(hydrateTech("nm"))).toBe("BIOTIC");
    expect(techColor(hydrateTech("cr2"))).toBe("UNIT_UPGRADE");
    // A faction technology reads its colour from its own card.
    expect(techColor(hydrateTech("so"))).toBe("CYBERNETIC");
  });

  it("combines the colour with availability", () => {
    const nm = hydrateTech("nm");
    expect(matchesTechFilter(nm, NO_TECH_FILTER, false)).toBe(true);
    expect(matchesTechFilter(nm, { color: "BIOTIC", availableOnly: false, ownedOnly: false }, false)).toBe(true);
    expect(matchesTechFilter(nm, { color: "WARFARE", availableOnly: false, ownedOnly: false }, true)).toBe(false);
    expect(matchesTechFilter(nm, { color: "BIOTIC", availableOnly: true, ownedOnly: false }, false)).toBe(false);
    expect(matchesTechFilter(nm, { color: null, availableOnly: true, ownedOnly: false }, true)).toBe(true);
  });

  it("owned shows only technologies the viewer has, and combines with the colour", () => {
    const nm = hydrateTech("nm");
    const owned = { color: null, availableOnly: false, ownedOnly: true } as const;
    expect(matchesTechFilter(nm, owned, false, true)).toBe(true);
    expect(matchesTechFilter(nm, owned, false, false)).toBe(false);
    expect(matchesTechFilter(nm, { ...owned, color: "WARFARE" }, false, true)).toBe(false);
    expect(isTechFilterActive(owned)).toBe(true);
  });
});
