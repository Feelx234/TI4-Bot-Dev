import { test } from "@playwright/test";
import { shot } from "../_shared/shot";

// The whole viewport, to show where the box sits on the screen.
test("notifications: position in the window", async ({ page }, testInfo) => {
  await page.goto("/dev/toasts?scenario=stack");
  await page.locator(".auto-resolve-toast").nth(2).waitFor();
  await shot(page, testInfo, "3-in-place");
});
