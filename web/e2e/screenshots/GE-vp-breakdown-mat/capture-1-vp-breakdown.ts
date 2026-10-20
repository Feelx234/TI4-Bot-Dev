import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { A, B, C, D, noHorizontalScroll, openPlayers, openScene, prefix, SIZES, type SizeName } from "./scene";

// The reworked VP breakdown: tap a seat's VP chip, get a sheet with a tab per seat, the points by
// source, the way to the target and the public objectives still open.
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`vp breakdown ${size}`, async ({ page }, testInfo) => {
    test.skip(prefix === "before", "the dialog is new");
    await openScene(page, size, { passed: [D, B] });
    await openPlayers(page, size);
    await page.getByTestId("player-vp").first().scrollIntoViewIfNeeded();
    await page.getByTestId("player-vp").first().click();
    const dialog = page.getByTestId("score-dialog");
    await expect(dialog).toBeVisible();
    await expect(page.getByTestId("score-total")).toContainText("7 / 10 VP");
    await expect(page.getByTestId("score-sum")).toHaveAttribute("data-matches", "true");
    expect(await noHorizontalScroll(page)).toBeLessThanOrEqual(0);
    // 44 px targets.
    for (const t of await page.locator(".score-tab, .score-dialog__close").all()) {
      expect((await t.boundingBox())!.height).toBeGreaterThanOrEqual(43.5);
    }
    if (size === "390x844") {
      const box = (await dialog.boundingBox())!;
      expect(box.width).toBeLessThanOrEqual(390);
    }
    await shot(page, testInfo, `after-1-own-${size}`);

    // One tap to the leader.
    await page.getByTestId(`score-tab-${B}`).click();
    await expect(page.getByTestId("score-leader")).toBeVisible();
    await expect(page.getByTestId("score-total")).toContainText("8 / 10 VP");
    await expect(page.getByTestId("score-passed")).toBeVisible();
    await shot(page, testInfo, `after-2-leader-${size}`);
    await page.locator(".score-dialog__scroll").evaluate((el) => el.scrollTo(0, 1000));
    await shot(page, testInfo, `after-3-leader-open-objectives-${size}`);

    await page.getByTestId(`score-tab-${C}`).click();
    await expect(page.getByTestId("score-sum")).toHaveAttribute("data-matches", "true");
    await shot(page, testInfo, `after-4-trailing-${size}`);
    void A;
  });
}
