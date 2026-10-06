import { test } from "@playwright/test";
import { shot } from "../_shared/shot";

// Several automatic decisions in a row stack upwards from the bottom-left corner.
test("notifications: stacked auto-selected choices", async ({ page }, testInfo) => {
  await page.goto("/dev/toasts?scenario=stack");
  await page.locator(".auto-resolve-toast").nth(2).waitFor();
  await shot(page, testInfo, "2-stack", { of: page.locator(".auto-resolve-toast-container"), pad: 24 });
});
