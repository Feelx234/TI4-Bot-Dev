import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { assertNoHorizontalScroll, newPage, openGeneric, politicsChoice } from "./scene";

// Politics primary: both looked-at agendas as cards, the six orders as options with printed names.
for (const size of ["desktop", "390x844"] as const) {
  test(`politics both agendas ${size}`, async ({ browser, baseURL }, testInfo) => {
    const { context, page } = await newPage(browser, size, baseURL);
    await openGeneric(page, politicsChoice);
    await expect(page.getByTestId("agenda-card")).toHaveCount(2);
    await expect(page.getByTestId("choice-option")).toHaveCount(6);
    await expect(page.getByTestId("choice-option").first()).toContainText("Minister of Commerce on top");
    await page.locator('[data-option-id="mutiny,minister_commerce/"]').click();
    if (size === "390x844") await assertNoHorizontalScroll(page);
    await shot(page, testInfo, `4-politics-${size}`);
    // The options further down on a phone.
    if (size === "390x844") {
      await page.locator('[data-option-id="/mutiny,minister_commerce"]').scrollIntoViewIfNeeded();
      await shot(page, testInfo, `4-politics-${size}-options`);
    }
    await context.close();
  });
}
