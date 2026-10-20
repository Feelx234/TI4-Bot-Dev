import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { SIZES } from "../DO-offline-game/offlineGame";
import { openTechnologyPrimary } from "./realPa";

// The Technologies button of the header (no decision open), "Can research now" on. The dialog gets
// the board now, so Ada's exhausted specialty planets count: with Psychoarchaeology Scanlink
// Drone Network (yellow, from Wellon) and Non-Euclidean Shielding (R R) are listed, without they are not.
for (const [size, viewport] of Object.entries(SIZES)) {
  for (const preset of ["research", "research_nopa"] as const) {
    test(`header technologies ${preset} ${size}`, async ({ page }, testInfo) => {
      test.setTimeout(300_000);
      await page.setViewportSize(viewport);
      await openTechnologyPrimary(page, preset, true);
      const menu = page.getByTestId("top-menu-button");
      if (await menu.count()) await menu.click();
      await page.getByTestId("technology-modal-button").click();
      await page.getByTestId("technology-modal").waitFor();
      await page.getByTestId("tech-filter-available").click();
      await page.waitForTimeout(500);
      const shown = await page
        .locator('[data-testid^="tech-card-"]')
        .evaluateAll((els) => els.map((e) => e.getAttribute("data-testid")!.replace("tech-card-", "")));
      console.log(`HEADER ${preset} ${size} ${JSON.stringify(shown)}`);
      if (preset === "research") {
        expect(shown).toContain("gls");
        expect(shown).toContain("nes");
      } else {
        expect(shown).not.toContain("gls");
        expect(shown).not.toContain("nes");
      }
      await shot(page, testInfo, `real-header-${preset}-${size}`);
    });
  }
}
