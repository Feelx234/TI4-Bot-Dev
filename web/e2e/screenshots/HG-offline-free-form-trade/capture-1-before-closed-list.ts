import { test } from "@playwright/test";
import { click, openDesk, shotDesk } from "../AG-trade-staging-desk/_trade";

// Before: the offline host hid details.free_form from the desk (commit 551c598a), so the desk offered only the engine's listed deals
// and a combination off the list (2 trade goods for 1 commodity) could not be proposed. Same fixture as an offline choice without the flag.
test("offline before: the closed list", async ({ page }, testInfo) => {
  await openDesk(page);
  for (let i = 0; i < 2; i++) await click(page, "stage-give-tg-inc");
  await click(page, "stage-receive-cm-inc");
  await page.getByTestId("stage-status-invalid").waitFor();
  await shotDesk(page, testInfo, "1-before-closed-list");
});
