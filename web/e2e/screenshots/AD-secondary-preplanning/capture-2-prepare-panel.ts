import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openWaiting } from "./prep";

// The panel opened, nothing chosen yet: follow or skip, private and not binding.
test("secondary prep: the prepare panel", async ({ page }, testInfo) => {
  await openWaiting(page, { name: "Technology", card: "pok7technology" });
  await page.getByTestId("secondary-prep-chip").click();
  await page.getByTestId("secondary-prep-panel").waitFor();
  await shot(page, testInfo, "2-prepare-panel");
});
