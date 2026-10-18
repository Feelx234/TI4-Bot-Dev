import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { SIZES, openPlanner, openStatusWaiting } from "./scene";

for (const size of SIZES) {
  test(`chip while waiting, ${size.name}`, async ({ page }, testInfo) => {
    await openStatusWaiting(page, size);
    await expect(page.getByTestId("status-plan-chip")).toContainText("Plan status tokens");
    await shot(page, testInfo, `1-chip-${size.name}`);
  });

  test(`planning the gain, ${size.name}`, async ({ page }, testInfo) => {
    await openStatusWaiting(page, size);
    await openPlanner(page);
    await expect(page.getByTestId("status-plan-save")).toBeDisabled();
    await page.getByTestId("status-plan-gain-toggle").check();
    // Two tokens by default; move one to fleet.
    await page.getByTestId("status-plan-gain-tactic-minus").click();
    await page.getByTestId("status-plan-gain-fleet-plus").click();
    await expect(page.getByTestId("status-plan-gain-note")).toContainText("2 tokens");
    await shot(page, testInfo, `2-plan-gain-${size.name}`);
    await page.getByTestId("status-plan-save").click();
    await expect(page.getByTestId("status-plan-chip")).toContainText("Gain 2 command tokens: 1 tactic, 1 fleet");
  });

  test(`planning the redistribution, ${size.name}`, async ({ page }, testInfo) => {
    await openStatusWaiting(page, size);
    await openPlanner(page);
    await page.getByTestId("status-plan-gain-toggle").check();
    await page.getByTestId("status-plan-redistribute-toggle").check();
    // 8 held + 2 gained = 10 tokens; a wrong total cannot be saved.
    await page.getByTestId("status-plan-redis-strategic-plus").click();
    await expect(page.getByTestId("status-plan-problem")).toContainText("Remove 1");
    await expect(page.getByTestId("status-plan-save")).toBeDisabled();
    await shot(page, testInfo, `3a-plan-redistribute-wrong-total-${size.name}`);
    await page.getByTestId("status-plan-redis-strategic-minus").click();
    await page.getByTestId("status-plan-redis-tactic-minus").click();
    await page.getByTestId("status-plan-redis-fleet-plus").click();
    await expect(page.getByTestId("status-plan-problem")).toContainText("Arranges all 10 tokens");
    await shot(page, testInfo, `3b-plan-redistribute-${size.name}`);
    await page.getByTestId("status-plan-save").click();
    await expect(page.getByTestId("status-plan-chip")).toContainText("Redistribute to tactic 4 / fleet 4 / strategy 2");
    await shot(page, testInfo, `4-saved-chip-${size.name}`);
  });

  test(`plan cleared, ${size.name}`, async ({ page }, testInfo) => {
    await openStatusWaiting(page, size);
    await openPlanner(page);
    await page.getByTestId("status-plan-gain-toggle").check();
    await page.getByTestId("status-plan-save").click();
    await expect(page.getByTestId("status-plan-clear-chip")).toBeVisible();
    await page.getByTestId("status-plan-clear-chip").click();
    await expect(page.getByTestId("status-plan-chip")).toContainText("Plan status tokens");
    await expect(page.getByTestId("status-plan-clear-chip")).toHaveCount(0);
    await shot(page, testInfo, `7-cleared-${size.name}`);
  });
}
