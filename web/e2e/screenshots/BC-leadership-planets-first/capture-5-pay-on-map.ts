import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openChoice, prefix, primaryChoice } from "./scene";

// The panel minimised so the map pays: ringed planets and the bar.
test("leadership primary: pay on the map", async ({ page }, testInfo) => {
  await openChoice(page, primaryChoice());
  await page.getByTestId("token-buy-plus").click();
  await page.getByTestId("token-pay-on-map").click();
  await page.getByTestId("token-payment-bar").waitFor();
  await shot(page, testInfo, `${prefix}-5-pay-on-map`);
});
