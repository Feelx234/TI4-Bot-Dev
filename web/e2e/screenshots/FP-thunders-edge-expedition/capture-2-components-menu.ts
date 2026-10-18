import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { SIZES, atActionMenu, playUntilScreen, startExpeditionGame, untilClosing } from "./expeditionGame";

// The turn bar of a real offline game (real engine in a Web Worker, rule bots, seed 12, 3 players).
// The `expedition` start preset has the bots hold four of the six slices; the human can pay two.
for (const [size, viewport] of Object.entries(SIZES)) {
  test(`without the switch ${size}`, async ({ page }, testInfo) => {
    test.setTimeout(240_000);
    await page.setViewportSize(viewport);
    await startExpeditionGame(page, { thundersEdge: false, preset: false });
    await playUntilScreen(page, () => atActionMenu(page));
    // What a table that never switched the content on offers: no Thunder's Edge anywhere.
    await expect(page.getByTestId("turn-bar-components")).toBeDisabled();
    await shot(page, testInfo, `1-switch-off-no-components-${size}`);
  });

  test(`components menu ${size}`, async ({ page }, testInfo) => {
    test.setTimeout(240_000);
    await page.setViewportSize(viewport);
    await startExpeditionGame(page);
    await playUntilScreen(page, () => atActionMenu(page));
    await page.getByTestId("turn-bar-components").click();
    await expect(page.getByTestId("turn-bar-group-intro")).toBeVisible();
    await shot(page, testInfo, `2-before-the-action-${size}`);
    await page.keyboard.press("Escape");

    await untilClosing(page);
    await shot(page, testInfo, `3-end-of-turn-${size}`);
    await page.getByTestId("turn-bar-components").click();
    await expect(page.getByTestId("turn-bar-item-component|expedition|secret")).toBeEnabled();
    await expect(page.getByTestId("turn-bar-item-component|expedition|resources")).toHaveAttribute("aria-disabled", "true");
    await shot(page, testInfo, `4-slices-and-claimants-${size}`);

    // Hover a slice: the printed text and the breakthrough it grants.
    if (size === "desktop") {
      await page.getByTestId("turn-bar-item-component|expedition|secret").hover();
      await shot(page, testInfo, `5-slice-hover-breakthrough-${size}`);
    }

    // Claim the secret-objective slice: the only secret is discarded without asking.
    await page.getByTestId("turn-bar-item-component|expedition|secret").click();
    await expect(page.getByTestId("turn-bar-end")).toBeEnabled({ timeout: 20_000 });
    await page.getByTestId("turn-bar-components").click();
    await expect(page.getByTestId("turn-bar-item-component|expedition|secret")).toHaveAttribute("aria-disabled", "true");
    await expect(page.getByTestId("turn-bar-item-component|expedition|tech_planet")).toHaveAttribute("aria-disabled", "true");
    await shot(page, testInfo, `6-one-slice-per-turn-${size}`);
  });
}
