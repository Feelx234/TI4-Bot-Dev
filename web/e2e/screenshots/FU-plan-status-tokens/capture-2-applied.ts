import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { SIZES, acceptBatches, gainQuestion, openPlanner, openStatusWaiting, push, redistributeQuestion } from "./scene";

for (const size of SIZES) {
  test(`gain answered from the plan (auto), ${size.name}`, async ({ page }, testInfo) => {
    const game = await openStatusWaiting(page, size, "auto");
    await acceptBatches(page, game);
    await openPlanner(page);
    await page.getByTestId("status-plan-gain-toggle").check();
    await page.getByTestId("status-plan-gain-tactic-minus").click();
    await page.getByTestId("status-plan-gain-fleet-plus").click();
    await page.getByTestId("status-plan-save").click();
    push(game, { version: 42, history: { cursor: 41, redo_count: 0, generation: 0 }, choice: gainQuestion(2) });
    await expect(page.getByTestId("status-plan-autoplay-toast")).toContainText("Gain 2 command tokens: 1 tactic, 1 fleet");
    await expect(page.getByTestId("pending-choice-dialog")).toHaveCount(0);
    await shot(page, testInfo, `5a-gain-auto-toast-${size.name}`);
    await expect(page.getByTestId("status-plan-answered-toast")).toContainText("Answered from your plan");
    await shot(page, testInfo, `5b-gain-answered-${size.name}`);
  });

  test(`redistribution ready to confirm (review), ${size.name}`, async ({ page }, testInfo) => {
    const game = await openStatusWaiting(page, size, "review");
    await openPlanner(page);
    await page.getByTestId("status-plan-gain-toggle").check();
    await page.getByTestId("status-plan-redistribute-toggle").check();
    await page.getByTestId("status-plan-redis-tactic-minus").click();
    await page.getByTestId("status-plan-redis-fleet-plus").click();
    await page.getByTestId("status-plan-save").click();
    push(game, { version: 42, history: { cursor: 41, redo_count: 0, generation: 0 }, choice: redistributeQuestion({ tactic: 5, fleet: 3, strategic: 2 }) });
    await expect(page.getByTestId("status-plan-bar")).toContainText("Redistribute to tactic 4 / fleet 4 / strategy 2");
    await expect(page.getByTestId("token-status-plan")).toBeVisible();
    await shot(page, testInfo, `6-redistribution-from-plan-${size.name}`);
  });

  test(`plan needing review, ${size.name}`, async ({ page }, testInfo) => {
    const game = await openStatusWaiting(page, size, "auto");
    await openPlanner(page);
    await page.getByTestId("status-plan-gain-toggle").check();
    await page.getByTestId("status-plan-save").click();
    // The game gives 3 tokens (an effect added one), not the 2 planned: nothing is applied.
    push(game, { version: 42, history: { cursor: 41, redo_count: 0, generation: 0 }, choice: gainQuestion(3) });
    await expect(page.getByTestId("status-plan-needs-review")).toContainText("You gain 3 command tokens now, not the 2 you planned");
    await expect(page.getByTestId("command-token-panel")).toBeVisible();
    await shot(page, testInfo, `8a-needs-review-gain-${size.name}`);
  });

  test(`redistribution needing review, ${size.name}`, async ({ page }, testInfo) => {
    const game = await openStatusWaiting(page, size, "auto");
    await openPlanner(page);
    await page.getByTestId("status-plan-redistribute-toggle").check();
    await page.getByTestId("status-plan-save").click();
    // The plan arranges 10 tokens (8 held + 2); the game now asks about 11.
    push(game, { version: 42, history: { cursor: 41, redo_count: 0, generation: 0 }, choice: redistributeQuestion({ tactic: 5, fleet: 3, strategic: 3 }) });
    await expect(page.getByTestId("status-plan-needs-review")).toContainText("You hold 11 command tokens now");
    await shot(page, testInfo, `8b-needs-review-redistribute-${size.name}`);
  });
}
