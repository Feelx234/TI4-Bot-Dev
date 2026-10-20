import { test } from "@playwright/test";
import { actor, openBarrageRecap, opponent, take, viewports, type BarrageScene } from "./scene";

// The anti-fighter barrage note in the combat overlay after the barrage, by case.
const cases: Record<string, BarrageScene> = {
  // Feedback fb_1792391288_b1b8a1e9: a destroyer's barrage hit, the defender had only a carrier.
  "no-fighters": { defender: ["carrier"], attacker: ["destroyer"], hits: { [opponent.id]: 1 } },
  "with-fighters": {
    defender: ["carrier", "fighter", "fighter", "fighter"],
    attacker: ["destroyer", "destroyer"],
    hits: { [opponent.id]: 2 },
    defenderAfter: ["carrier", "fighter"],
  },
  "raid-formation": {
    defender: ["carrier", "dreadnought"],
    attacker: ["destroyer", "destroyer"],
    hits: { [opponent.id]: 2 },
    attackerFaction: "argent",
  },
  missed: { defender: ["carrier", "fighter"], attacker: ["destroyer"], hits: { [opponent.id]: 0 } },
};

for (const v of viewports) {
  for (const [id, scene] of Object.entries(cases)) {
    test(`barrage-${id}-${v.id}`, async ({ page }, testInfo) => {
      await openBarrageRecap(page, v, scene);
      void actor;
      await take(page, testInfo, `barrage-${id}`, v);
    });
  }
}
