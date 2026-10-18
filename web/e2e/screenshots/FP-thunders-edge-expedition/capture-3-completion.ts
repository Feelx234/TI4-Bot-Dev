import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { SIZES, atActionMenu, playUntilScreen, startExpeditionGame, untilClosing } from "./expeditionGame";

// The expedition played to its end in a real offline game: the human claims the secret-objective
// slice at the end of the first turn and the technology-specialty slice at the end of the second.
// The sixth slice completes it: the finisher picks the system and, three seats being tied with two
// slices each, which seat places the infantry. Thunder's Edge then stands on the map.
for (const [size, viewport] of Object.entries(SIZES)) {
  test(`completion ${size}`, async ({ page }, testInfo) => {
    test.setTimeout(400_000);
    await page.setViewportSize(viewport);
    await startExpeditionGame(page);
    await playUntilScreen(page, () => atActionMenu(page));

    await untilClosing(page);
    await page.getByTestId("turn-bar-components").click();
    await page.getByTestId("turn-bar-item-component|expedition|secret").click();
    await expect(page.getByTestId("turn-bar-end")).toBeEnabled({ timeout: 20_000 });
    await page.getByTestId("turn-bar-end").click();

    // The second turn: the last slice.
    await untilClosing(page);
    await page.getByTestId("turn-bar-components").click();
    await shot(page, testInfo, `1-the-last-slice-${size}`);
    await page.getByTestId("turn-bar-item-component|expedition|tech_planet").click();

    // 1. Where to put it.
    const dialog = page.getByTestId("pending-choice-dialog");
    await expect(dialog).toBeVisible({ timeout: 20_000 });
    await expect(dialog).toContainText("choose a neutral system for Thunder's Edge");
    await shot(page, testInfo, `2-choose-the-system-${size}`);
    const chosen = /in system (\S+)/.exec((await dialog.innerText()))?.[1];
    if (!chosen) throw new Error("no system option in the dialog");
    await dialog.getByTestId("submit-choice-button").click();

    // 2. Which of the tied seats places the infantry.
    await expect(dialog).toContainText("which tied player places the infantry", { timeout: 20_000 });
    await shot(page, testInfo, `3-choose-the-placer-${size}`);
    await dialog.getByTestId("submit-choice-button").click();

    // 3. Thunder's Edge is on the map; the expedition group is gone.
    await expect(dialog).toHaveCount(0, { timeout: 20_000 });
    await shot(page, testInfo, `4-thunders-edge-placed-${size}`);

    // The system that got it: click its tile.
    await page.getByTestId(`system-hex-${chosen}`).click({ force: true });
    await page.waitForTimeout(600);
    await shot(page, testInfo, `5-the-system-${size}`);
  });
}

