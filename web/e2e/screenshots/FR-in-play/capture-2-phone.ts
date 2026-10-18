import { expect, test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { SIZES, earlyGame, expectNoHorizontalScroll, midGame } from "./inPlayStates";

test.beforeEach(async ({ page }) => {
  await page.setViewportSize(SIZES["390x844"]);
});

test("phone menu entry and the full-height sheet, mid-game", async ({ page }, testInfo) => {
  await openMockedGame(page, midGame);
  await page.getByTestId("top-menu-button").click();
  const entry = page.getByTestId("in-play-menu-button");
  await expect(entry).toBeVisible();
  expect((await entry.boundingBox())!.height).toBeGreaterThanOrEqual(44);
  await shot(page, testInfo, "1-menu-entry-390x844");
  await entry.click();
  const sheet = page.getByTestId("in-play-sheet");
  await expect(sheet).toBeVisible();
  const box = (await sheet.boundingBox())!;
  expect(box.height).toBeGreaterThanOrEqual(830);
  await expect(page.getByTestId("in-play-count-laws")).toHaveText("3");
  await expectNoHorizontalScroll(page, 390);
  await shot(page, testInfo, "2-sheet-top-390x844");
  // Scroll to the discard piles: the section headers stick to the top of the sheet.
  const scroll = page.getByTestId("in-play-scroll");
  await page.getByTestId("in-play-section-discard-action").scrollIntoViewIfNeeded();
  await scroll.evaluate((el) => el.scrollBy(0, 40));
  const head = page.getByTestId("in-play-section-discard-action").locator(".in-play__section-head");
  const scrollTop = (await scroll.boundingBox())!.y;
  expect(Math.abs((await head.boundingBox())!.y - scrollTop)).toBeLessThanOrEqual(2);
  await expectNoHorizontalScroll(page, 390);
  await shot(page, testInfo, "3-sheet-discards-390x844");
});

test("phone sheet, filtered to the discard piles", async ({ page }, testInfo) => {
  await openMockedGame(page, midGame);
  await page.getByTestId("top-menu-button").click();
  await page.getByTestId("in-play-menu-button").click();
  await page.getByTestId("in-play-filter-discard").click();
  await expect(page.getByTestId("in-play-section-laws")).toHaveCount(0);
  await expectNoHorizontalScroll(page, 390);
  await shot(page, testInfo, "4-sheet-filter-discard-390x844");
  await page.getByTestId("in-play-close").click();
  await expect(page.getByTestId("in-play-sheet")).toHaveCount(0);
});

test("phone sheet, early game nearly empty", async ({ page }, testInfo) => {
  await openMockedGame(page, earlyGame);
  await page.getByTestId("top-menu-button").click();
  await page.getByTestId("in-play-menu-button").click();
  await expect(page.getByTestId("in-play-section-laws")).toContainText("No law has been passed yet.");
  await expectNoHorizontalScroll(page, 390);
  await shot(page, testInfo, "5-sheet-early-game-390x844");
});
