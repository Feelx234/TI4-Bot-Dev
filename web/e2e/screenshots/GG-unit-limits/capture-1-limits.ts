import { expect, test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";

// The produce dialog states each capped unit's component limit (31.4): units on the board, units
// left in reinforcements, the limit; builds staged in the dialog count against what is left, the +
// stops at the limit, and a unit the engine no longer offers (none left) is listed with the reason.
// The fixtures carry the engine's shape (details.unit_limits as built by ProductionWindow). The
// "before" shots were taken with TI4_SHOT_PREFIX=before on commit 80ce1bf1, where nothing of this
// exists, so the assertions only run for "after".
const prefix = process.env.TI4_SHOT_PREFIX ?? "after";
const SIZES = {
  desktop: { width: 1440, height: 1000 },
  "390x844": { width: 390, height: 844 },
} as const;

const option = (unit: string, cost: number, count = 1) => ({
  id: `build|${unit}|${count}`,
  kind: "produce",
  label: `produce ${count}x ${unit} for ${cost}`,
  payload: {
    unit,
    count,
    placed: count,
    cost,
    printed_cost: cost,
    production_spent: 1,
    available_resources: 30,
  },
});

type Held = Record<string, number>;
const LIMITS: Record<string, { limit: number; unit: string }> = {
  flagship: { limit: 1, unit: "flagship" },
  warsun: { limit: 2, unit: "warsun" },
  dreadnought: { limit: 5, unit: "dreadnought" },
  cruiser: { limit: 8, unit: "cruiser" },
  destroyer: { limit: 8, unit: "destroyer" },
  carrier: { limit: 4, unit: "carrier" },
  mech: { limit: 4, unit: "mech" },
  pds: { limit: 6, unit: "pds" },
  spacedock: { limit: 3, unit: "spacedock" },
};

const production = (held: Held) => {
  const unit_limits: Record<string, unknown> = {};
  const options = [];
  const cost: Record<string, number> = {
    destroyer: 1, cruiser: 2, carrier: 3, dreadnought: 4, flagship: 8, warsun: 12, mech: 2,
  };
  for (const [base, { limit, unit }] of Object.entries(LIMITS)) {
    if (base === "pds" || base === "spacedock") continue;
    const h = held[base] ?? 0;
    unit_limits[base] = { units: [unit], limit, held: h, remaining: Math.max(0, limit - h) };
    // The engine drops an option once nothing is left in the box.
    if (h < limit) options.push(option(unit, cost[base]));
  }
  options.splice(0, 0, option("fighter", 1, 2));
  options.push(option("infantry", 1, 2));
  return {
    prompt: "produce in 18 (6 left)",
    context: {
      subtype: "produce_unit",
      target: { System: "18" },
      outstanding: [{ kind: "production_capacity", amount: 6, paid: 0 }],
    },
    details: { fleet_supply: { used: 1, limit: 8 }, capacity: { transport: 8, consumed: 0, fighters: 0, unused_support: 0 }, unit_limits },
    options: [...options, { id: "done_producing", kind: "decline", label: "produce nothing further" }],
  };
};

interface Scenario {
  name: string;
  held: Held;
  stage: [string, number][];
  /** Test ids that must read as given (after only). */
  texts: [string, string][];
  disabled?: string[];
  limitRows?: string[];
}

const scenarios: Scenario[] = [
  {
    name: "1-early-game",
    held: { carrier: 1, cruiser: 1, destroyer: 2, dreadnought: 1, flagship: 0, warsun: 0, mech: 0 },
    stage: [],
    texts: [
      ["produce-limit-build|carrier|1", "Carrier 1 on board · 3 left (limit 4)"],
      ["produce-limit-build|flagship|1", "Flagship 0 on board · 1 left (limit 1)"],
      ["produce-limit-build|warsun|1", "War sun 0 on board · 2 left (limit 2)"],
    ],
  },
  {
    name: "2-carriers-at-limit",
    held: { carrier: 4, cruiser: 3, dreadnought: 2, flagship: 1, warsun: 0, mech: 1 },
    stage: [],
    texts: [
      ["produce-limit-row-carrier", "Carrier 4 on board · none left (limit 4)"],
      ["produce-limit-row-carrier", "Limit reached: no carrier left in reinforcements"],
    ],
    limitRows: ["carrier", "flagship"],
  },
  {
    name: "3-flagship-and-war-sun-at-limit",
    held: { carrier: 2, cruiser: 5, dreadnought: 4, flagship: 1, warsun: 2, mech: 4 },
    stage: [],
    texts: [
      ["produce-limit-row-flagship", "Limit reached: no flagship left in reinforcements"],
      ["produce-limit-row-warsun", "War sun 2 on board · none left (limit 2)"],
      ["produce-limit-row-mech", "Limit reached: no mech left in reinforcements"],
    ],
    limitRows: ["flagship", "warsun", "mech"],
  },
  {
    name: "4-queued-units-reduce-remaining",
    held: { carrier: 3, cruiser: 6, dreadnought: 3, flagship: 0, warsun: 1, mech: 0 },
    stage: [["build|carrier|1", 1], ["build|dreadnought|1", 2], ["build|warsun|1", 1]],
    texts: [
      ["produce-limit-build|carrier|1", "Carrier 3 on board · 1 in this build · none left (limit 4)"],
      ["produce-limit-build|dreadnought|1", "Dreadnought 3 on board · 2 in this build · none left (limit 5)"],
      ["produce-limit-build|warsun|1", "War sun 1 on board · 1 in this build · none left (limit 2)"],
      ["produce-limit-build|cruiser|1", "Cruiser 6 on board · 2 left (limit 8)"],
    ],
    disabled: ["build|carrier|1", "build|dreadnought|1", "build|warsun|1"],
  },
];

for (const [sizeName, size] of Object.entries(SIZES)) {
  for (const s of scenarios) {
    test(`${s.name} ${sizeName}`, async ({ page }, testInfo) => {
      await page.setViewportSize(size);
      await openMockedGame(page, { choice: production(s.held) });
      await page.getByTestId("production-builder-drawer").waitFor();
      for (const [id, times] of s.stage) {
        for (let i = 0; i < times; i += 1) {
          const add = page.getByTestId(`produce-unit-btn-${id}`);
          if (await add.isEnabled()) await add.click();
        }
      }
      if (prefix === "after") {
        for (const [id, text] of s.texts) await expect(page.getByTestId(id)).toContainText(text);
        for (const id of s.disabled ?? []) {
          await expect(page.getByTestId(`produce-unit-btn-${id}`)).toBeDisabled();
          await expect(page.getByTestId(`produce-option-${id}`)).toContainText("Limit reached");
        }
        for (const base of s.limitRows ?? []) {
          await expect(page.getByTestId(`produce-limit-row-${base}`).getByRole("button")).toBeDisabled();
        }
        if (sizeName !== "desktop") {
          const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
          expect(overflow).toBe(false);
        }
      }
      await shot(page, testInfo, `${prefix}-${s.name}-${sizeName}`, { of: page.getByTestId("production-builder-drawer"), pad: 4 });
    });
  }
}
