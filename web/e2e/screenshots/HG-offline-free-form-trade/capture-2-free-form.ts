import { test } from "@playwright/test";
import { click, openFreeDesk, shotDesk } from "../DB-free-form-trade/_free";

// After: the offline host announces details.free_form again and answers the composed deal itself, so the desk proposes any amounts.
test("offline after: a composed deal is proposable", async ({ page }, testInfo) => {
  await openFreeDesk(page);
  for (let i = 0; i < 2; i++) await click(page, "stage-give-tg-inc");
  await click(page, "stage-receive-cm-inc");
  await page.getByTestId("stage-status-custom").waitFor();
  await shotDesk(page, testInfo, "2-free-form");
});
