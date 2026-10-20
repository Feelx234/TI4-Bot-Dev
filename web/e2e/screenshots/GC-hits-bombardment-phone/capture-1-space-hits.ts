import { expect, test } from "@playwright/test";
import { layoutFacts, openSpace, spaceCasualtyChoice, spaceSustainChoice, take, viewports } from "./scene";

// After the fix: no horizontal scroll, every button inside the viewport and 44px on phone sizes,
// and the Confirm button reachable without scrolling the list (sticky bar).
async function assertPhoneLayout(page: import("@playwright/test").Page, v: { id: string; width: number }) {
  if (process.env.TI4_SHOT_PREFIX === "before" || v.id === "desktop") return;
  const facts = await layoutFacts(page);
  expect(facts.scrollW, "horizontal scroll").toBeLessThanOrEqual(facts.vw);
  expect(facts.clipped, "buttons outside the viewport").toEqual([]);
  expect(facts.small, "targets under 44px").toEqual([]);
  await expect(page.getByTestId("hit-confirm")).toBeInViewport({ ratio: 1 });
}

// Space combat: five hits owed against a mixed fleet (8 fighters, 2 destroyers, 2 dreadnoughts, 2 carriers).
for (const v of viewports) {
  test(`space-sustain-hits-${v.id}`, async ({ page }, testInfo) => {
    await openSpace(page, v, spaceSustainChoice);
    await page.getByTestId("hit-assignment-panel").scrollIntoViewIfNeeded();
    await assertPhoneLayout(page, v);
    await take(page, testInfo, "space-sustain-hits", v);
  });
  test(`space-casualty-hits-${v.id}`, async ({ page }, testInfo) => {
    await openSpace(page, v, spaceCasualtyChoice);
    await page.getByTestId("hit-assignment-panel").scrollIntoViewIfNeeded();
    await assertPhoneLayout(page, v);
    await take(page, testInfo, "space-casualty-hits", v);
  });
}
