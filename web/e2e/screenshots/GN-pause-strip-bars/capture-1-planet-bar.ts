import { expect, test, type Page } from "@playwright/test";
import { galleryDecision, galleryPlayers } from "../_shared/fixtures";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";

// Bio-Stims asks its own question (planet pick), so the pause strip floats at the bottom edge.
// "before" re-applies the old layout (no lift) by cancelling the margin; "after" is the real app.
const SIZES = { "390x844": { width: 390, height: 844 }, "360x740": { width: 360, height: 740 }, desktop: { width: 1440, height: 900 } };

async function coveredChips(page: Page) {
  return page.evaluate(() => {
    const out: string[] = [];
    for (const el of document.querySelectorAll<HTMLElement>('[data-testid^="planet-chip-"]')) {
      const r = el.getBoundingClientRect();
      const hit = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
      if (!hit || !el.contains(hit)) out.push(el.dataset.testid ?? "?");
    }
    return out;
  });
}

for (const [size, viewport] of Object.entries(SIZES)) {
  for (const step of ["before", "after"] as const) {
    test(`planet bar under pause strip ${step} ${size}`, async ({ page }, testInfo) => {
      await page.setViewportSize(viewport);
      await openMockedGame(page, { choice: galleryDecision("Bio-Stims"), players: galleryPlayers });
      await expect(page.getByTestId("prompt-switch-strip")).toBeVisible();
      await expect(page.locator('[data-testid^="planet-chip-"]').first()).toBeVisible();
      if (step === "before") await page.addStyleTag({ content: ".choice-banner,.choice-minimized-pill{margin-bottom:0!important}" });
      await page.waitForTimeout(300);
      const covered = await coveredChips(page);
      if (step === "after") expect(covered, "chips under the strip").toEqual([]);
      await shot(page, testInfo, `${step}-${size}`);
    });
  }
}
