import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { playerWithHand, opponent } from "../_shared/players";
import { openCornerGame, cornerCrop, pushEntry } from "../G-corner-notifications/corner";
import { rollEntry } from "../DM-public-dice-rolls/dice";
import { cannonRoll } from "./cannon";

const SIZES = { desktop: { width: 1440, height: 900 }, "390x844": { width: 390, height: 844 } } as const;
const toastLocator = (page: import("@playwright/test").Page) =>
  page.locator('[data-testid="corner-toast"][data-toast-kind="notice"]');

for (const [size, viewport] of Object.entries(SIZES)) {
  // The feedback case: one PDS II in range rolls a 1. Everyone is told the gun fired and missed.
  test(`space cannon toast: miss ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    const game = await openCornerGame(page, { players: [playerWithHand(), opponent] });
    pushEntry(game, rollEntry(cannonRoll(opponent.id, "pds2", "lodor", [1], 5)));
    const toast = toastLocator(page);
    await toast.waitFor();
    await expect(page.getByTestId("toast-notice-tag")).toHaveText("Space Cannon");
    await expect(toast).toContainText("Space Cannon Offense in #18");
    await expect(toast).toContainText("PDS II on Lodor (needs 5+): 1 miss - no hits");
    await page.waitForTimeout(700);
    const box = (await toast.boundingBox())!;
    expect(box.x + box.width).toBeLessThanOrEqual(viewport.width);
    await shot(page, testInfo, `1-miss-${size}`, size === "desktop" ? cornerCrop(page) : {});
  });

  test(`space cannon toast: hit ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    const game = await openCornerGame(page, { players: [playerWithHand(), opponent] });
    pushEntry(game, rollEntry(cannonRoll(opponent.id, "pds2", "lodor", [5], 5)));
    const toast = toastLocator(page);
    await toast.waitFor();
    await expect(toast).toContainText("PDS II on Lodor (needs 5+): 5 hit - 1 hit");
    await page.waitForTimeout(700);
    await shot(page, testInfo, `2-hit-${size}`, size === "desktop" ? cornerCrop(page) : {});
  });

  // Several guns and dice of one volley: one toast, a line per gun, a total.
  test(`space cannon toast: several guns ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    const game = await openCornerGame(page, { players: [playerWithHand(), opponent] });
    pushEntry(game, rollEntry(cannonRoll(opponent.id, "pds2", "lodor", [2, 9], 5)));
    pushEntry(game, rollEntry(cannonRoll(opponent.id, "pds", "mecatol_rex", [3], 6)));
    pushEntry(game, rollEntry(cannonRoll(opponent.id, "xxcha_mech", undefined, [8], 8, { unit_system: "19" })));
    const toast = toastLocator(page);
    await toast.waitFor();
    await expect(toast).toHaveCount(1);
    await expect(toast).toContainText("Total: 2 hits");
    await page.waitForTimeout(700);
    await shot(page, testInfo, `3-several-guns-${size}`, size === "desktop" ? cornerCrop(page) : {});
  });
}

// Space Cannon Defense during an invasion reads the same way.
test("space cannon toast: defense", async ({ page }, testInfo) => {
  const game = await openCornerGame(page, { players: [playerWithHand(), opponent] });
  pushEntry(
    game,
    rollEntry(cannonRoll(opponent.id, "pds", "mecatol_rex", [6, 2], 6, { reason: "space cannon defense", label: "Space cannon defense" })),
  );
  const toast = toastLocator(page);
  await toast.waitFor();
  await expect(toast).toContainText("Space Cannon Defense in #18");
  await page.waitForTimeout(700);
  await shot(page, testInfo, "4-defense", cornerCrop(page));
});
