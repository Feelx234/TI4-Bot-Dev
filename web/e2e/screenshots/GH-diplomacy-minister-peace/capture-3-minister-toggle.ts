import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { SIZES, ministerOfPeaceOffer, openScene, openSheet, legacy, type SizeName } from "./scene";

// The toggle: a checkbox in the offer, the Prompts control on the player sheet, the auto-decline.
test.skip(legacy, "the base commit has no switch for Minister of Peace");

for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`Minister of Peace switch ticked in the offer ${size}`, async ({ page }, testInfo) => {
    const { advance, submissions } = await openScene(page, size);
    advance(1, null);
    advance(2, ministerOfPeaceOffer("peace-2"));
    await page.getByTestId("minister_of_peace-switch-input").check();
    await expect(page.getByTestId("minister_of_peace-switch-input")).toBeChecked();
    expect(submissions).toHaveLength(0);
    await shot(page, testInfo, `after-6-switch-ticked-${size}`);
  });

  test(`Minister of Peace on the sheet and skipped ${size}`, async ({ page }, testInfo) => {
    const { advance, submissions } = await openScene(page, size);
    await openSheet(page, size);
    const button = page.getByTestId("minister_of_peace-ask-btn");
    await expect(button).toHaveText("Minister of Peace ask");
    await shot(page, testInfo, `after-7-sheet-ask-${size}`);
    await button.click();
    await expect(button).toHaveText("Minister of Peace skipped");
    await shot(page, testInfo, `after-8-sheet-skipped-${size}`);
    if (size !== "desktop") await page.getByTestId("player-sheet-toggle").click();
    advance(1, null);
    await page.waitForTimeout(300);
    advance(2, ministerOfPeaceOffer("peace-3"));
    await expect.poll(() => submissions.length).toBe(1);
    expect(JSON.stringify(submissions[0])).toContain("decline");
    await expect(page.getByTestId("corner-toast")).toContainText("Minister of Peace skipped");
    await shot(page, testInfo, `after-9-declined-toast-${size}`);
  });
}
