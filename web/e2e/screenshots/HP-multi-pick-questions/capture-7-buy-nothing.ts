import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { assertNoHorizontalScroll, leadershipPurchasePrimary, newPage, openLeadership } from "./scene";

// Leadership's primary after the three free tokens: buying nothing is the first answer again (the
// question lists the decline first; the panel's confirm reads "No purchase" with nothing staged
// and is enabled), against the secondary where at least one token is bought (capture 2).
for (const size of ["desktop", "390x844"] as const) {
  test(`leadership primary may buy nothing ${size}`, async ({ browser, baseURL }, testInfo) => {
    const { context, page } = await newPage(browser, size, baseURL);
    await openLeadership(page, leadershipPurchasePrimary);
    await expect(page.getByTestId("token-buy-count")).toHaveText("0");
    await expect(page.getByTestId("token-confirm")).toBeEnabled();
    await expect(page.getByTestId("token-confirm")).toHaveText("No purchase");
    if (size === "390x844") await assertNoHorizontalScroll(page);
    await shot(page, testInfo, `7-primary-buy-nothing-${size}`);
    await context.close();
  });
}
