import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openWaiting } from "./prep";

// Trade: follow or skip only, with the warning that a replenish removes the window.
test("secondary prep: Trade warning", async ({ page }, testInfo) => {
  await openWaiting(page, { name: "Trade", card: "pok5trade" });
  await page.getByTestId("secondary-prep-chip").click();
  await page.getByTestId("prep-follow").click();
  await page.getByTestId("prep-trade-warning").waitFor();
  await shot(page, testInfo, "6-trade-warning");
});
