import { describe, expect, it } from "vitest";
import {
  limitOfUnit,
  limitReachedText,
  queuedOfLimit,
  readUnitLimits,
  stagedOfLimit,
  unitLimitText,
  unitLimitView,
} from "./productionUnitLimits.ts";

const raw = {
  carrier: { units: ["carrier", "sol_carrier2"], limit: 4, held: 2, remaining: 2 },
  flagship: { units: ["letnev_flagship"], limit: 1, held: 1, remaining: 0 },
  bad: { limit: "x" },
};
const opt = (unit: string, placed = 1) => ({
  id: `build|${unit}|1`,
  kind: "produce",
  label: unit,
  payload: { unit, placed },
});

describe("unit limits", () => {
  it("reads the engine entries and skips malformed ones", () => {
    const limits = readUnitLimits(raw);
    expect(limits.map((l) => l.base)).toEqual(["carrier", "flagship"]);
    expect(limitOfUnit(limits, "sol_carrier2")?.base).toBe("carrier");
    expect(limitOfUnit(limits, "fighter")).toBeUndefined();
    expect(readUnitLimits(undefined)).toEqual([]);
  });

  it("subtracts staged and queued builds from what is left", () => {
    const [carrier] = readUnitLimits(raw);
    const options = [opt("carrier"), opt("sol_carrier2")];
    const staged = stagedOfLimit(carrier, options, { [options[0].id]: 1, [options[1].id]: 1 });
    expect(staged).toBe(2);
    const view = unitLimitView(carrier, staged);
    expect(view).toMatchObject({ onBoard: 2, left: 0, atLimit: true });
    expect(unitLimitView(carrier, 1, 0).left).toBe(1);
    expect(unitLimitView(carrier, 1, 3).left).toBe(0);
    expect(queuedOfLimit(carrier, ["carrier", "cruiser", "carrier"])).toBe(2);
  });

  it("words the row", () => {
    const [carrier] = readUnitLimits(raw);
    expect(unitLimitText("Carrier", unitLimitView(carrier, 0))).toBe("Carrier 2 on board · 2 left (limit 4)");
    expect(unitLimitText("Carrier", unitLimitView(carrier, 1))).toBe(
      "Carrier 2 on board · 1 in this build · 1 left (limit 4)",
    );
    expect(unitLimitText("Flagship", unitLimitView(readUnitLimits(raw)[1], 0))).toContain("none left (limit 1)");
    expect(limitReachedText("warsun")).toBe("Limit reached: no war sun left in reinforcements");
  });
});
