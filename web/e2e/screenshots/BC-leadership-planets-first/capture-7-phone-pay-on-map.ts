import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openChoice, prefix, primaryChoice } from "./scene";

// Phone (390x844, touch): paying on the map. The map must stay tappable and a confirm control reachable.
test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
test("leadership primary: phone, pay on the map", async ({ page }, testInfo) => {
  await openChoice(page, primaryChoice());
  await page.getByTestId("token-buy-plus").tap();
  await shot(page, testInfo, `${prefix}-7-phone-bought`);
  await page.getByTestId("token-pay-on-map").tap();
  await page.getByTestId("token-payment-bar").waitFor();
  await shot(page, testInfo, `${prefix}-7-phone-pay-on-map`);
});
