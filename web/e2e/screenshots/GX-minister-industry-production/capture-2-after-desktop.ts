import { test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { afterGame } from "./fixtures";

test("after: production dialog, desktop", async ({ page }, testInfo) => {
  await openMockedGame(page, afterGame());
  await page.getByTestId("production-builder-drawer").waitFor();
  await shot(page, testInfo, "2-after-desktop");
});

test("after: staged builds", async ({ page }, testInfo) => {
  await openMockedGame(page, afterGame());
  await page.getByTestId("production-builder-drawer").waitFor();
  await page.getByRole("button", { name: "Add 1x dreadnought for 4" }).click();
  await page.getByRole("button", { name: "Add 1x carrier for 3" }).click();
  await shot(page, testInfo, "2b-after-staged");
});
