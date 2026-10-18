import { expect, test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { SIZES, earlyGame, midGame } from "./inPlayStates";

test.beforeEach(async ({ page }) => {
  await page.setViewportSize(SIZES.desktop);
});

test("desktop header button and the open panel, mid-game", async ({ page }, testInfo) => {
  await openMockedGame(page, midGame);
  // The button sits in the header actions, next to Versions, which stays where it was.
  const actions = page.locator(".game-header__actions");
  await expect(actions.getByTestId("version-info-button")).toBeVisible();
  await expect(actions.getByTestId("in-play-button")).toBeVisible();
  await shot(page, testInfo, "1-header-button-desktop", { of: actions, pad: 8 });
  await actions.getByTestId("in-play-button").click();
  const panel = page.getByTestId("in-play-side");
  await expect(panel).toBeVisible();
  await expect(page.getByTestId("in-play-count-laws")).toHaveText("3");
  await expect(page.getByTestId("in-play-count-discard-action")).toHaveText("9");
  await expect(page.getByTestId("in-play-section-table")).toContainText("Removed by");
  const box = (await panel.boundingBox())!;
  expect(box.width).toBeGreaterThan(900);
  await shot(page, testInfo, "2-panel-mid-game-desktop");
  // It stays open beside the game: the board is still there and the header button still works.
  await expect(page.getByTestId("in-play-button")).toHaveAttribute("aria-pressed", "true");
});

test("desktop panel, filtered to the discard piles and searched", async ({ page }, testInfo) => {
  await openMockedGame(page, midGame);
  await page.getByTestId("in-play-button").click();
  await page.getByTestId("in-play-filter-discard").click();
  await expect(page.getByTestId("in-play-section-laws")).toHaveCount(0);
  await expect(page.getByTestId("in-play-section-discard-agenda")).toBeVisible();
  await shot(page, testInfo, "3-discards-desktop");
  await page.getByTestId("in-play-filter-all").click();
  await page.getByTestId("in-play-search").fill("direct hit");
  await expect(page.getByTestId("in-play-section-laws")).toHaveCount(0);
  await expect(page.getByTestId("in-play-section-discard-action")).toContainText("x3");
  await shot(page, testInfo, "4-search-desktop");
});

test("desktop panel keyboard: expand a clamped card, Escape closes", async ({ page }, testInfo) => {
  await openMockedGame(page, midGame);
  await page.getByTestId("in-play-button").click();
  const more = page.getByTestId("in-play-section-laws").getByRole("button", { name: "More" }).first();
  await more.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByTestId("in-play-section-laws").getByRole("button", { name: "Less" }).first()).toHaveAttribute("aria-expanded", "true");
  await shot(page, testInfo, "5-expanded-law-desktop");
  await page.getByTestId("in-play-search").focus();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("in-play-side")).toHaveCount(0);
});

test("desktop panel, early game nearly empty", async ({ page }, testInfo) => {
  await openMockedGame(page, earlyGame);
  await page.getByTestId("in-play-button").click();
  await expect(page.getByTestId("in-play-count-laws")).toHaveText("0");
  await expect(page.getByTestId("in-play-section-laws")).toContainText("No law has been passed yet.");
  await expect(page.getByTestId("in-play-section-table")).toContainText("On Mecatol Rex");
  await shot(page, testInfo, "6-early-game-desktop");
});
