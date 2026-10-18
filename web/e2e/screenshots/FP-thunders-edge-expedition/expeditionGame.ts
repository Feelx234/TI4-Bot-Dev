import { expect, type Page } from "@playwright/test";
import { clickOnce } from "../../offlineDriver";
import { SIZES, requireEngine } from "../DO-offline-game/offlineGame";

export { SIZES };

/** Seeds of the captures: the engine and the rule bots are deterministic, so is the table. */
export const SEED = 12;

/** The start preset of the captures: four slices claimed by the bots, two left for the human. */
export const PRESET = "expedition";

/**
 * Opens the setup form of the real app (real launcher, real engine in a Web Worker) with every
 * call to a server blocked, switches Thunder's Edge content on and starts a 3-player game as seat 1.
 */
export async function startExpeditionGame(page: Page, options: { thundersEdge?: boolean; preset?: boolean } = {}): Promise<string> {
  requireEngine();
  await page.route(/\/(api|ws|health)(\/|$)/, (route) => route.abort());
  await page.goto("/offline");
  await page.getByTestId("offline-nickname").fill("Ada");
  await page.getByTestId("offline-player-count").selectOption("3");
  await page.getByTestId("offline-seat").selectOption("1");
  await page.getByTestId("offline-seed").fill(String(SEED));
  if (options.preset ?? true) await page.getByTestId("offline-preset").fill(PRESET);
  if (options.thundersEdge ?? true) await page.getByTestId("offline-thunders-edge").getByRole("checkbox").check();
  await page.getByTestId("offline-start").click();
  await expect(page.getByTestId("offline-game")).toBeVisible({ timeout: 60_000 });
  const id = /\/offline\/games\/([^/?#]+)/.exec(page.url())?.[1];
  if (!id) throw new Error(`not on a game page: ${page.url()}`);
  return decodeURIComponent(id);
}

/** Plays with the dumb clicker until `ready()` says the screen is the one wanted. */
export async function playUntilScreen(page: Page, ready: () => Promise<boolean>, maxClicks = 600) {
  for (let i = 0; i < maxClicks; i++) {
    if (await ready()) return;
    const clicked = await clickOnce(page);
    await page.waitForTimeout(clicked ? 120 : 300);
  }
  throw new Error("never reached the wanted screen");
}

/** The turn bar of the human with no decision dialog open: the human may act. */
export async function atActionMenu(page: Page): Promise<boolean> {
  const bar = await page.getByTestId("turn-bar-pass").count();
  const dialog = await page.locator('[data-testid="pending-choice-dialog"], [data-testid="decision-modal"]').count();
  return bar > 0 && dialog === 0;
}

/** Plays until the human's turn is closing (End turn is enabled) with no dialog open. */
export async function untilClosing(page: Page, maxClicks = 400) {
  for (let i = 0; i < maxClicks; i++) {
    const end = page.getByTestId("turn-bar-end");
    const dialog = await page.locator('[data-testid="pending-choice-dialog"], [data-testid="decision-modal"]').count();
    if ((await end.count()) > 0 && (await end.isEnabled()) && dialog === 0) return;
    const clicked = await clickOnceNoEnd(page);
    await page.waitForTimeout(clicked ? 120 : 300);
  }
  throw new Error("the turn never closed");
}

/** `clickOnce` of the offline driver, except that it never ends the turn by itself. */
async function clickOnceNoEnd(page: Page): Promise<boolean> {
  const end = page.getByTestId("turn-bar-end");
  if ((await end.count()) > 0 && (await end.isEnabled())) return false;
  return clickOnce(page);
}
