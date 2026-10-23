import { expect, test, type Page } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { opponent, playerWithHand } from "../_shared/players";
import { shot } from "../_shared/shot";

// The technology viewer's "Owned" filter, last chip in the row. TI4_SHOT_PREFIX=before takes the
// unfiltered row on the commit before the change (there is no Owned chip then).
const PREFIX = process.env.TI4_SHOT_PREFIX ?? "after";
const SIZES = [["desktop", { width: 1440, height: 900 }], ["phone", { width: 390, height: 844 }]] as const;

async function open(page: Page, viewport: { width: number; height: number }) {
  await page.setViewportSize(viewport);
  await openMockedGame(page, { players: [playerWithHand({ technologies: ["amd", "st", "nm"] }), opponent] });
  const menu = page.getByTestId("top-menu-button");
  if (await menu.count()) await menu.click();
  await page.getByTestId("technology-modal-button").click();
  await page.getByTestId("technology-modal").waitFor();
  await page.waitForTimeout(500);
}

for (const [size, viewport] of SIZES) {
  test(`chips ${size}`, async ({ page }, testInfo) => {
    await open(page, viewport);
    await shot(page, testInfo, `${PREFIX}-chips-${size}`);
  });
  if (PREFIX !== "before") {
    test(`owned ${size}`, async ({ page }, testInfo) => {
      await open(page, viewport);
      await page.getByTestId("tech-filter-owned").click();
      await expect(page.getByTestId("tech-card-amd")).toBeVisible();
      await expect(page.getByTestId("tech-card-ws")).toHaveCount(0);
      await page.getByTestId("tech-filter-owned").scrollIntoViewIfNeeded();
      await shot(page, testInfo, `after-owned-${size}`);
    });
    test(`owned and colour ${size}`, async ({ page }, testInfo) => {
      await open(page, viewport);
      await page.getByTestId("tech-filter-biotic").click();
      await page.getByTestId("tech-filter-owned").click();
      await expect(page.getByTestId("tech-card-nm")).toBeVisible();
      await expect(page.getByTestId("tech-card-amd")).toHaveCount(0);
      await page.getByTestId("tech-filter-owned").scrollIntoViewIfNeeded();
      await shot(page, testInfo, `after-owned-green-${size}`);
    });
  }
}
