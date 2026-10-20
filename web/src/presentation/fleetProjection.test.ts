import { describe, expect, it } from "vitest";
import {
  fighterMovesAlone,
  projectFleetSupply,
  readFleetInputs,
  type FleetInputs,
} from "./fleetProjection.ts";

const inputs = (over: Partial<FleetInputs> = {}): FleetInputs => ({
  used: 0,
  limit: 3,
  ships: 0,
  capacity: 0,
  carried: 0,
  fighters: 0,
  fighterSupport: 0,
  upgraded: false,
  half: false,
  ...over,
});
const none = { ships: 0, capacity: 0, fighters: 0, fighterTypes: [] as string[] };

describe("fighterMovesAlone", () => {
  it("is true for the upgraded and special fighters the engine offers a move, false for the base one", () => {
    expect(fighterMovesAlone("fighter2")).toBe(true);
    expect(fighterMovesAlone("naalu_fighter2")).toBe(true);
    expect(fighterMovesAlone("fighter")).toBe(false);
    expect(fighterMovesAlone("carrier")).toBe(false);
  });
});

describe("readFleetInputs", () => {
  it("reads the engine detail and refuses one without the sum's inputs", () => {
    expect(
      readFleetInputs({
        used: 2, limit: 4, ships: 1, capacity: 0, carried: 0, fighters: 1,
        fighter_support: 0, fighters_upgraded: true, fighters_half: false,
      }),
    ).toEqual(inputs({ used: 2, limit: 4, ships: 1, fighters: 1, upgraded: true }));
    expect(readFleetInputs({ used: 2, limit: 4 })).toBeNull();
    expect(readFleetInputs(undefined)).toBeNull();
  });
});

describe("projectFleetSupply", () => {
  it("charges Fighter II moving alone: no capacity holds it, so each one is a ship", () => {
    const p = projectFleetSupply(inputs({ ships: 1 }), { ...none, fighters: 2, fighterTypes: ["fighter2"] });
    expect(p).toMatchObject({ used: 3, uncarriedFighters: 2, fighterCharge: 2 });
  });

  it("does not charge fighters a moving carrier holds", () => {
    const p = projectFleetSupply(inputs(), { ships: 1, capacity: 4, fighters: 3, fighterTypes: ["fighter2"] });
    expect(p).toMatchObject({ used: 1, uncarriedFighters: 0 });
  });

  it("charges only the fighters beyond the capacity", () => {
    const p = projectFleetSupply(inputs(), { ships: 1, capacity: 2, fighters: 5, fighterTypes: ["fighter2"] });
    expect(p.used).toBe(4);
    expect(p.uncarriedFighters).toBe(3);
  });

  it("counts the fighters already in the system and the capacity already there", () => {
    const now = inputs({ ships: 2, capacity: 2, fighters: 2, upgraded: true, used: 2 });
    expect(projectFleetSupply(now, none).used).toBe(2);
    expect(projectFleetSupply(now, { ...none, fighters: 1, fighterTypes: ["fighter2"] }).used).toBe(3);
  });

  it("charges nothing for base fighters, which cannot move alone", () => {
    const p = projectFleetSupply(inputs(), { ...none, fighters: 2, fighterTypes: ["fighter"] });
    expect(p.used).toBe(0);
  });

  it("lets a structure's fighter support excuse fighters first", () => {
    const now = inputs({ fighterSupport: 2, upgraded: true });
    expect(projectFleetSupply(now, { ...none, fighters: 3, fighterTypes: ["fighter2"] }).used).toBe(1);
  });

  it("counts ground forces in space against the same capacity", () => {
    const now = inputs({ capacity: 2, carried: 2, ships: 1 });
    const p = projectFleetSupply(now, { ...none, fighters: 1, fighterTypes: ["fighter2"] });
    expect(p).toMatchObject({ used: 2, uncarriedFighters: 1 });
  });

  it("halves the charge for Naalu's Hybrid Crystal Fighter II, rounding up", () => {
    const p = projectFleetSupply(inputs(), { ...none, fighters: 3, fighterTypes: ["naalu_fighter2"] });
    expect(p).toMatchObject({ used: 2, uncarriedFighters: 3, fighterCharge: 2 });
  });

  it("flags over the limit exactly when the sum exceeds it", () => {
    const p = projectFleetSupply(inputs({ ships: 2, limit: 3 }), { ...none, fighters: 2, fighterTypes: ["fighter2"] });
    expect(p.used).toBe(4);
    expect(p.used > 3).toBe(true);
  });
});
