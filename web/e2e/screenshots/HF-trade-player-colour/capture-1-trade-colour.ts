import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { SIZES, checking, openTrade, prefix } from "./scene";

// The trade dialog names its partner with the partner's seat colour (badge: colour, shape, number).
for (const size of SIZES) {
  for (const mode of ["propose", "answer"] as const) {
    test(`trade ${mode} at ${size.name}`, async ({ page }, testInfo) => {
      await openTrade(page, mode, size);
      if (mode === "answer") await page.getByTestId("offer-net").waitFor();
      else await page.getByTestId("stage-receive").waitFor();
      if (checking) {
        const header = page.getByTestId("trade-header-partner");
        await expect(header).toContainText("Rina");
        await expect(header.getByLabel("Position 4")).toBeVisible();
        await expect(header).toHaveAttribute("data-seat-color", "#EF4444");
        if (mode === "propose") await expect(page.getByTestId("stage-partner")).toContainText("Rina");
        else await expect(page.getByTestId("offer-proposer").first()).toContainText("Rina");
        // Still readable on a phone: nothing sticks out sideways.
        const overflow = await page.evaluate(
          () => document.querySelector(".choice-workflow-modal")!.scrollWidth - document.documentElement.clientWidth,
        );
        expect(overflow, "no horizontal overflow").toBeLessThanOrEqual(0);
      }
      await shot(page, testInfo, `${prefix}-${mode}-${size.name}`);
    });
  }
}
