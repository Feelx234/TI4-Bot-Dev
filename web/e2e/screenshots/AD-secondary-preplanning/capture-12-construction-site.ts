import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openWaiting } from "./prep";

// Construction: what to place and where.
test("secondary prep: Construction site", async ({ page }, testInfo) => {
  await openWaiting(page, { name: "Construction", card: "pok4construction" });
  await page.getByTestId("secondary-prep-chip").click();
  await page.getByTestId("prep-follow").click();
  await page.getByTestId("prep-site-jord").click();
  await page.getByTestId("prep-unit-spacedock").click();
  await page.getByTestId("secondary-prepared-badge").waitFor();
  await shot(page, testInfo, "12-construction-site");
});
