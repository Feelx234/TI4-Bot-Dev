import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { assertNoHorizontalScroll, leadershipPrimary, leadershipPurchase, newPage, openLeadership } from "./scene";

// Leadership primary: the one gain question for the three free tokens with the purchase planned on
// the same screen (the host maps the plan onto the gain, the purchase multi-pick and the gain of
// the bought tokens).
for (const size of ["desktop", "390x844"] as const) {
  test(`leadership primary ${size}`, async ({ browser, baseURL }, testInfo) => {
    const { context, page } = await newPage(browser, size, baseURL);
    await openLeadership(page, leadershipPrimary);
    await expect(page.getByTestId("choice-prompt")).toContainText("Gain 3 command tokens");
    // Jord 2 + Centauri 3 = 5: one token, the largest item last (the engine takes it, wasting 2).
    await page.getByTestId("token-payment-planet-jord").click();
    await page.getByTestId("token-payment-planet-centauri").click();
    await expect(page.getByTestId("token-buy-count")).toHaveText("1");
    for (const pool of ["tactic", "tactic", "fleet", "strategic"]) await page.getByTestId(`token-plus-${pool}`).click();
    await expect(page.getByTestId("token-confirm")).toBeEnabled();
    await expect(page.getByTestId("token-payment-problem")).toHaveCount(0);
    if (size === "390x844") await assertNoHorizontalScroll(page);
    await shot(page, testInfo, `2-leadership-primary-${size}`);
    await context.close();
  });
}

// Leadership's purchase question alone (the secondary after following): one multi-pick of at
// least one token. As opened, Confirm waits for a payment; then two tokens staged.
for (const size of ["desktop", "390x844"] as const) {
  for (const stage of ["open", "staged"] as const) {
    if (size === "390x844" && stage === "open") continue;
    test(`leadership purchase ${stage} ${size}`, async ({ browser, baseURL }, testInfo) => {
      const { context, page } = await newPage(browser, size, baseURL);
      await openLeadership(page, leadershipPurchase);
      if (stage === "open") {
        await expect(page.getByTestId("token-confirm")).toBeDisabled();
      } else {
        for (const planet of ["dal_bootha", "rarron", "centauri"]) await page.getByTestId(`token-payment-planet-${planet}`).click();
        await expect(page.getByTestId("token-buy-count")).toHaveText("2");
        await page.getByTestId("token-plus-tactic").click();
        await page.getByTestId("token-plus-fleet").click();
        await expect(page.getByTestId("token-confirm")).toBeEnabled();
      }
      if (size === "390x844") await assertNoHorizontalScroll(page);
      await shot(page, testInfo, `2-leadership-purchase-${stage}-${size}`);
      await context.close();
    });
  }
}
