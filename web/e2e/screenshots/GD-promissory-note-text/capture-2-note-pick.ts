import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { SIZES, openNotePick, prefix } from "./_scene";

// Choosing a note to give (Mentak commander, Visionaria Select, Naalu, Empyrean): the engine's
// option labels are the raw note ids (antivirus:nekro, support:jolnar, ...).
for (const [size, viewport] of Object.entries(SIZES)) {
  test(`note pick ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    await openNotePick(page);
    await shot(page, testInfo, `${prefix}2-note-pick-${size}`);
  });
}
