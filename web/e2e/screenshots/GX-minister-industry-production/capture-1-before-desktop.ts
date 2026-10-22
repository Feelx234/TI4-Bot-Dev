import { test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { beforeGame } from "./fixtures";

// What the law asked before: one generic "produce one unit" list, repeated per unit.
test("before: single-unit list, desktop", async ({ page }, testInfo) => {
  await openMockedGame(page, beforeGame());
  await page.waitForTimeout(800);
  await shot(page, testInfo, "1-before-desktop");
});
