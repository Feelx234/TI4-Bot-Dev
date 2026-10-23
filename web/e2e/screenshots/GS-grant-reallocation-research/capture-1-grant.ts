import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openGrant, prefix, SIZES, type SizeName } from "./scene";

// Research Grant Reallocation: "The elected player gains any 1 technology of their choice."
// Before: a plain option list. After: the technology dialog with every offered card pickable.
test.describe.configure({ retries: 2 });

for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`grant reallocation ${size}`, async ({ page }, testInfo) => {
    await openGrant(page, size);
    if (prefix === "before") {
      await shot(page, testInfo, `${prefix}-grant-${size}`);
      return;
    }
    await page.getByTestId("technology-modal").waitFor();
    // Prerequisites are not needed: gd, hm and x89c4 are pickable for a seat that lacks them.
    for (const id of ["gd", "hm", "x89c4", "sr"]) {
      await expect(page.getByTestId(`tech-card-${id}`)).toHaveAttribute("data-selectable", "true");
    }
    await expect(page.getByTestId("tech-card-sar")).toHaveAttribute("data-selectable", "false");
    await expect(page.getByTestId("confirm-research-btn")).toBeDisabled();
    await page.getByTestId("tech-card-hm").click();
    await expect(page.getByTestId("confirm-research-btn")).toBeEnabled();
    await shot(page, testInfo, `${prefix}-grant-${size}`);
  });
}
