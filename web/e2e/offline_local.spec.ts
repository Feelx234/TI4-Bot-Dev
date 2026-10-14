import { expect, test, type Page } from "@playwright/test";
import { playUntil, startOfflineGame, storedAnswers } from "./offlineDriver";

// The offline game, end to end, with the production build only (see offline.config.ts): the
// engine runs in a Web Worker (JSPI), and nothing but the built static files is ever fetched.

const baseOrigin = (page: Page) => new URL(page.url() || "http://127.0.0.1").origin;

test("plays an offline game with the network blocked and resumes after a refresh", async ({ page, context, baseURL }) => {
  const origin = new URL(baseURL!).origin;
  const requests: string[] = [];
  // Only the built files are served; every call to a server (and anything off-origin) fails.
  await context.route("**/*", (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== origin || /^\/(api|ws|advisor|health)(\/|$)/.test(url.pathname)) return route.abort();
    requests.push(`${route.request().method()} ${url.pathname}`);
    return route.continue();
  });
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));

  const gameId = await startOfflineGame(page, 4);
  await expect(page.getByTestId("offline-banner")).toBeVisible();
  // The engine and its worker are loaded now: cut the network entirely.
  await context.setOffline(true);

  const played = await playUntil(page, gameId, 20);
  expect(played).toBeGreaterThanOrEqual(20);
  // Hidden offline: turn redo, advisor.
  await expect(page.getByTestId("turn-redo-btn")).toHaveCount(0);

  // Refresh: the page comes back from the browser cache of the built files; the game from the store.
  await context.setOffline(false);
  const before = await storedAnswers(page, gameId);
  await page.reload();
  await page.waitForSelector('[data-testid="offline-game"]', { timeout: 60_000 });
  expect(page.url()).toContain(`/offline/games/${gameId}`);
  expect(await storedAnswers(page, gameId)).toBe(before);
  // And it plays on from there.
  await context.setOffline(true);
  await playUntil(page, gameId, before + 5);

  // Nothing but static files of the build was requested.
  const unexpected = requests.filter((r) => !/^GET \/(assets\/|offline|$)/.test(r));
  expect(unexpected).toEqual([]);
  expect(requests.some((r) => /\.wasm$/.test(r))).toBe(true);
  expect(errors).toEqual([]);
  expect(baseOrigin(page)).toBe(origin);
});
