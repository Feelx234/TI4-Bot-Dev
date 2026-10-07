import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openWaiting } from "./prep";

// Diplomacy: the viewer's exhausted planets, up to two.
test("secondary prep: Diplomacy planets", async ({ page }, testInfo) => {
  await openWaiting(page, { name: "Diplomacy", card: "pok2diplomacy" });
  await page.getByTestId("secondary-prep-chip").click();
  await page.getByTestId("prep-follow").click();
  await page.getByTestId("prep-planet-exhausted").click();
  await page.getByTestId("secondary-prepared-badge").waitFor();
  await shot(page, testInfo, "11-diplomacy-planets");
});
