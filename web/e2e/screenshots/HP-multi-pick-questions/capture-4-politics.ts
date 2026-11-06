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
    await expect(page.getByTestId("choice-prompt")).toContainText("Place Minister of Commerce and Mutiny");
    await expect(page.getByTestId("choice-option").first()).toContainText("Minister of Commerce on top");
    await expect(page.getByTestId("choice-search-input")).toHaveCount(0);
    if (size === "390x844") {
      // The cards first, as the dialog opens; then the six orders further down with one picked.
      await assertNoHorizontalScroll(page);
      await shot(page, testInfo, `4-politics-${size}`);
      await page.locator('[data-option-id="mutiny,minister_commerce/"]').click();
      await page.locator('[data-option-id="/mutiny,minister_commerce"]').scrollIntoViewIfNeeded();
      await shot(page, testInfo, `4-politics-${size}-options`);
    } else {
      await page.locator('[data-option-id="mutiny,minister_commerce/"]').click();
      await shot(page, testInfo, `4-politics-${size}`);
    }
    await context.close();
  });
}
