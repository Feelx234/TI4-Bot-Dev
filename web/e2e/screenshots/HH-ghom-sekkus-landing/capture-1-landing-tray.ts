import { expect, test } from "@playwright/test";
import { openLanding, prefix, take, viewports } from "./scene";

// The landing tray with G'hom Sek'kus: the extra sources, then one staged from Lodor.
for (const v of viewports) {
  test(`landing-tray-${v.id}`, async ({ page }, testInfo) => {
    await openLanding(page, v);
    await page.getByTestId("invasion-landing-tray").scrollIntoViewIfNeeded();
    await take(page, testInfo, "offered", v);
    if (prefix === "after") {
      await expect(page.getByTestId("invasion-landing-origin")).toHaveCount(3);
      await page.getByRole("button", { name: "land infantry on jord from Lodor" }).click();
      await expect(page.getByRole("button", { name: "land mech on jord from Lodor" })).toBeDisabled();
      await page.getByRole("button", { name: "land infantry on jord from Lodor" }).scrollIntoViewIfNeeded();
      await take(page, testInfo, "staged", v);
    }
  });
}
