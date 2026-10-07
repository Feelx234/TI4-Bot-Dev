import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openWaiting } from "./prep";

// Follow plus a technology picked: the Prepared badge and the clear control.
test("secondary prep: prepared badge", async ({ page }, testInfo) => {
  await openWaiting(page, { name: "Technology", card: "pok7technology" });
  await page.getByTestId("secondary-prep-chip").click();
  await page.getByTestId("prep-follow").click();
  await page.getByTestId("prep-tech-amd").click();
  await page.getByTestId("secondary-prepared-badge").waitFor();
  await shot(page, testInfo, "3-prepared-badge");
});
