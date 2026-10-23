import { test } from "@playwright/test";
import { click, openFreeDesk, shotDesk } from "../DB-free-form-trade/_free";

test("offline after: free-form desk on a phone", async ({ page }, testInfo) => {
  await openFreeDesk(page, 390, 844);
  for (let i = 0; i < 2; i++) await click(page, "stage-give-tg-inc");
  await click(page, "stage-receive-cm-inc");
  await page.getByTestId("stage-status-custom").waitFor();
  await shotDesk(page, testInfo, "3-phone");
});
