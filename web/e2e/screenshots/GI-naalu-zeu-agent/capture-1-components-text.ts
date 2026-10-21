import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { SIZES, ZEU_ACTION, checking, openTurnMenu, prefix } from "./scene";

// The Components menu prints the card the game dealt (Prophecy of Kings Z'eu).
for (const size of SIZES) {
  test(`components menu Z'eu at ${size.name}`, async ({ page }, testInfo) => {
    await openTurnMenu(page, size.width, size.height);
    await page.getByTestId("turn-bar-components").click();
    await page.getByTestId("turn-bar-menu-components").waitFor();
    const row = page.getByTestId(`turn-bar-item-${ZEU_ACTION.id}`);
    await row.scrollIntoViewIfNeeded();
    if (checking) await expect(row.getByTestId("turn-bar-row-text")).toContainText("tactical action in a non-home system");
    await shot(page, testInfo, `${prefix}-components-${size.name}`);
  });
}
