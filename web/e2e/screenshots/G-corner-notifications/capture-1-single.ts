import { test } from "@playwright/test";
import { shot } from "../_shared/shot";

// One decision with a single legal choice was made for the player; the corner box says so.
test("notifications: one auto-selected choice", async ({ page }, testInfo) => {
  await page.goto("/dev/toasts?scenario=single");
  await page.locator(".auto-resolve-toast").first().waitFor();
  await shot(page, testInfo, "1-single", { of: page.locator(".auto-resolve-toast-container"), pad: 24 });
});
