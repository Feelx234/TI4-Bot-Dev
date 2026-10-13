import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";

// The start screen and the lobby of a table, against a real server (`crates/ti4-server`,
// started by playwright.config.ts) through the proxy of vite. A table is made, filled and
// started from the page, and several tabs of one browser are several seats of its game.
const WASM = fileURLToPath(new URL("../src/session/ti4.wasm", import.meta.url));
test.skip(!existsSync(WASM), "the engine is not built: run web2/scripts/build-wasm.sh");
test.use({ viewport: { width: 1920, height: 1080 } });

/** The two long names of the demo data: a name is cut, never the row. */
const LONG = [
  "Bartholomew Featherstonehaugh the Third of Mecatol",
  "Wilhelmina Vanderbilt-Montgomery",
];

const players = (page: Page) => page.getByRole("region", { name: "Players" });
const rows = (page: Page) => players(page).getByRole("listitem");
const setup = (page: Page) => page.getByRole("region", { name: "Setup" });
const note = (page: Page) => page.locator("#lobby-note");
const main = (page: Page) => page.locator('button[aria-keyshortcuts="Enter"]');
const title = (page: Page) => page.locator("header strong").first();
/** The seat of the page in the running game: the row of the player table that is marked as its own. */
const ownSeat = (page: Page) =>
  page.getByRole("region", { name: "Players" }).locator('tbody tr[class*="bg-cyan"] th button');
/** A seat of a game is named by the id of its player, until the game knows the nicknames. */
const seatName = (player: string | null) => new RegExp(`^Seat ${player!.toUpperCase()}, `);

/** The errors of the page. A test expects none. */
function errorsOf(page: Page) {
  const errors: string[] = [];
  page.on("console", (message) => message.type() === "error" && errors.push(message.text()));
  page.on("pageerror", (error) => errors.push(error.message));
  return errors;
}

/** A table that is made through the calls of the lobby, with its host. */
async function makeTable(request: APIRequestContext, count: number, nickname: string) {
  const response = await request.post("/api/games", {
    data: { player_count: count, seed: 11, nickname },
  });
  expect(response.ok()).toBe(true);
  const made = await response.json();
  return {
    id: made.game_id as string,
    host: { id: made.player.id as string, session: made.player_session as string },
    async call(path: string, data: object, session: string | null = made.player_session) {
      const answer = await request.post(`/api/games/${made.game_id}/lobby/${path}`, {
        data,
        headers: session ? { "x-ti4-player-session": session } : {},
      });
      expect(answer.ok(), `${path}: ${await answer.text()}`).toBe(true);
      return answer.json();
    },
  };
}

/** The page is the player `id` of the table: what the browser keeps of a player. */
const asPlayer = (page: Page, game: string, id: string, session: string) =>
  page.addInitScript(
    ([key, value]) => localStorage.setItem(key, value),
    [`ti4.online.${game}.players`, JSON.stringify({ [id]: session })],
  );

test("three tabs of one browser make a table from the start screen, fill it and play its game", async ({
  page,
  context,
}) => {
  // Longer than a test of this suite should be: see "Tests" in AGENTS.md.
  test.setTimeout(10_000);
  const errors = errorsOf(page);
  // The start screen: the name, then "Multiplayer", which makes a table of four.
  await page.goto("/");
  await page.getByLabel("Name").fill("Ada");
  const asked = page.waitForRequest(
    (request) => request.method() === "POST" && new URL(request.url()).pathname === "/api/games",
  );
  await page.getByRole("link", { name: /Multiplayer/ }).click();
  expect((await asked).postDataJSON()).toEqual({ player_count: 4, nickname: "Ada" });
  await expect(page).toHaveURL(/\?online=[^&]+&seat=[^&]+$/);
  const game = new URL(page.url()).searchParams.get("online")!;

  // The lobby: the host, three open seats, and a start that waits for them.
  await expect(title(page)).toHaveText("Lobby · 4 players");
  await expect(rows(page)).toHaveCount(4);
  await expect(rows(page).nth(0)).toContainText("Ada (you)");
  await expect(rows(page).nth(0).getByRole("img", { name: "Host" })).toBeVisible();
  await expect(rows(page).nth(0)).toContainText("Random");
  await expect(rows(page).nth(1).getByRole("img", { name: "Open seat" })).toBeVisible();
  await expect(players(page)).toContainText("1 of 4");
  await expect(setup(page)).toContainText("Players4");
  await expect(setup(page)).toContainText("Strategy cardsThunder's Edge");
  await expect(note(page)).toHaveText("Waiting for 3 open seats");
  await expect(main(page)).toHaveText(/Start game/);
  await expect(main(page)).toBeDisabled();
  // The board is the map that the table will play, with the places of the four seats.
  await expect(page.locator("g.system")).not.toHaveCount(0);
  await expect(page.locator("g.system.home")).toHaveCount(4);
  // What the kinds of a seat do is behind "?", not on the screen.
  await expect(page.getByText("The server plays this seat by rules")).toHaveCount(0);
  await players(page).getByRole("button", { name: "About seats that the server plays" }).click();
  await expect(page.getByText("The server plays this seat by rules")).toBeVisible();
  await page.keyboard.press("Escape");
  await page.screenshot({ path: "shots/online/lobby-host.png", animations: "disabled" });

  // A second and a third tab join as other players of this browser.
  const tabs: Page[] = [];
  for (const name of ["Ada 2", "Ada 3"]) {
    const opened = context.waitForEvent("page");
    await players(page).getByRole("button", { name: "Join in a new tab" }).click();
    const tab = await opened;
    await tab.waitForURL(/\?online=[^&]+&seat=[^&]+$/);
    await expect(rows(tab).filter({ hasText: "(you)" })).toContainText(name);
    // The first tab sees the player who joined, with no reload and no wait for a poll.
    await expect(rows(page).filter({ hasText: name })).toHaveCount(1, { timeout: 2_000 });
    tabs.push(tab);
  }
  const [second, third] = tabs;
  const seats = [page, second, third].map((tab) => new URL(tab.url()).searchParams.get("seat"));
  expect(new Set(seats).size).toBe(3);
  await expect(players(page)).toContainText("3 of 4");
  // A player who is not the host has the same screen, with no controls of the table.
  await expect(rows(second)).toHaveCount(4);
  await expect(main(second)).toHaveCount(0);
  await expect(note(second)).toHaveText("Waiting for 1 open seat");
  await expect(players(second).getByRole("button", { name: "Rule bot" })).toHaveCount(0);
  await expect(players(second).getByRole("button", { name: /^Remove / })).toHaveCount(0);
  // The other tabs of this browser are marked for the host: "(yours)".
  await expect(rows(page).filter({ hasText: "(yours)" })).toHaveCount(2);

  // A reload of a tab keeps its seat.
  await second.reload();
  await expect(rows(second).filter({ hasText: "(you)" })).toContainText("Ada 2");
  expect(new URL(second.url()).searchParams.get("seat")).toBe(seats[1]);

  // The host lets the server play the last seat, and every tab sees it at once.
  await players(page).getByRole("button", { name: "Rule bot", exact: true }).click();
  for (const tab of [page, second, third]) {
    await expect(rows(tab).nth(3)).toContainText("Rule bot 1");
    await expect(
      rows(tab)
        .nth(3)
        .getByRole("img", { name: /^Rule bot/ }),
    ).toBeVisible();
    await expect(players(tab)).toContainText("4 of 4");
  }
  await expect(note(second)).toHaveText("Waiting for Ada to start");
  // A seat that the server plays can be removed again, and added again.
  await players(page).getByRole("button", { name: "Remove Rule bot 1" }).click();
  await expect(players(second)).toContainText("3 of 4");
  await players(page).getByRole("button", { name: "Fill with rule bots" }).click();
  await expect(players(third)).toContainText("4 of 4");
  await expect(note(page)).toHaveText("");
  await expect(main(page)).toBeEnabled();
  await page.screenshot({ path: "shots/online/lobby-full.png", animations: "disabled" });

  // Enter is the main button: the game starts, and every tab mounts it with its player.
  await page.keyboard.press("Enter");
  for (const [index, tab] of [page, second, third].entries()) {
    await expect(tab.getByText("Round 1 · Strategy phase")).toBeVisible();
    await expect(tab.getByRole("region", { name: "Players" }).locator("tbody tr")).toHaveCount(4);
    // Each tab is another seat of the same game: the seat of its address.
    await expect(ownSeat(tab)).toHaveAttribute("aria-label", seatName(seats[index]));
    expect(new URL(tab.url()).searchParams.get("online")).toBe(game);
    expect(new URL(tab.url()).searchParams.get("seat")).toBe(seats[index]);
  }
  // A reload of a tab of the running game is the same seat.
  await third.reload();
  await expect(third.getByText("Round 1 · Strategy phase")).toBeVisible();
  await expect(ownSeat(third)).toHaveAttribute("aria-label", seatName(seats[2]));
  expect(errors).toEqual([]);
});

test("a visitor of another browser sees the table, joins with a name and is told what changes", async ({
  page,
  request,
}) => {
  const errors = errorsOf(page);
  const table = await makeTable(request, 3, "Ada");
  // The link of the table has no seat: this browser has no player there, so it is a visitor.
  await page.goto(`/?online=${table.id}`);
  await expect(title(page)).toHaveText("Lobby · 3 players");
  await expect(rows(page).nth(0)).toContainText("Ada");
  await expect(rows(page).filter({ hasText: "(you)" })).toHaveCount(0);
  await expect(note(page)).toHaveText("2 open seats");
  await expect(players(page).getByRole("button")).toHaveCount(1); // only "?"
  // The host has no socket open: one sign says so.
  await expect(rows(page).nth(0).getByRole("img", { name: "Host · not connected" })).toBeVisible();
  await page.screenshot({ path: "shots/online/lobby-visitor.png", animations: "disabled" });

  // Somebody else joins: the visitor sees it with no reload.
  await table.call("join", { kind: "new", nickname: "Blair" }, null);
  await expect(rows(page).nth(1)).toContainText("Blair");
  await expect(note(page)).toHaveText("1 open seat");

  // The visitor joins. A name is needed; Enter in the field is the main button.
  await expect(main(page)).toHaveText(/Join/);
  await page.getByLabel("Name").fill("");
  await expect(main(page)).toBeDisabled();
  await page.getByLabel("Name").fill("Cy");
  await page.getByLabel("Name").press("Enter");
  await expect(page).toHaveURL(/&seat=[^&]+$/);
  await expect(rows(page).nth(2)).toContainText("Cy (you)");
  // The table is full. Its other players came through the calls of the lobby and have no page
  // that says "Ready" for them, as a player of `web/` who has not pressed the button.
  await expect(note(page)).toHaveText("Waiting for Ada to be ready");
  await expect(main(page)).toHaveCount(0);
  expect(await page.evaluate(() => localStorage.getItem("ti4.nickname"))).toBe("Cy");

  // The host removes the player: the tab is a visitor again, and the browser forgets the seat.
  const seat = new URL(page.url()).searchParams.get("seat")!;
  await table.call("remove-bot", { player_id: seat });
  await expect(page).toHaveURL(new RegExp(`\\?online=${table.id}$`));
  await expect(note(page)).toHaveText("1 open seat");
  await expect(main(page)).toHaveText(/Join/);
  expect(
    await page.evaluate((key) => localStorage.getItem(key), `ti4.online.${table.id}.players`),
  ).toBe("{}");
  expect(errors).toEqual([]);
});

test("a tab with no seat is the one player of this browser, or asks which of several", async ({
  page,
  request,
}) => {
  const table = await makeTable(request, 4, "Ada");
  const joined = await table.call("join", { kind: "new", nickname: "Ada 2" }, null);
  // One player of this browser: the tab is that player, and the address gets its seat.
  await asPlayer(page, table.id, table.host.id, table.host.session);
  await page.goto(`/?online=${table.id}`);
  await expect(page).toHaveURL(new RegExp(`&seat=${table.host.id}$`));
  await expect(rows(page).nth(0)).toContainText("Ada (you)");

  // Two players of this browser: the list marks them, and a row makes the tab that player.
  await page.addInitScript(
    ([key, value]) => localStorage.setItem(key, value),
    [
      `ti4.online.${table.id}.players`,
      JSON.stringify({
        [table.host.id]: table.host.session,
        [joined.player.id]: joined.player_session,
      }),
    ],
  );
  await page.goto(`/?online=${table.id}`);
  await expect(rows(page).filter({ hasText: "(yours)" })).toHaveCount(2);
  await expect(rows(page).filter({ hasText: "(you)" })).toHaveCount(0);
  await expect(main(page)).toHaveText(/Join/);
  await rows(page).nth(1).getByRole("button", { name: /Ada 2/ }).click();
  await expect(page).toHaveURL(new RegExp(`&seat=${joined.player.id}$`));
  await expect(rows(page).nth(1)).toContainText("Ada 2 (you)");
  await expect(rows(page).nth(0)).toContainText("Ada (yours)");

  // A seat of the address that this browser does not have is no seat.
  await page.goto(`/?online=${table.id}&seat=somebody_else`);
  await expect(rows(page).filter({ hasText: "(yours)" })).toHaveCount(2);
  await expect(page).toHaveURL(new RegExp(`\\?online=${table.id}$`));
});

test("a call that the server refuses is a toast, and the lobby stays as it was", async ({
  page,
  request,
}) => {
  const table = await makeTable(request, 2, "Ada");
  await asPlayer(page, table.id, table.host.id, table.host.session);
  await page.goto(`/?online=${table.id}`);
  await expect(players(page)).toContainText("1 of 2");
  // The seat is taken behind the back of the page, and the answer of the server is late.
  await page.route("**/lobby/bots", async (route) => {
    await table.call("join", { kind: "new", nickname: "Blair" }, null);
    await route.continue();
  });
  await players(page).getByRole("button", { name: "Random", exact: true }).click();
  await expect(page.getByRole("status").filter({ hasText: "Seat is unavailable" })).toBeVisible();
  await expect(rows(page).nth(1)).toContainText("Blair");
  await expect(players(page)).toContainText("2 of 2");
});

test("a lost connection of the lobby is said in the header, and the table comes back by itself", async ({
  page,
  request,
}) => {
  const errors = errorsOf(page);
  const table = await makeTable(request, 3, "Ada");
  await asPlayer(page, table.id, table.host.id, table.host.session);
  let down = false;
  const open: { close(): Promise<void> }[] = [];
  await page.routeWebSocket(/\/ws\/tables\//, (socket) => {
    if (down) {
      return void socket.close();
    }
    socket.connectToServer();
    open.push(socket);
  });
  await page.goto(`/?online=${table.id}`);
  await expect(players(page)).toContainText("1 of 3");
  down = true;
  await Promise.all(open.map((socket) => socket.close()));
  // One signal: the lobby stays in view.
  await expect(page.locator("header").getByRole("status")).toHaveText("Offline · reconnecting");
  await expect(rows(page)).toHaveCount(3);
  // What changed meanwhile is there when the page is back.
  await table.call("bots", { kind: "random" });
  down = false;
  await expect(players(page)).toContainText("2 of 3");
  await expect(page.locator("header").getByRole("status")).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("the lobby of eight seats with the long names fits the screen", async ({ page, request }) => {
  const table = await makeTable(request, 8, LONG[0]);
  await table.call("join", { kind: "new", nickname: LONG[1] }, null);
  await table.call("bots", { kind: "smart", count: 3 });
  await table.call("bots", { kind: "random", count: 2 });
  await asPlayer(page, table.id, table.host.id, table.host.session);
  await page.goto(`/?online=${table.id}`);
  await expect(rows(page)).toHaveCount(8);
  await expect(page.locator("g.system.home")).toHaveCount(8);
  // The page does not scroll, and no row is wider than the panel.
  const size = await page.evaluate(() => ({
    width: document.documentElement.scrollWidth,
    height: document.documentElement.scrollHeight,
  }));
  expect(size).toEqual({ width: 1920, height: 1080 });
  const list = players(page).getByRole("list");
  expect(await list.evaluate((node) => node.scrollHeight <= node.clientHeight)).toBe(true);
  expect(await list.evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
  // A long name is cut, and the full name is on hover.
  const name = rows(page).nth(0).getByTitle(LONG[0]);
  await expect(name).toContainText("(you)");
  expect(await name.evaluate((node) => node.scrollWidth > node.clientWidth)).toBe(true);
  await expect(main(page)).toBeInViewport();
  await page.screenshot({ path: "shots/online/lobby-eight.png", animations: "disabled" });
});
