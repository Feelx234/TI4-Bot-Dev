import { expect, test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { SIZES, VARIANTS, prefix } from "./scene";

// The agenda being decided, as every seat and spectator sees it: from the reveal until the vote
// resolves. The data is only the public game state (agenda_in_play, agenda_casts, agenda_predictions,
// agenda_log) plus the catalog's printed text; no decision is needed to know the agenda.
for (const [size, viewport] of Object.entries(SIZES)) {
  for (const variant of VARIANTS) {
    test(`agenda banner ${variant.name} ${size}`, async ({ page }, testInfo) => {
      await page.setViewportSize(viewport);
      await openMockedGame(page, variant.options);
      await page.waitForTimeout(600);
      const strip = page.getByTestId("agenda-strip");
      if (variant.gone) {
        if (prefix === "after") await expect(strip).toHaveCount(0);
        await shot(page, testInfo, `${prefix}-${variant.name}-${size}`);
        return;
      }
      if (prefix === "before") {
        await shot(page, testInfo, `${prefix}-${variant.name}-${size}`);
        return;
      }
      await expect(strip).toBeVisible();
      // Phones show the one line first and open it on tap; wide windows start open.
      const toggle = page.getByTestId("agenda-strip-toggle");
      if (size !== "desktop") {
        await shot(page, testInfo, `${prefix}-${variant.name}-${size}-line`);
        await toggle.click();
      }
      const details = page.getByTestId("agenda-strip-details");
      await expect(details).toBeVisible();
      const text = (await strip.innerText()).replace(/\s+/g, " ");
      expect(text).not.toMatch(/player_[0-9a-f]{8}/);
      for (const part of variant.expect) expect(text, part).toContain(part);
      const box = (await strip.boundingBox())!;
      expect(box.x).toBeGreaterThanOrEqual(0);
      expect(box.x + box.width).toBeLessThanOrEqual(viewport.width + 1);
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
      await shot(page, testInfo, `${prefix}-${variant.name}-${size}`);
    });
  }
}
