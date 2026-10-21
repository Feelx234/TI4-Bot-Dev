import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { SIZES, ministerOfPeaceOffer, openScene, prefix, legacy, type SizeName } from "./scene";

// Minister of Peace: the law as printed, the activated system by its planets and who activated it.
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`Minister of Peace offer ${size}`, async ({ page }, testInfo) => {
    const { advance, submissions } = await openScene(page, size);
    advance(1, null);
    advance(2, ministerOfPeaceOffer());
    await expect(page.getByTestId("pending-choice-dialog")).toBeVisible();
    if (!legacy) {
      const dialog = page.getByTestId("pending-choice-dialog");
      await expect(dialog).toContainText("immediately end the active player's turn");
      await expect(dialog).toContainText("System 27 · New Albion");
      await expect(page.getByTestId("minister_of_peace-switch-input")).not.toBeChecked();
    }
    await page.waitForTimeout(800);
    expect(submissions).toHaveLength(0);
    await shot(page, testInfo, `${prefix}-5-minister-offer-${size}`);
  });
}
