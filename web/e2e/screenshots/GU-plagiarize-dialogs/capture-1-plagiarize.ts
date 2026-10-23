import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { OFFERED, openPayment, openPick, prefix, SIZES, type SizeName } from "./scene";

// Plagiarize: "Spend 5 influence and choose a non-faction technology owned by 1 of your neighbors.
// Gain that technology." The engine asks the payment first, then the pick.
test.describe.configure({ retries: 2 });

for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`plagiarize payment ${size}`, async ({ page }, testInfo) => {
    await openPayment(page, size);
    await expect(page.getByTestId("payment-drawer")).toContainText("5");
    await shot(page, testInfo, `${prefix}-payment-${size}`);
  });

  test(`plagiarize pick ${size}`, async ({ page }, testInfo) => {
    await openPick(page, size);
    if (prefix === "before") {
      await shot(page, testInfo, `${prefix}-pick-${size}`);
      return;
    }
    await page.getByTestId("technology-modal").waitFor();
    for (const id of OFFERED) {
      await expect(page.getByTestId(`tech-card-${id}`)).toHaveAttribute("data-selectable", "true");
    }
    // Everything the engine did not offer is gone, not dimmed; no skips or resources row.
    await expect(page.getByTestId("tech-card-ps")).toHaveCount(0);
    await expect(page.getByTestId("tech-skips-list")).toHaveCount(0);
    await expect(page.getByTestId("confirm-research-btn")).toBeDisabled();
    await page.getByTestId("tech-card-hm").click();
    await expect(page.getByTestId("confirm-research-btn")).toBeEnabled();
    await shot(page, testInfo, `${prefix}-pick-${size}`);
  });
}
