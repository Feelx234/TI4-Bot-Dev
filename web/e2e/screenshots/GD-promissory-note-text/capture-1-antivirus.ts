import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { SIZES, openNoteReaction, prefix } from "./_scene";

// The Antivirus offered at the start of a combat (the engine option id carries the note's alias).
for (const [size, viewport] of Object.entries(SIZES)) {
  test(`antivirus reaction ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    await openNoteReaction(page, "SPACE_COMBAT_STARTED", "promissory:nekro:antivirus:SPACE_COMBAT_STARTED:after");
    await shot(page, testInfo, `${prefix}1-antivirus-reaction-${size}`);
  });
}
