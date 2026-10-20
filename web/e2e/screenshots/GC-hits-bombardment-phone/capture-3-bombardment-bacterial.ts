import { test } from "@playwright/test";
import { openBombardment, take, viewports } from "./scene";

// Bombardment result with X-89 Bacterial Weapon: 2 of 3 dice hit, each counts twice.
for (const v of viewports.filter((x) => x.id === "desktop" || x.id === "phone")) {
  test(`bombardment-bacterial-${v.id}`, async ({ page }, testInfo) => {
    await openBombardment(page, v);
    await page.getByTestId("invasion-step").scrollIntoViewIfNeeded();
    await take(page, testInfo, "bombardment-bacterial", v);
  });
}
