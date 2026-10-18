import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { ACTION_MENU, noHorizontalScroll, openHeldItems, openScene, rich, richBoard, richState, rival, SIZES, type SizeName } from "./scene";

// A mid-game seat: promissory notes (hand, faceup, given, Support, Alliance), relics and fragments,
// exhaustable technologies, unit upgrades, leaders, a legendary ability and attachments.
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`rich seat ${size}`, async ({ page }, testInfo) => {
    await openScene(page, size, { players: [rich, rival], state: richState, board: richBoard, choice: ACTION_MENU });
    // The entry next to the card counts, with a badge.
    if (size === "desktop") {
      await shot(page, testInfo, "0-entry-desktop", { of: page.getByTestId("player-sheet-drawer") });
    } else {
      await page.getByTestId("player-sheet-toggle").click();
      await page.waitForTimeout(400);
      await page.getByTestId("held-items-button-0").scrollIntoViewIfNeeded();
      await shot(page, testInfo, "0-entry-390x844");
      await page.getByTestId("player-sheet-close").click();
    }
    await openHeldItems(page, size);
    const panel = page.getByTestId("held-items-panel");
    await expect(panel).toHaveAttribute("data-own", "true");
    await expect(page.getByTestId("held-item-relic-heartofixth")).toContainText("exhausted");
    await expect(page.getByTestId("held-item-tech-aida")).toHaveAttribute("data-usable", "true");
    await expect(page.getByTestId("held-item-tech-pi")).toContainText("exhausted");
    await expect(page.getByTestId("held-item-legendary-primor")).toContainText("ability exhausted");
    await expect(page.getByTestId("held-item-fragments-cultural")).toContainText("3 of 3");
    await expect(page.getByTestId("held-section-alliance")).toBeVisible();
    await expect(page.getByTestId("held-section-promissory")).toContainText("Support for the Throne");
    expect(await noHorizontalScroll(page)).toBeLessThanOrEqual(0);
    if (size === "390x844") {
      const box = (await panel.boundingBox())!;
      expect(box.width).toBeLessThanOrEqual(390);
      expect(box.height).toBeGreaterThanOrEqual(840);
    }
    await shot(page, testInfo, `1-rich-${size}`);
    // Keyboard: Escape closes, focus returns to the button.
    if (size === "desktop") {
      await page.getByTestId("held-items-panel").locator(".held-items__scroll").evaluate((el) => el.scrollTo(0, 700));
      await shot(page, testInfo, "2-rich-scrolled-desktop");
    } else {
      await page.getByTestId("held-items-panel").locator(".held-items__scroll").evaluate((el) => el.scrollTo(0, 1400));
      await shot(page, testInfo, "2-rich-scrolled-390x844");
    }
    await page.keyboard.press("Escape");
    await expect(panel).toHaveCount(0);
  });
}
