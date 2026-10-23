import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { opponent } from "../_shared/players";
import { SIZES, openStatusWaiting } from "../FU-plan-status-tokens/scene";

// `TI4_SHOT_PREFIX=before` names the shots taken on the commit without the change.
const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

for (const size of SIZES) {
  test(`singleplayer: every other seat is a bot, ${size.name}`, async ({ page }, testInfo) => {
    await openStatusWaiting(page, size, undefined, [opponent.id]);
    await page.waitForTimeout(300);
    if (prefix === "after") {
      await expect(page.getByTestId("status-plan-chip")).toHaveCount(0);
      await expect(page.getByTestId("status-plan-dialog")).toHaveCount(0);
    }
    await shot(page, testInfo, `${prefix}-solo-${size.name}`);
  });

  test(`multiplayer: another human seat, ${size.name}`, async ({ page }, testInfo) => {
    await openStatusWaiting(page, size);
    await expect(page.getByTestId("status-plan-chip")).toContainText("Plan status tokens");
    await shot(page, testInfo, `${prefix}-multi-${size.name}`);
  });
}
