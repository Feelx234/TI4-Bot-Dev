import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  expect,
  test,
  type APIRequestContext,
  type BrowserContext,
  type Page,
} from "@playwright/test";

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
/** The value of a setup row: the number of a counter, or the list. */
const value = (page: Page, label: string) => setup(page).getByLabel(label, { exact: true });
/** The value of a setup row as words, for a viewer with no control there. */
const said = (page: Page, label: string) =>
  setup(page).locator("dt", { hasText: label }).locator("xpath=following-sibling::dd");
const note = (page: Page) => page.locator("#lobby-note");
const main = (page: Page) => page.locator('button[aria-keyshortcuts="Enter"]');
const title = (page: Page) => page.locator("header strong").first();
const toast = (page: Page, text: string) => page.getByRole("status").filter({ hasText: text });
/** The action panel of the running game, and its main button. */
const panel = (page: Page) => page.locator("#step-panel");
const send = (page: Page) => page.locator('footer button[aria-keyshortcuts="Enter"]');
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

/** The players that this browser has at a table: each tab is the one of its address. */
const asPlayers = (context: BrowserContext, game: string, players: Record<string, string>) =>
  context.addInitScript(
    ([key, value]) => localStorage.setItem(key, value),
    [`ti4.online.${game}.players`, JSON.stringify(players)],
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
  await expect(value(page, "Players")).toHaveText("4");
  await expect(value(page, "Strategy cards")).toHaveValue("te");
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
  // The table is full: nobody says "Ready", the host starts.
  await expect(note(page)).toHaveText("Waiting for Ada to start");
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
  // The board is asked over HTTP, and its answer is late: it comes when the socket is gone.
  let board = () => {};
  const held = new Promise<void>((release) => {
    board = release;
  });
  await page.route("**/lobby/map-preview", async (route) => {
    await held;
    await route.continue();
  });
  await page.goto(`/?online=${table.id}`);
  await expect(players(page)).toContainText("1 of 3");
  down = true;
  await Promise.all(open.map((socket) => socket.close()));
  // One signal: the lobby stays in view.
  await expect(page.locator("header").getByRole("status")).toHaveText("Offline · reconnecting");
  await expect(rows(page)).toHaveCount(3);
  // An answer of a call says nothing about the socket: the page is still offline.
  board();
  await expect(page.locator("g.system.home")).toHaveCount(3);
  await expect(page.locator("header").getByRole("status")).toHaveText("Offline · reconnecting");
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

test("the host sets up the table, and the other tab is told at once", async ({
  page,
  context,
  request,
}) => {
  const errors = errorsOf(page);
  const table = await makeTable(request, 3, "Ada");
  const blair = await table.call("join", { kind: "new", nickname: "Blair" }, null);
  await asPlayers(context, table.id, {
    [table.host.id]: table.host.session,
    [blair.player.id]: blair.player_session,
  });
  await page.goto(`/?online=${table.id}&seat=${table.host.id}`);
  const other = await context.newPage();
  await other.goto(`/?online=${table.id}&seat=${blair.player.id}`);
  await expect(title(other)).toHaveText("Lobby · 3 players");
  // The same screen for a player who is not the host, with the values and no controls.
  await expect(setup(other).getByRole("button", { name: /player/ })).toHaveCount(0);
  await expect(setup(other).getByRole("combobox")).toHaveCount(0);
  await expect(said(other, "Strategy cards")).toHaveText("Thunder's Edge");

  // The players, in steps of one: a change applies at once, for every tab.
  await setup(page).getByRole("button", { name: "One player more" }).click();
  await expect(value(page, "Players")).toHaveText("4");
  await expect(title(other)).toHaveText("Lobby · 4 players");
  await expect(rows(other)).toHaveCount(4);
  await expect(toast(other, "The host set the players to 4")).toBeVisible();
  await expect(page.locator("g.system.home")).toHaveCount(4);
  // The next table of this browser has as many seats.
  expect(await page.evaluate(() => localStorage.getItem("ti4.players"))).toBe("4");
  // A seat that the server plays gives its seat, an open seat first; a person does not.
  await players(page).getByRole("button", { name: "Fill with rule bots" }).click();
  await expect(players(page)).toContainText("4 of 4");
  await setup(page).getByRole("button", { name: "One player fewer" }).click();
  await expect(rows(other)).toHaveCount(3);
  await setup(page).getByRole("button", { name: "One player fewer" }).click();
  await expect(rows(other)).toHaveCount(2);
  const fewer = setup(page).getByRole("button", { name: /^One player fewer/ });
  await expect(fewer).toHaveAttribute("aria-disabled", "true");
  await expect(fewer).toHaveAttribute("title", "A table has 2 seats at least");

  // The set of strategy cards, from the list of the server.
  await value(page, "Strategy cards").selectOption({ label: "Prophecy of Kings" });
  await expect(said(other, "Strategy cards")).toHaveText("Prophecy of Kings");
  await expect(toast(other, "The host changed the strategy cards")).toBeVisible();
  // What a set changes is behind "?": no card is named on the screen.
  await expect(setup(page).getByText("Construction")).toHaveCount(0);
  await setup(page).getByRole("button", { name: "About Strategy cards" }).click();
  await expect(
    page
      .getByRole("note")
      .getByText(/Construction/)
      .first(),
  ).toBeVisible();
  await page.keyboard.press("Escape");

  // The wait of the seats that the server plays, in steps of half a second.
  await expect(value(page, "Bots wait")).toHaveText("Off");
  await setup(page).getByRole("button", { name: "Wait more" }).click();
  await expect(value(page, "Bots wait")).toHaveText("0.5 s");
  await expect(said(other, "Bots wait")).toHaveText("0.5 s");

  // The name is edited in the row of the viewer. Esc leaves the editor.
  await rows(page).nth(0).getByRole("button", { name: "Change your name" }).click();
  await page.getByLabel("Your name").fill("Nobody");
  await page.keyboard.press("Escape");
  await expect(rows(page).nth(0)).toContainText("Ada (you)");
  await rows(page).nth(0).getByRole("button", { name: "Change your name" }).click();
  await page.getByLabel("Your name").fill("Grace");
  await page.keyboard.press("Enter");
  await expect(rows(page).nth(0)).toContainText("Grace (you)");
  await expect(rows(other).nth(0)).toContainText("Grace");
  expect(await page.evaluate(() => localStorage.getItem("ti4.nickname"))).toBe("Grace");
  // Enter in the name was not the main button: the table is still a lobby.
  await expect(main(page)).toHaveText(/Start game/);
  // A row of another player is no control.
  await expect(rows(other).getByRole("button", { name: "Change your name" })).toHaveCount(1);
  await expect(rows(other).nth(1).getByRole("button", { name: "Change your name" })).toBeVisible();
  await page.screenshot({ path: "shots/online/lobby-setup.png", animations: "disabled" });
  expect(errors).toEqual([]);
});

test("a secondary is drafted in one tab while a rule bot takes its time, and the toolbar says who the game waits for", async ({
  page,
  context,
  request,
}) => {
  // Longer than a test of this suite should be: see "Tests" in AGENTS.md.
  test.setTimeout(10_000);
  const errors = errorsOf(page);
  const table = await makeTable(request, 4, "Ada");
  await asPlayers(context, table.id, { [table.host.id]: table.host.session });
  await page.goto(`/?online=${table.id}&seat=${table.host.id}`);
  // The host starts from the late game. It is a game of its own: eight seats, its map, its
  // cards and its factions, so these rows have no control.
  await value(page, "Start from").selectOption({ label: "Late game" });
  await expect(title(page)).toHaveText("Lobby · 8 players");
  await expect(said(page, "Players")).toHaveText("8");
  await expect(setup(page).getByRole("combobox")).toHaveCount(1);
  await expect(rows(page).nth(0)).toContainText("Sol");
  // Two rule bots, a second person, and four more rule bots: the seats are in this order.
  await table.call("bots", { kind: "smart", count: 2 });
  const blair = await table.call("join", { kind: "new", nickname: "Blair" }, null);
  await table.call("bots", { kind: "smart", count: 4 });
  await asPlayers(context, table.id, {
    [table.host.id]: table.host.session,
    [blair.player.id]: blair.player_session,
  });
  const other = await context.newPage();
  const otherErrors = errorsOf(other);
  await other.goto(`/?online=${table.id}&seat=${blair.player.id}`);
  await expect(said(other, "Start from")).toHaveText("Late game");
  // The seats that the server plays wait a second before each answer.
  await setup(page).getByRole("button", { name: "Wait more" }).click();
  await setup(page).getByRole("button", { name: "Wait more" }).click();
  await expect(value(page, "Bots wait")).toHaveText("1 s");
  await expect(said(other, "Bots wait")).toHaveText("1 s");
  await expect(toast(other, "The host set the wait of the bots to 1 s")).toBeVisible();
  await page.keyboard.press("Enter");
  for (const tab of [page, other]) {
    await expect(tab.getByText("Round 4 · Action phase")).toBeVisible();
  }

  // The host has the turn, as the first seat of the scenario, and plays Leadership.
  await panel(page)
    .getByRole("button", { name: /Take your strategic action/ })
    .click();
  await send(page).click();
  // The other tab drafts its secondary from the moment the card is played.
  await expect(panel(other).getByRole("heading", { name: "Leadership" })).toBeVisible();
  await expect(panel(other).getByText("✎ Private draft")).toBeVisible();
  await expect(title(other)).toHaveText("Waiting for Sol");
  for (const answer of ["Tactic pool", "Tactic pool", "Tactic pool", "Spend nothing further"]) {
    await panel(page).getByRole("button", { name: answer }).click();
    await send(page).click();
  }
  // The first seat after the host is a rule bot. It takes a second for each of its answers, and
  // every tab reads who the game waits for.
  for (const tab of [page, other]) {
    await expect(title(tab)).toHaveText("Waiting for Hacan");
    await expect(panel(tab).getByText("▶ Deciding")).toHaveCount(1);
  }
  // Meanwhile the draft is made and marked ready. With no wait the game would be past this
  // seat before the first click.
  await panel(other).getByRole("button", { name: "Spend nothing further" }).click();
  await expect(send(other)).toHaveText(/^Record/);
  await send(other).click();
  await expect(send(other)).toHaveText(/^Mark draft ready/);
  await send(other).click();
  await expect(panel(other).getByText("✎ Draft ready")).toBeVisible();
  await expect(title(other)).toHaveText("Waiting for Hacan");
  // The draft is private: the host sees the row of that seat wait, and nothing of the draft.
  await expect(panel(page).getByText("✎")).toHaveCount(0);
  await other.screenshot({ path: "shots/online/secondary-draft-wait.png", animations: "disabled" });
  expect([...errors, ...otherErrors]).toEqual([]);
});

test("a table starts from the late game, and each tab has the seat of its join", async ({
  page,
  context,
  request,
}) => {
  const errors = errorsOf(page);
  const table = await makeTable(request, 4, "Ada");
  await table.call("settings", { start_from: "late-game" });
  const blair = await table.call("join", { kind: "new", nickname: "Blair" }, null);
  await table.call("bots", { kind: "smart", fill: true });
  await asPlayers(context, table.id, {
    [table.host.id]: table.host.session,
    [blair.player.id]: blair.player_session,
  });
  await page.goto(`/?online=${table.id}&seat=${table.host.id}`);
  const other = await context.newPage();
  await other.goto(`/?online=${table.id}&seat=${blair.player.id}`);
  // Every player is told where the game starts, and the seats have the factions of the scenario.
  await expect(said(other, "Start from")).toHaveText("Late game");
  await expect(rows(other).nth(0)).toContainText("Sol");
  await expect(rows(other).nth(1)).toContainText("Blair (you)");
  await expect(rows(other).nth(1)).toContainText("Hacan");
  await expect(rows(other).filter({ hasText: "Random" })).toHaveCount(0);
  await page.keyboard.press("Enter");
  for (const tab of [page, other]) {
    await expect(tab.getByText("Round 4 · Action phase")).toBeVisible();
    await expect(tab.getByRole("region", { name: "Players" }).locator("tbody tr")).toHaveCount(8);
  }
  // The seats are not drawn: the host is the first seat, the second person the second.
  await expect(ownSeat(page)).toHaveAttribute("aria-label", /Sol/);
  await expect(ownSeat(other)).toHaveAttribute("aria-label", /Hacan/);
  await expect(title(page)).toHaveText("Your decision");
  await expect(title(other)).toHaveText("Waiting for Sol");
  // A sheet of the late game in each tab: each seat has its own hand of the scenario.
  for (const tab of [page, other]) {
    await tab.getByRole("button", { name: "Secrets", exact: true }).click();
    const mine = tab
      .getByRole("complementary", { name: "Secrets" })
      .getByRole("region", { name: "Your secret objectives" });
    await expect(mine.getByRole("article")).toHaveCount(3);
  }
  await page.getByRole("button", { name: "Objectives", exact: true }).click();
  await expect(page.getByRole("complementary", { name: "Objectives" })).toContainText(
    "10 VP to win",
  );
  expect(errors).toEqual([]);
});
