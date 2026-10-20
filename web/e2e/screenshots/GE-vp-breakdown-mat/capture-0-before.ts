import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { A, B, D, openPlayers, openScene, prefix, SIZES, type SizeName } from "./scene";

// Run with TI4_SHOT_PREFIX=before on the base commit (80ce1bf1); skipped otherwise. The PNGs are
// committed under before/.
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`before ${size}`, async ({ page }, testInfo) => {
    test.skip(prefix !== "before", "before shots come from the base commit");
    await openScene(page, size, { passed: [D, B], thunders: false });
    await openPlayers(page, size);
    await shot(page, testInfo, `before-1-players-${size}`);
    const chip = page.getByTestId("player-vp").first();
    await chip.scrollIntoViewIfNeeded();
    await chip.hover();
    await page.waitForTimeout(500);
    await shot(page, testInfo, `before-2-vp-tooltip-${size}`);
    void A;
  });
}
