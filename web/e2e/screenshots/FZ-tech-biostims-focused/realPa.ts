import { expect, type Page } from "@playwright/test";
import { clickOnce } from "../../offlineDriver";
import { SEED, requireEngine } from "../DO-offline-game/offlineGame";

/**
 * The real wasm engine with the `research` / `research_nopa` start preset (every seat holds two
 * exhausted specialty planets; `research` also gives Psychoarchaeology), driven through the real
 * UI as seat 3: the strategy draft takes the Technology card, then its primary is used.
 */
export async function openTechnologyPrimary(page: Page, preset: "research" | "research_nopa", stopAtTurn = false) {
  requireEngine();
  await page.route(/\/(api|ws|health)(\/|$)/, (route) => route.abort());
  await page.goto("/offline");
  await page.getByTestId("offline-nickname").fill("Ada");
  await page.getByTestId("offline-player-count").selectOption("4");
  await page.getByTestId("offline-seat").selectOption("3");
  await page.getByTestId("offline-seed").fill(String(SEED));
  await page.getByTestId("offline-preset").fill(preset);
  await page.getByTestId("offline-start").click();
  await expect(page.getByTestId("offline-game")).toBeVisible({ timeout: 60_000 });
  for (let i = 0; i < 300; i++) {
    if (!stopAtTurn && (await page.getByTestId("technology-modal").count())) return;
    if (stopAtTurn && (await page.getByTestId("turn-bar-pass").count())) return;
    if (false) console.log(`STEP ${i} ${(await page.getByTestId("turn-status-banner").innerText().catch(() => "?")).replace(/\s+/g, " ")} | ${await page.locator("[data-testid^='turn-bar-strategic-']").evaluateAll((e) => e.map((x) => x.getAttribute("data-testid")))}`);
    if (false) console.log(`OPTS ${await page.locator('[data-testid="choice-option"]').evaluateAll((e) => e.map((x) => x.getAttribute("data-option-id")))}`);
    const draft = page.locator('[data-testid="choice-option"][data-option-id*="technology"]');
    if (await draft.count()) {
      await draft.first().click();
      await page.locator("button:text-matches('^Choose card$')").click({ timeout: 2_000 }).catch(() => undefined);
      await page.waitForTimeout(200);
      continue;
    }
    if (await page.getByTestId("choice-option").count()) {
      await page.getByTestId("choice-option").first().click({ timeout: 2_000 }).catch(() => undefined);
      await page.locator("button:text-matches('^Choose card$')").click({ timeout: 2_000 }).catch(() => undefined);
      await page.waitForTimeout(300);
      continue;
    }
    const primary = stopAtTurn ? page.locator("[data-never]") : page.locator('[data-testid^="turn-bar-strategic-"][data-testid*="technology"]:not([disabled])');
    if (await primary.count()) {
      await primary.first().click({ timeout: 2_000 }).catch(() => undefined);
      await page.waitForTimeout(300);
      continue;
    }
    const clicked = await clickOnce(page);
    await page.waitForTimeout(clicked ? 150 : 300);
  }
  throw new Error("never reached the Technology primary");
}
