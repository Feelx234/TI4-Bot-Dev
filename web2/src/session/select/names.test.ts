import { expect, it } from "vitest";
import { unitType } from "./names";

it("reads the type of a unit from its id, also of a faction and of an upgrade", () => {
  expect(unitType("carrier")).toBe("carrier");
  expect(unitType("sol_carrier")).toBe("carrier");
  expect(unitType("cruiser2")).toBe("cruiser");
  expect(unitType("sol_carrier2")).toBe("carrier");
  expect(unitType("fighter2")).toBe("fighter");
  expect(unitType("spacedock2")).toBe("dock");
  expect(unitType("warsun")).toBe("warsun");
  expect(unitType("token")).toBeNull();
});
