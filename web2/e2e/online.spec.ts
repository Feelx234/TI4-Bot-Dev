import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";

// A game on a real server (`crates/ti4-server`, started by playwright.config.ts), through the
// proxy of vite: the page has the first seat of eight, the other seats decide at random. The
// printed cards are those of the engine of the page: `scripts/build-wasm.sh`.
const WASM = fileURLToPath(new URL("../src/session/ti4.wasm", import.meta.url));
test.skip(!existsSync(WASM), "the engine is not built: run web2/scripts/build-wasm.sh");
test.use({ viewport: { width: 1920, height: 1080 } });
test.setTimeout(120_000);

const panel = (page: Page) => page.locator("#step-panel");
const send = (page: Page) => page.locator('footer button[aria-keyshortcuts="Enter"]');
const heading = (page: Page) => panel(page).getByRole("heading").first();
const row = (page: Page, name: string) => panel(page).getByRole("button", { name });
const system = (page: Page, id: string) => page.locator("g.system").filter({ hasText: `#${id}` });
const workspace = (page: Page, name: string) =>
  page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name });
const status = (page: Page) => page.locator("header strong").first();

/** The errors of the page. A test expects none. */
function errorsOf(page: Page) {
  const errors: string[] = [];
  page.on("console", (message) => message.type() === "error" && errors.push(message.text()));
  page.on("pageerror", (error) => errors.push(error.message));
  return errors;
}

test("a seeded game on the server: the first turn, with a draft, the sheets and a reload", async ({
  page,
  request,
}) => {
  const errors = errorsOf(page);
  // The game is made as the page makes it, with a seed, so that the board is the same each run.
  const response = await request.post("/api/games", {
    data: { player_count: 8, seed: 3, nickname: "Ada", fill_seats: "random" },
  });
  expect(response.ok()).toBe(true);
  const made = await response.json();
  await page.addInitScript(
    ([key, value]) => localStorage.setItem(key, value),
    [`ti4.online.${made.game_id}.session`, made.player_session],
  );
  await page.goto(`/?online=${made.game_id}`);
  await expect(page.getByText("Round 1 · Strategy phase")).toBeVisible();
  await expect(page.getByRole("region", { name: "Players" }).locator("tbody tr")).toHaveCount(8);
  await expect(heading(page)).toHaveText("Choose a strategy card");
  // The tools of a local game are not there, and the header leads back to it.
  await expect(page.getByRole("button", { name: "Dev", exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Undo" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Local game" })).toBeVisible();

  // The seat answers what it is asked, up to the menu of its first turn: the other seats play.
  for (let count = 0; ; count++) {
    expect(count).toBeLessThan(20);
    await expect(status(page)).toHaveText("Your decision");
    if (await row(page, "Take a tactical action").count()) {
      break;
    }
    await panel(page).getByRole("button", { pressed: false }).first().click();
    await send(page).click();
    await expect(status(page)).not.toHaveText("Your decision");
  }
  await expect(page.getByText("Round 1 · Action phase")).toBeVisible();

  // The draft of the turn is played by the server, on a copy: each change is one request.
  await workspace(page, "Draft").click();
  await expect(heading(page)).toHaveText("Activation");
  await expect(page.getByText("Private draft").first()).toBeVisible();
  await system(page, "23").click();
  await page.keyboard.press("Enter");
  await expect(heading(page)).toHaveText("Movement");
  await system(page, "01").click();
  const sheet = page.getByRole("group", { name: /^Move from Jord/ });
  await sheet.getByRole("button", { name: "Carrier 1 of 2" }).click();
  await sheet.getByRole("button", { name: "Destroyer 1 of 1" }).click();
  await expect(sheet.getByText(/· 2 ships$/)).toBeVisible();
  await expect(system(page, "23")).toHaveAttribute("aria-label", /2 Sol ships/);
  await expect(page.locator("footer").getByText("The action ends here.")).toBeVisible();
  await page.screenshot({ path: "shots/online/draft-movement.png", animations: "disabled" });

  // A reload resumes the game as the same player, and the draft is kept for this game.
  await page.reload();
  await expect(row(page, "Take a tactical action")).toBeVisible();
  await expect(system(page, "23")).not.toHaveAttribute("aria-label", /Sol ship/);
  await workspace(page, "Draft").click();
  await expect(heading(page)).toHaveText("Movement");
  await expect(system(page, "23")).toHaveAttribute("aria-label", /2 Sol ships/);

  // "Apply to Live" sends the action, the system and the movement as one request.
  const apply = page.getByRole("button", { name: "Apply to Live" });
  await expect(apply).toBeEnabled();
  await apply.click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText(/^Activate .+ · #23$/)).toBeVisible();
  await dialog.getByRole("button", { name: "Apply to Live" }).click();
  await expect(workspace(page, "Live")).toHaveAttribute("aria-pressed", "true");
  await expect(system(page, "23")).toHaveAttribute("aria-label", /2 Sol ships/);
  await expect(heading(page)).not.toHaveText("Movement");
  await page.screenshot({ path: "shots/online/after-draft.png", animations: "disabled" });

  // The sheets have the facts that the server sends, as in a local game.
  await page.getByRole("button", { name: "Cards", exact: true }).click();
  const cards = page.getByRole("complementary", { name: "Cards" });
  await expect(cards.getByText(/^Hand \d+ \/ 7$/)).toBeVisible();
  await expect(cards.getByText(/of 122 cards left/)).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Secrets", exact: true }).click();
  const secrets = page.getByRole("complementary", { name: "Secrets" });
  await expect(secrets.getByText(/^\d scored · \d \/ 3$/)).toBeVisible();
  await expect(secrets.getByText(/^\d+ of 40 unseen · \d+ in the deck$/)).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Objectives", exact: true }).click();
  await expect(
    page.getByRole("complementary", { name: "Objectives" }).getByText("10 VP to win"),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Agendas", exact: true }).click();
  await expect(
    page.getByRole("complementary", { name: "Agendas" }).getByText(/in the deck$/),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  expect(errors).toEqual([]);
});

test("?online starts a game on the server and goes to its address", async ({ page }) => {
  const errors = errorsOf(page);
  await page.goto("/?online");
  await expect(page).toHaveURL(/\?online=.+/);
  await expect(page.getByText("Round 1 · Strategy phase")).toBeVisible();
  await expect(page.getByRole("region", { name: "Players" }).locator("tbody tr")).toHaveCount(8);
  // The page plays a seat: it is asked, or it waits for a seat before it.
  await expect(status(page)).toHaveText(/^(Your decision|Waiting for .+|Turn of .+)$/);
  // The player of this browser is kept for the game, so the address is the game.
  const id = new URL(page.url()).searchParams.get("online");
  expect(
    await page.evaluate((key) => localStorage.getItem(key), `ti4.online.${id}.session`),
  ).toBeTruthy();
  expect(errors).toEqual([]);
});

test("a lost connection is said in the toolbar, and the game comes back by itself", async ({
  page,
}) => {
  const errors = errorsOf(page);
  // The sockets of the page go through the test, which can cut them and refuse new ones.
  let down = false;
  const open: { close(): Promise<void> }[] = [];
  await page.routeWebSocket(/\/ws\/games\//, (socket) => {
    if (down) {
      return void socket.close();
    }
    socket.connectToServer();
    open.push(socket);
  });
  await page.goto("/?online");
  await expect(page.getByText("Round 1 · Strategy phase")).toBeVisible();
  const before = await status(page).textContent();

  down = true;
  await Promise.all(open.map((socket) => socket.close()));
  // One signal, and no error: the game stays in view, and nothing can be sent.
  await expect(status(page)).toHaveText("Offline · reconnecting");
  await expect(page.getByRole("region", { name: "Players" }).locator("tbody tr")).toHaveCount(8);
  await expect(send(page)).toHaveCount(0);
  await expect(panel(page).getByText("The game cannot be reached.")).toBeVisible();
  await page.screenshot({ path: "shots/online/offline.png", animations: "disabled" });

  // The page tries again by itself, and the snapshot after it is the game as it is.
  down = false;
  await expect(status(page)).toHaveText(before ?? "", { timeout: 20_000 });
  expect(errors).toEqual([]);
});
