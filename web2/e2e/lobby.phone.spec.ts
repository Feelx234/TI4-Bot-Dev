import { expect, test, type Page } from "@playwright/test";

// The lobby of a table on a Pixel 9 in portrait: the panes of the game shell, Map · Players ·
// Setup, with the footer under every pane.

const LONG = [
  "Bartholomew Featherstonehaugh the Third of Mecatol",
  "Wilhelmina Vanderbilt-Montgomery",
];

const pane = (page: Page, name: string) =>
  page.getByRole("navigation", { name: "Panes" }).getByRole("button", { name });
const main = (page: Page) => page.locator('button[aria-keyshortcuts="Enter"]');
const fits = async (page: Page) =>
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(412);

test("phone: the start screen and the lobby of eight seats with the long names", async ({
  page,
  request,
}) => {
  await page.goto("/");
  await expect(page.getByRole("link", { name: /Multiplayer/ })).toBeVisible();
  await fits(page);
  await page.screenshot({ path: "shots/phone/start.png", animations: "disabled" });

  const response = await request.post("/api/games", {
    data: { player_count: 8, seed: 11, nickname: LONG[0] },
  });
  const made = await response.json();
  const call = async (path: string, data: object, session?: string) =>
    expect(
      (
        await request.post(`/api/games/${made.game_id}/lobby/${path}`, {
          data,
          headers: session ? { "x-ti4-player-session": session } : {},
        })
      ).ok(),
    ).toBe(true);
  await call("join", { kind: "new", nickname: LONG[1] });
  await call("bots", { kind: "smart", count: 3 }, made.player_session);
  await call("bots", { kind: "random", count: 2 }, made.player_session);
  await page.addInitScript(
    ([key, value]) => localStorage.setItem(key, value),
    [
      `ti4.online.${made.game_id}.players`,
      JSON.stringify({ [made.player.id]: made.player_session }),
    ],
  );
  await page.goto(`/?online=${made.game_id}`);

  // The Players pane is first: nothing is cut at the side, and a long name has a second line.
  const rows = page.getByRole("region", { name: "Players" }).getByRole("listitem");
  await expect(rows).toHaveCount(8);
  await expect(rows.nth(0)).toContainText(`${LONG[0]} (you)`);
  await fits(page);
  for (const row of await rows.all()) {
    const box = (await row.boundingBox())!;
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(412);
  }
  const name = rows.nth(0).getByTitle(LONG[0]);
  expect(await name.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
  // The main button is under every pane.
  await expect(main(page)).toHaveText("Start game");
  await expect(main(page)).toBeInViewport();
  await page.screenshot({ path: "shots/phone/online/lobby-players.png", animations: "disabled" });

  await pane(page, "Setup").tap();
  // The host has the controls of the setup: a counter is under a finger.
  const setup = page.getByRole("region", { name: "Setup" });
  await expect(setup.getByLabel("Players", { exact: true })).toHaveText("8");
  await expect(setup.getByLabel("Strategy cards", { exact: true })).toHaveValue("te");
  await setup.getByRole("button", { name: "Wait more" }).tap();
  await expect(setup.getByLabel("Bots wait", { exact: true })).toHaveText("0.5 s");
  await expect(main(page)).toBeInViewport();
  await fits(page);
  await page.screenshot({ path: "shots/phone/online/lobby-setup.png", animations: "disabled" });

  await pane(page, "Map").tap();
  await expect(page.locator("g.system.home")).toHaveCount(8);
  await expect(main(page)).toBeInViewport();
  await fits(page);
  await page.screenshot({ path: "shots/phone/online/lobby-map.png", animations: "disabled" });

  // A visitor has the name and "Join" in the footer, in reach on every pane.
  await page.addInitScript(
    (key) => localStorage.removeItem(key),
    `ti4.online.${made.game_id}.players`,
  );
  await page.goto(`/?online=${made.game_id}`);
  await expect(main(page)).toHaveText("Join");
  await expect(page.getByLabel("Name")).toBeInViewport();
  await expect(main(page)).toBeInViewport();
  await fits(page);
  await page.screenshot({ path: "shots/phone/online/lobby-visitor.png", animations: "disabled" });
});
