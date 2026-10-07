import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openWaiting } from "./prep";

test.use({ viewport: { width: 390, height: 844 } });

// Phone width: the panel spans the screen under the status bar.
test("secondary prep: phone width", async ({ page }, testInfo) => {
  await openWaiting(page, { name: "Technology", card: "pok7technology" });
  await page.getByTestId("secondary-prep-chip").click();
  await page.getByTestId("prep-follow").click();
  await page.getByTestId("prep-tech-amd").click();
  await page.getByTestId("secondary-prepared-badge").waitFor();
  await shot(page, testInfo, "10-phone");
});
