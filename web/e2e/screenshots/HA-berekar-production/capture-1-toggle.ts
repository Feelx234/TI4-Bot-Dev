import { expect, test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { opponent, playerWithHand } from "../_shared/players";
import { shot } from "../_shared/shot";
import { nothingInReach, withAgent } from "./scene";

// Berekar Berekon (Winnu agent) in the owner's own production dialog: a toggle that lowers the
// displayed total by 2 and is sent first when the builds are confirmed.
const game = (choice: typeof withAgent) => ({ players: [playerWithHand({ faction: "winnu" }), opponent], choice });

test("production dialog with the Berekar Berekon toggle, desktop", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await openMockedGame(page, game(withAgent));
  await page.getByTestId("production-builder-drawer").waitFor();
  await page.getByTestId("produce-unit-btn-build|carrier|1").click();
  await expect(page.getByTestId("production-total-sum")).toContainText("3");
  await page.waitForTimeout(300);
  await shot(page, testInfo, "toggle-off-desktop");
  await page.getByTestId("production-agent-toggle").check();
  await expect(page.getByTestId("production-total-sum")).toContainText("1");
  await expect(page.getByTestId("production-total-discount")).toContainText("2");
  await page.waitForTimeout(300);
  await shot(page, testInfo, "toggle-on-desktop");
});

test("production dialog with only the card within reach, desktop", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await openMockedGame(page, game(nothingInReach));
  await page.getByTestId("production-builder-drawer").waitFor();
  await expect(page.getByTestId("production-agent-button")).toBeVisible();
  await page.waitForTimeout(300);
  await shot(page, testInfo, "only-card-desktop");
});

test.describe("phone", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
  test("production dialog with the toggle on, phone", async ({ page }, testInfo) => {
    await openMockedGame(page, game(withAgent));
    await page.getByTestId("production-builder-drawer").waitFor();
    await page.getByTestId("produce-unit-btn-build|carrier|1").click();
    await page.getByTestId("production-agent-toggle").check();
    await expect(page.getByTestId("production-total-sum")).toContainText("1");
    await page.getByTestId("production-total").scrollIntoViewIfNeeded();
    await page.waitForTimeout(300);
    await shot(page, testInfo, "toggle-on-phone");
  });
});
