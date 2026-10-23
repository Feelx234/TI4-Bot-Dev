import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { arriveAtReadyStep, prefix, topmost } from "./scene";

// s2-07-0228 (2026-10-22): after "Choose myself" on the secondary window of Diplomacy, the follow-up
// question (which planets to ready) brought the "Prepared ... Confirm / Choose myself" bar back, over
// the ready-planets bar's Confirm. Now "Choose myself" holds for the whole secondary window, and a bar
// that is up anyway docks above the ready-planets bar instead of over it.
for (const phone of [false, true]) {
  const size = phone ? "phone" : "desktop";
  test.describe(size, () => {
    if (phone) test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
    else test.use({ viewport: { width: 1280, height: 800 } });

    test(`ready step after Choose myself, ${size}`, async ({ page }, testInfo) => {
      await arriveAtReadyStep(page, true);
      const cover = await topmost(page, "ready-bar-confirm");
      console.log(`ready-bar ${prefix} ${size} chose-myself: topmost at Confirm = ${cover}`);
      await shot(page, testInfo, `${prefix}-1-chose-myself-${size}`);
      if (prefix === "after") {
        await expect(page.getByTestId("secondary-prepared-bar")).toHaveCount(0);
        expect(cover).toBe("itself");
      }
    });

    test(`prepared bar stacks above the ready bar, ${size}`, async ({ page }, testInfo) => {
      await arriveAtReadyStep(page, false);
      const cover = await topmost(page, "ready-bar-confirm");
      console.log(`ready-bar ${prefix} ${size} stacked: topmost at Confirm = ${cover}`);
      await shot(page, testInfo, `${prefix}-2-stacked-${size}`);
      if (prefix === "after") expect(cover).toBe("itself");
    });
  });
}
