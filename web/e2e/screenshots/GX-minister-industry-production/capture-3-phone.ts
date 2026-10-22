import { test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { afterGame, beforeGame } from "./fixtures";

test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

test("before: single-unit list, phone", async ({ page }, testInfo) => {
  await openMockedGame(page, beforeGame());
  await page.waitForTimeout(800);
  await shot(page, testInfo, "3-before-phone");
});

test("after: production dialog, phone", async ({ page }, testInfo) => {
  await openMockedGame(page, afterGame());
  await page.getByTestId("production-builder-drawer").waitFor();
  await shot(page, testInfo, "3b-after-phone");
});
