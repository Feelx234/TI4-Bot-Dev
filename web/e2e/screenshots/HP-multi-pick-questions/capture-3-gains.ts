import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { assertNoHorizontalScroll, gainChoice, newPage, openTokens, predictiveChoice } from "./scene";

// A gain of several tokens is ONE question (a split per option): the panel stages every token.
for (const left of [2, 3]) {
  test(`gain split of ${left}`, async ({ browser, baseURL }, testInfo) => {
    const { context, page } = await newPage(browser, "desktop", baseURL);
    await openTokens(page, gainChoice(left));
    await expect(page.getByTestId("choice-prompt")).toContainText(`Gain ${left} command tokens`);
    const pools = left === 2 ? ["tactic", "fleet"] : ["tactic", "strategic", "strategic"];
    for (const pool of pools) await page.getByTestId(`token-plus-${pool}`).click();
    await expect(page.getByTestId("token-confirm")).toBeEnabled();
    await shot(page, testInfo, `3-gain-${left}-desktop`);
    await context.close();
  });
}

test("gain split of 3 phone", async ({ browser, baseURL }, testInfo) => {
  const { context, page } = await newPage(browser, "390x844", baseURL);
  await openTokens(page, gainChoice(3));
  for (const pool of ["tactic", "fleet", "fleet"]) await page.getByTestId(`token-plus-${pool}`).click();
  await assertNoHorizontalScroll(page);
  await shot(page, testInfo, "3-gain-3-390x844");
  await context.close();
});

// Predictive Intelligence: one redistribution question over every split (no fleet floor).
for (const size of ["desktop", "390x844"] as const) {
  test(`predictive intelligence ${size}`, async ({ browser, baseURL }, testInfo) => {
    const { context, page } = await newPage(browser, size, baseURL);
    await openTokens(page, predictiveChoice);
    await expect(page.getByTestId("command-token-panel")).toHaveAttribute("data-mode", "redistribute");
    await page.getByTestId("token-minus-tactic").click();
    await page.getByTestId("token-minus-tactic").click();
    await page.getByTestId("token-plus-strategic").click();
    await page.getByTestId("token-plus-strategic").click();
    await expect(page.getByTestId("token-confirm")).toBeEnabled();
    if (size === "390x844") await assertNoHorizontalScroll(page);
    await shot(page, testInfo, `3-predictive-${size}`);
    await context.close();
  });
}
