import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { SIZES, diplomacyRiderPick, openScene, prefix, warfareRiderPick, umbatPick, legacy, type SizeName } from "./scene";

// The Diplomacy Rider's system question, and the other system questions that listed bare ids.
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`Diplomacy Rider system pick ${size}`, async ({ page }, testInfo) => {
    await openScene(page, size, diplomacyRiderPick());
    await expect(page.getByTestId("pending-choice-dialog")).toBeVisible();
    if (!legacy) {
      await expect(page.getByTestId("predict-outcome-text")).toContainText("choose 1 system that contains a planet you control");
      await expect(page.getByTestId("pending-choice-dialog")).toContainText("System 25 · Quann (2/1)");
      await expect(page.getByTestId("pending-choice-dialog")).toContainText("System 27 · New Albion");
    }
    await shot(page, testInfo, `${prefix}-1-diplomacy-rider-${size}`);
  });

  test(`Diplomacy Rider on the map ${size}`, async ({ page }, testInfo) => {
    await openScene(page, size, diplomacyRiderPick());
    const button = page.getByTestId("system-pick-inspect-map-btn");
    if (!(await button.count())) {
      await shot(page, testInfo, `${prefix}-2-rider-map-${size}`);
      return;
    }
    await button.click();
    await page.waitForTimeout(600);
    await shot(page, testInfo, `${prefix}-2-rider-map-${size}`);
  });

  test(`Warfare Rider system pick ${size}`, async ({ page }, testInfo) => {
    await openScene(page, size, warfareRiderPick());
    await shot(page, testInfo, `${prefix}-3-warfare-rider-${size}`);
  });

  test(`Umbat system pick ${size}`, async ({ page }, testInfo) => {
    await openScene(page, size, umbatPick());
    await shot(page, testInfo, `${prefix}-4-umbat-${size}`);
  });
}
