import { test, expect } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openPicker } from "./open";

// Every linked pair gets its own arc and its own entry in the pair list.
const CASES: [number, string][] = [
  [5, "5pRaceForMecatol"],
  [3, "3pHyperlanes"],
  [3, "3pInPersonHyperlanes"],
];

for (const [players, alias] of CASES) {
  test(`links ${alias} desktop`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await openPicker(page, players, alias);
    const n = await page.getByTestId("hyperlane-link").count();
    expect(n).toBeGreaterThan(5);
    await expect(page.getByTestId("hyperlane-pair")).toHaveCount(n);
    await shot(page, testInfo, `10-links-${alias}-desktop`);
  });
}

test("links phone", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openPicker(page, 5, "5pRaceForMecatol");
  await shot(page, testInfo, "10-links-5pRaceForMecatol-390x844");
});

test("pick one pair from the list", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openPicker(page, 5, "5pRaceForMecatol");
  await page.getByTestId("hyperlane-link-legend").locator("summary").click();
  await page.getByTestId("hyperlane-pair").nth(7).hover();
  await expect(page.locator('[data-testid="hyperlane-link"][data-hot]')).toHaveCount(1);
  await shot(page, testInfo, "11-pair-picked-desktop");
});

test("pair picked, phone", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openPicker(page, 5, "5pRaceForMecatol");
  await page.getByTestId("hyperlane-link-legend").locator("summary").click();
  await page.getByTestId("hyperlane-pair").nth(7).click();
  await shot(page, testInfo, "11-pair-picked-390x844");
});
