import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openPlayers, openScene, SIZES, type SizeName } from "./scene";

// Breakthroughs belong to the Thunder's Edge content: hidden on a table without it, shown as before
// on a table with it.
for (const size of Object.keys(SIZES) as SizeName[]) {
  for (const thunders of [false, true]) {
    test(`breakthrough ${thunders ? "on" : "off"} ${size}`, async ({ page }, testInfo) => {
      await openScene(page, size, { thunders, passed: [] });
      await openPlayers(page, size);
      if (thunders) await expect(page.getByTestId("player-breakthrough").first()).toBeVisible();
      else await expect(page.getByTestId("player-breakthrough")).toHaveCount(0);
      await page.getByTestId("player-card").first().scrollIntoViewIfNeeded();
      await shot(page, testInfo, `${thunders ? "after-2-on" : "after-1-off"}-${size}`);
      // The Held items panel has no Breakthrough section either.
      await page.getByTestId("held-items-button-0").scrollIntoViewIfNeeded();
      await page.getByTestId("held-items-button-0").click();
      await page.getByTestId("held-items-panel").waitFor();
      await page.waitForTimeout(250);
      if (thunders) await expect(page.getByTestId("held-item-breakthrough")).toBeVisible();
      else await expect(page.getByTestId("held-item-breakthrough")).toHaveCount(0);
    });
  }
}
