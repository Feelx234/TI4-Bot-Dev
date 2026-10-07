import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openWaiting } from "./prep";

// Leadership: how many tokens to buy and into which pool (no strategy token needed).
test("secondary prep: Leadership plan", async ({ page }, testInfo) => {
  await openWaiting(page, { name: "Leadership", card: "pok1leadership", viewer: { strategic_tokens: 0 } });
  await page.getByTestId("secondary-prep-chip").click();
  await page.getByTestId("prep-follow").click();
  await page.getByTestId("prep-tokens-2").click();
  await page.getByTestId("prep-pool-fleet").click();
  await page.getByTestId("secondary-prepared-badge").waitFor();
  await shot(page, testInfo, "13-leadership-plan");
});
