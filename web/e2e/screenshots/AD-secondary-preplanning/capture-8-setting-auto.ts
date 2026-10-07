import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openWaiting } from "./prep";

// The same setting switched to Auto-play.
test("secondary prep: setting, Auto", async ({ page }, testInfo) => {
  await openWaiting(page, { name: "Technology", card: "pok7technology" });
  await page.getByTestId("secondary-prep-mode-auto").click();
  await shot(page, testInfo, "8-setting-auto", { of: page.getByTestId("secondary-prep-mode").locator("xpath=../.."), pad: 6 });
});
