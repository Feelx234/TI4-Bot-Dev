import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openWaiting } from "./prep";

// The viewer-local setting in the player sheet header: Review is the default.
test("secondary prep: setting, Review (default)", async ({ page }, testInfo) => {
  await openWaiting(page, { name: "Technology", card: "pok7technology" });
  await page.getByTestId("secondary-prep-mode").waitFor();
  await shot(page, testInfo, "7-setting-review", { of: page.getByTestId("secondary-prep-mode").locator("xpath=../.."), pad: 6 });
});
