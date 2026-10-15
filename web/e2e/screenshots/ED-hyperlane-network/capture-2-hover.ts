import { test, expect } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openPicker } from "./open";

// After only: pointing at a system lights every route it has and rings the systems at the far ends.
test("hover a linked system (Race for Mecatol)", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openPicker(page, 5, "5pRaceForMecatol");
  const linked = page.locator('[data-testid="map-preview-board"] [data-linked="true"]').first();
  await expect(linked).toBeVisible();
  const title = await linked.locator("title").textContent();
  expect(title).toMatch(/Connected through hyperlanes to system/);
  await linked.dispatchEvent("mouseover");
  await linked.dispatchEvent("mouseenter");
  await shot(page, testInfo, "7-hover-desktop", { of: page.getByTestId("map-preview-board") });
});

test("tap a linked system on a phone", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openPicker(page, 5, "5pRaceForMecatol");
  await page.locator('[data-testid="map-preview-board"] [data-linked="true"]').nth(3).dispatchEvent("click");
  await shot(page, testInfo, "7-tap-390x844");
});
