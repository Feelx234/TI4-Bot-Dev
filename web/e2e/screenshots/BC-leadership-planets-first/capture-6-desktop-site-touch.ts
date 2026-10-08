import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openChoice, prefix, primaryChoice } from "./scene";

// A phone with "Request desktop site": the desktop layout (about 980 css px wide) on a touch screen.
test.use({ viewport: { width: 980, height: 760 }, hasTouch: true });
test("leadership primary: desktop site on a phone, pay on the map", async ({ page }, testInfo) => {
  await openChoice(page, primaryChoice());
  await page.getByTestId("token-buy-plus").tap();
  await page.getByTestId("token-pay-on-map").tap();
  await page.getByTestId("token-payment-bar").waitFor();
  await shot(page, testInfo, `${prefix}-6-desktop-site-pay-on-map`);
});
