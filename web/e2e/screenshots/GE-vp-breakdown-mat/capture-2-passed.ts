import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { A, B, C, D, openPlayers, openScene, players, SIZES, type SizeName } from "./scene";

// A seat that passed shows a Passed chip on its mat and in the seat legend over the board; the
// place (1st, 2nd) once the page has seen the passes one after the other. Cleared the next round.
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`passed badge ${size}`, async ({ page }, testInfo) => {
    const game = await openScene(page, size, { passed: [] });
    await expect(page.getByTestId("player-passed")).toHaveCount(0);
    let version = 1000;
    const push = async (passed: string[], round = 3, phase = "action") => {
      const { events: _events, ...update } = game.snapshot;
      game.send({
        ...update,
        type: "state_update",
        game_version: ++version,
        view: { ...update.view, round, phase, players: players(passed) },
      });
      await page.waitForTimeout(300);
    };
    await push([D]);
    await push([D, B]);
    await expect(page.getByTestId("player-passed")).toHaveCount(2);
    if (size === "desktop") {
      await expect(page.getByTestId("seat-passed")).toHaveCount(2);
      await expect(page.getByTestId("seat-passed").first()).toBeVisible();
    }
    await openPlayers(page, size);
    const cards = page.getByTestId("player-card");
    await expect(cards.nth(1).getByTestId("player-passed")).toHaveText("Passed (2nd)");
    await expect(cards.nth(3).getByTestId("player-passed")).toHaveText("Passed (1st)");
    await expect(cards.nth(0).getByTestId("player-passed")).toHaveCount(0);
    await cards.nth(1).scrollIntoViewIfNeeded();
    await shot(page, testInfo, `after-1-mat-${size}`);
    if (size === "desktop") await shot(page, testInfo, "after-2-seat-legend-desktop", { of: page.locator(".board-seat-legend"), pad: 6 });
    else {
      await page.getByTestId("player-sheet-close").click();
      await page.waitForTimeout(300);
      await page.getByTestId("top-menu-button").click();
      await expect(page.getByTestId("seat-passed")).toHaveCount(2);
      await shot(page, testInfo, "after-2-seat-legend-390x844");
      await page.getByTestId("top-menu-close").click();
    }
    // Next round: cleared.
    await push([], 4);
    await expect(page.getByTestId("player-passed")).toHaveCount(0);
    void A;
    void C;
  });
}
