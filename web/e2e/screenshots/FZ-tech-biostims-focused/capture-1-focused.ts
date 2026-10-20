import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openFocusedAfter, openFocusedBefore, SIZES, type SizeName } from "./scene";

test.describe.configure({ retries: 2 });

for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`focused research before ${size}`, async ({ page }, testInfo) => {
    await openFocusedBefore(page, size);
    await expect(page.getByTestId("technology-modal")).toHaveCount(0);
    await shot(page, testInfo, `before-focused-${size}`);
  });
  test(`focused research after ${size}`, async ({ page }, testInfo) => {
    await openFocusedAfter(page, size);
    await expect(page.getByTestId("tech-card-amd")).toHaveAttribute("data-selectable", "true");
    await expect(page.getByTestId("tech-card-gd")).toHaveAttribute("data-selectable", "false");
    await page.getByTestId("tech-card-amd").click();
    await expect(page.getByTestId("confirm-research-btn")).toBeEnabled();
    await shot(page, testInfo, `after-focused-${size}`);
  });
}
