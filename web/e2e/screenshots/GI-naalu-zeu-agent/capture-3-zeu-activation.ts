import { expect, test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { opponent } from "../_shared/players";
import { shot } from "../_shared/shot";
import { SIZES, ZEU_QUESTION, checking, naalu, prefix } from "./scene";

// Z'eu's system choice is the tactical action's activation step: highlighted hexes and the
// activation bar, with a Decline beside it (before: a generic choice dialog).
for (const size of SIZES) {
  test(`Z'eu activation at ${size.name}`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width: size.width, height: size.height });
    await openMockedGame(page, { players: [naalu, opponent], choice: ZEU_QUESTION });
    if (checking) {
      await page.getByTestId("system-activation-bar").waitFor();
      await expect(page.getByTestId("decline-activation-btn")).toBeVisible();
    } else {
      await page.waitForTimeout(800);
    }
    await shot(page, testInfo, `${prefix}-activation-${size.name}`);
  });
}
