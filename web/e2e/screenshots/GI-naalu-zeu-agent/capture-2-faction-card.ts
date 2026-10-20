import { expect, test } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { opponent } from "../_shared/players";
import { shot } from "../_shared/shot";
import { SIZES, checking, naalu, prefix } from "./scene";

// The faction overview must print the same Z'eu as the Components menu.
for (const size of SIZES) {
  test(`faction card Z'eu at ${size.name}`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width: size.width, height: size.height });
    await openMockedGame(page, { players: [naalu, opponent] });
    if (size.width < 600) await page.getByTestId("player-sheet-toggle").click();
    const button = page.getByTestId("faction-info-button").first();
    await button.scrollIntoViewIfNeeded();
    await button.click();
    const card = page.getByTestId("faction-info-card");
    await card.waitFor();
    const agent = card.getByText("Z'eu").first();
    await agent.scrollIntoViewIfNeeded();
    if (checking) await expect(card).toContainText("tactical action in a non-home system");
    if (checking) await expect(card).not.toContainText("return that token");
    await page.waitForTimeout(250);
    await shot(page, testInfo, `${prefix}-faction-${size.name}`);
  });
}
