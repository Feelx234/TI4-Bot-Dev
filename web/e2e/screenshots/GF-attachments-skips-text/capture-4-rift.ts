import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { playerWithHand, opponent, actor } from "../_shared/players";
import { openMockedGame } from "../_shared/mockGame";
import { openLogFully } from "../_shared/eventLog";
import { openCornerGame, cornerCrop, pushEntry, publicEntry } from "../G-corner-notifications/corner";
import { rollEntry, riftDestroyed, riftSurvives } from "../DM-public-dice-rolls/dice";
import { prefix } from "./scene";

const SIZES = { desktop: { width: 1440, height: 900 }, "390x844": { width: 390, height: 844 } } as const;

// Two ships leave the same gravity rift: one rolls a 7 and gets through, one rolls a 2 and is
// destroyed. The toast used to merge them into "hits" ("1 hit": a high die is not a hit).
for (const [size, viewport] of Object.entries(SIZES)) {
  test(`rift toast with two ships ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    const game = await openCornerGame(page, { players: [playerWithHand(), opponent] });
    pushEntry(game, rollEntry({ ...riftSurvives(opponent.id), faces: [7] }));
    pushEntry(game, rollEntry(riftDestroyed(opponent.id)));
    const toast = page.locator('[data-testid="corner-toast"][data-toast-kind="notice"]');
    await toast.waitFor();
    if (prefix === "after") {
      await expect(toast).not.toContainText(/hit/i);
      await expect(toast).toContainText("rolled a 7 at the gravity rift in #67 and the ship survived");
      await expect(toast).toContainText("rolled a 2 at the gravity rift in #67 and the ship was destroyed");
    }
    await page.waitForTimeout(700);
    await shot(page, testInfo, `${prefix}-toast-${size}`, size === "desktop" ? cornerCrop(page) : {});
  });
}

test("rift lines in the event log", async ({ page }, testInfo) => {
  const events = [
    publicEntry("game-1", opponent.id, "activated #01", { action_id: "action_1", action_type: "tactical", action_actor: opponent.id, stage: "movement", decision_count: 1 }),
    rollEntry({ ...riftSurvives(opponent.id), faces: [7] }, { id: "game-2", decision_count: 1 }),
    rollEntry(riftDestroyed(actor), { id: "game-3", decision_count: 1 }),
  ];
  await openMockedGame(page, { players: [playerWithHand(), opponent], events, history: { cursor: 1, redo_count: 0 } });
  await openLogFully(page);
  const rows = page.getByTestId("event-log-roll");
  await expect(rows).toHaveCount(2);
  if (prefix === "after") {
    await expect(rows.nth(0)).toContainText("rolled a 7 at the gravity rift in #67 and the ship survived");
    await expect(rows.nth(1)).toContainText("the ship was destroyed");
    await expect(rows.nth(0).locator('.event-log__die[data-hit]')).toHaveCount(0);
  }
  await page.getByTestId("event-log-list").evaluate((el) => {
    el.scrollTop = el.scrollHeight;
  });
  await shot(page, testInfo, `${prefix}-event-log`, { of: page.locator("#event-log-drawer"), pad: 8 });
});
