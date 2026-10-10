import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";

// A real game of the wasm engine in the shell: `?local=<seed>&players=8&humans=<mask>`.
// Needs the engine: `scripts/build-wasm.sh`. Screenshots go to shots/local/.
const WASM = fileURLToPath(new URL("../src/session/ti4.wasm", import.meta.url));
test.skip(!existsSync(WASM), "the engine is not built: run web2/scripts/build-wasm.sh");
test.use({ viewport: { width: 1920, height: 1080 } });
// The engine plays the other seats between two choices, which takes a moment.
test.setTimeout(120_000);

const panel = (page: Page) => page.locator("#step-panel");
/** The main button of the footer: "Send", "Activate system", "Move fleet". Enter does what it says. */
const send = (page: Page) => page.locator('footer button[aria-keyshortcuts="Enter"]');
const heading = (page: Page) => panel(page).getByRole("heading").first();
const settings = (page: Page) => page.getByRole("complementary", { name: "Dev" });
const undo = (page: Page) => page.getByRole("button", { name: "Undo" });

/** Opens the Dev sheet from the header. */
async function openSettings(page: Page) {
  await page.getByRole("button", { name: "Dev", exact: true }).click();
  await expect(settings(page)).toBeVisible();
}

/** The line of the Dev sheet ("Local game · seed 42 · 8 seats · 4 answers saved"). Read, then closed. */
async function savedLine(page: Page) {
  await openSettings(page);
  const line =
    (await settings(page)
      .getByText(/^Local game · seed/)
      .textContent()) ?? "";
  await page.keyboard.press("Escape");
  await expect(settings(page)).toBeHidden();
  return line;
}

/**
 * Answers the open choice: with the first system when it is on the board, with nothing when the
 * main button already has something to send (a movement of no ship), or else with its first row.
 */
async function answer(page: Page) {
  const saved = await savedLine(page);
  const system = page.locator("g.system.target");
  if (await system.count()) {
    await system.first().click();
  } else if (await send(page).isDisabled()) {
    await panel(page).getByRole("button", { pressed: false }).first().click();
  }
  await expect(send(page)).toBeEnabled();
  await send(page).click();
  // The answer is saved, and the next choice is open.
  await expect.poll(() => savedLine(page)).not.toBe(saved);
  await expect(send(page)).toBeVisible();
}

test("a game of eight is played through the decision list", async ({ page }) => {
  const errors: string[] = [];
  page.on("console", (message) => message.type() === "error" && errors.push(message.text()));
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/?local=3&players=8&humans=1");
  await expect(send(page)).toBeDisabled();
  await expect(page.locator("g.system")).toHaveCount(55);
  await expect(page.getByRole("region", { name: "Players" }).locator("tbody tr")).toHaveCount(8);
  await expect(page.getByText("Round 1 · Strategy phase")).toBeVisible();
  await expect(heading(page)).toHaveText("Choose a strategy card");
  await page.screenshot({ path: "shots/local/first-choice.png", animations: "disabled" });

  // Enter does what the main button says, and a number key stages a row.
  await page.keyboard.press("2");
  await expect(panel(page).getByRole("button", { pressed: true })).toHaveCount(1);
  await expect(page.getByText("Not sent")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(panel(page).getByRole("button", { pressed: true })).toHaveCount(0);
  await page.keyboard.press("2");
  await page.keyboard.press("Enter");
  await expect(send(page)).toBeDisabled();

  for (let count = 1; count < 30; count++) {
    await answer(page);
  }
  await expect(page.getByText("Round 1 · Strategy phase")).toHaveCount(0);
  await page.screenshot({ path: "shots/local/after-30-choices.png", animations: "disabled" });
  expect(errors).toEqual([]);
});

test("a tactical action: the system and the movement are staged on the map", async ({ page }) => {
  const errors: string[] = [];
  page.on("console", (message) => message.type() === "error" && errors.push(message.text()));
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/?local=3&players=8&humans=1");
  await expect(send(page)).toBeDisabled();
  const row = (name: string) => panel(page).getByRole("button", { name });
  await row("8. Imperial").click();
  await send(page).click();
  await row("Decline").click();
  await send(page).click();
  await row("Take a tactical action").click();
  await send(page).click();

  // Activation: the board is the picker, and the panel has the choice, not a list of systems.
  await expect(heading(page)).toHaveText("Activation");
  await expect(send(page)).toHaveText(/Activate system/);
  await expect(send(page)).toBeDisabled();
  await expect(panel(page).getByRole("button", { name: /^Activate \d/ })).toHaveCount(0);
  const system = (id: string) => page.locator("g.system").filter({ hasText: `#${id}` });
  // Every system can be activated. Only those where the action does something stand out: the
  // 12 that the ships reach, and home with its space dock.
  await expect(page.locator("g.system.target")).toHaveCount(54);
  await expect(page.locator("g.system.target:not(.plain)")).toHaveCount(13);
  await expect(system("01")).not.toHaveClass(/plain/);
  await page.screenshot({ path: "shots/local/activation-open.png", animations: "disabled" });
  await system("01").click();
  await expect(panel(page).getByText("Production 6")).toBeVisible();
  await system("23").click();
  await expect(page.locator("g.system.chosen")).toHaveCount(1);
  await expect(panel(page).getByText("3 ships in range")).toBeVisible();
  await expect(panel(page).getByText("From 1 system")).toBeVisible();
  await page.screenshot({ path: "shots/local/activation.png", animations: "disabled" });
  await page.keyboard.press("Enter");

  // Movement: nothing is staged, and moving nothing is what the main button says.
  await expect(heading(page)).toHaveText("Movement");
  await expect(send(page)).toHaveText(/Move nothing/);
  await expect.poll(() => savedLine(page)).toContain("4 answers saved");
  // The system with the ships opens its fleet on the map.
  await system("01").click();
  const sheet = page.getByRole("group", { name: /^Move from Jord/ });
  await expect(sheet).toBeVisible();
  await sheet.getByRole("button", { name: "Carrier 1 of 2" }).click();
  await sheet.getByRole("button", { name: /^Load Infantry, Planet Jord, 5 left/ }).click();
  await sheet.getByRole("button", { name: "Load Infantry on Carrier 1 of 2" }).first().click();
  await sheet.getByRole("button", { name: "Load Infantry on Carrier 1 of 2" }).first().click();
  await sheet.getByRole("button", { name: "Destroyer 1 of 1" }).click();
  await expect(sheet.getByText("2 ships · 2 cargo")).toBeVisible();
  await expect(
    sheet.getByRole("button", { name: /^Load Infantry, Planet Jord, 3 left/ }),
  ).toBeVisible();
  await expect(panel(page).getByText("Fleet supply")).toBeVisible();
  await expect(send(page)).toHaveText(/Move fleet/);
  // The board shows what the movement leaves and what arrives.
  await expect(system("01")).toHaveAttribute("aria-label", /handled, 4 Sol ships/);
  await expect(system("23")).toHaveAttribute("aria-label", /2 Sol ships/);
  await page.screenshot({ path: "shots/local/movement.png", animations: "disabled" });
  // Nothing was sent yet.
  await expect.poll(() => savedLine(page)).toContain("4 answers saved");
  // A reload keeps what is staged, and with it the mark of the system.
  await page.reload();
  await expect(send(page)).toHaveText(/Move fleet/);
  await expect(system("01")).toHaveAttribute("aria-label", /handled, 4 Sol ships/);
  await system("01").click();
  await expect(sheet.getByText("2 ships · 2 cargo")).toBeVisible();

  // "Move fleet" sends the whole movement: each ship and each unit is one answer of the game.
  await send(page).click();
  await expect.poll(() => savedLine(page)).toContain("10 answers saved");
  await expect(heading(page)).not.toHaveText("Movement");
  await expect(system("23")).toHaveAttribute("aria-label", /2 Sol ships/);
  await expect(system("01")).toHaveAttribute("aria-label", /4 Sol ships/);
  await page.screenshot({ path: "shots/local/after-movement.png", animations: "disabled" });

  // One undo takes the movement back whole, and it is staged again as it was sent.
  await undo(page).click();
  await expect(heading(page)).toHaveText("Movement");
  await expect.poll(() => savedLine(page)).toContain("4 answers saved");
  await expect(send(page)).toHaveText(/Move fleet/);
  await expect(system("01")).toHaveAttribute("aria-label", /handled, 4 Sol ships/);
  // The sheet of the system is still open, where the player left it.
  await expect(sheet.getByText("2 ships · 2 cargo")).toBeVisible();
  // So one part of it can change: the destroyer stays, the carrier moves with its hold.
  await sheet.getByRole("button", { name: "Destroyer 1 of 1" }).click();
  await expect(sheet.getByText("1 ship · 2 cargo")).toBeVisible();
  await page.screenshot({ path: "shots/local/movement-undone.png", animations: "disabled" });
  await send(page).click();
  await expect.poll(() => savedLine(page)).toContain("9 answers saved");
  await expect(system("23")).toHaveAttribute("aria-label", /1 Sol ship/);
  // One undo again, and one more opens the activation again.
  await undo(page).click();
  await expect(heading(page)).toHaveText("Movement");
  await expect.poll(() => savedLine(page)).toContain("4 answers saved");
  await undo(page).click();
  await expect(heading(page)).toHaveText("Activation");
  await expect.poll(() => savedLine(page)).toContain("3 answers saved");
  expect(errors).toEqual([]);
});

test("hotseat: the viewer is the seat that is asked", async ({ page }) => {
  await page.goto("/?local=3&players=8&humans=255");
  await expect(send(page)).toBeDisabled();
  const me = page
    .getByRole("region", { name: "Players" })
    .locator('tbody tr:has([title="Active player"]) th');
  const seen = new Set<string>();
  for (let count = 0; count < 16; count++) {
    seen.add((await me.first().textContent()) ?? "");
    await answer(page);
  }
  // The strategy phase asks every seat once.
  expect(seen.size).toBe(8);
});

test("an inspected system shows its forces", async ({ page }) => {
  await page.goto("/?local=3&players=8&humans=1");
  await expect(send(page)).toBeDisabled();
  await page.getByRole("button", { name: /^Inspect .*system 01/ }).click();
  await expect(page.getByRole("heading", { name: "Jord - Sol · 01" })).toBeVisible();
  await expect(page.getByText("2 Carriers")).toBeVisible();
  await page.screenshot({ path: "shots/local/inspector.png", animations: "disabled" });
});

test("a reload goes on where the game was, and undo takes back one answer", async ({ page }) => {
  await page.goto("/?local=3&players=8&humans=1");
  await expect(send(page)).toBeDisabled();
  await expect(undo(page)).toHaveCount(0);
  const prompts: string[] = [];
  for (let count = 0; count < 20; count++) {
    prompts.push((await heading(page).textContent()) ?? "");
    await answer(page);
  }
  const open = await heading(page).textContent();
  const round = await page.getByText(/^Round \d+ · /).textContent();
  await expect.poll(() => savedLine(page)).toContain("20 answers saved");

  await page.reload();
  await expect(send(page)).toBeDisabled();
  await expect(heading(page)).toHaveText(open ?? "");
  await expect(page.getByText(/^Round \d+ · /)).toHaveText(round ?? "");
  await expect.poll(() => savedLine(page)).toContain("20 answers saved");

  // Undo opens the choice that was answered last, and the same answer leads to the same game.
  await undo(page).click();
  await expect.poll(() => savedLine(page)).toContain("19 answers saved");
  await expect(heading(page)).toHaveText(prompts[19]);
  await expect(send(page)).toBeDisabled();
  await answer(page);
  await expect(heading(page)).toHaveText(open ?? "");

  // Three in a row, without waiting for a replay: the engine goes back to its checkpoints.
  for (const left of [19, 18, 17]) {
    await undo(page).click();
    await expect.poll(() => savedLine(page)).toContain(`${left} answers saved`);
    await expect(heading(page)).toHaveText(prompts[left]);
  }
  await expect(page.getByText(/^Replaying /)).toHaveCount(0);

  await openSettings(page);
  await settings(page).getByRole("button", { name: "New game" }).click();
  await expect(heading(page)).toHaveText(prompts[0]);
  await expect.poll(() => savedLine(page)).toContain("0 answers saved");
  await expect(undo(page)).toHaveCount(0);
});

test("an exported game is imported in another browser and opens at the same choice", async ({
  page,
  browser,
}) => {
  await page.goto("/?local=5&players=8&humans=1");
  await expect(send(page)).toBeDisabled();
  for (let count = 0; count < 6; count++) {
    await answer(page);
  }
  const open = await heading(page).textContent();
  await openSettings(page);
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    settings(page).getByRole("button", { name: "Export" }).click(),
  ]);
  await page.keyboard.press("Escape");
  const path = await download.path();

  const other = await (await browser.newContext()).newPage();
  // Another game is open: the file says which game it is.
  await other.goto("/?local=3&players=8&humans=1");
  await expect(send(other)).toBeDisabled();
  await other.getByLabel("Saved game file").setInputFiles(path);
  await expect.poll(() => savedLine(other)).toContain("seed 5");
  await expect.poll(() => savedLine(other)).toContain("6 answers saved");
  await expect(heading(other)).toHaveText(open ?? "");

  // A file that is no saved game is refused, and says why.
  await other
    .getByLabel("Saved game file")
    .setInputFiles({ name: "x.json", mimeType: "application/json", buffer: Buffer.from("{}") });
  await openSettings(other);
  await expect(settings(other).getByRole("alert")).toContainText("format undefined");
  await other.context().close();
});

test("a saved game of another engine build is not replayed without a word", async ({ page }) => {
  await page.goto("/?local=3&players=8&humans=1");
  await expect(send(page)).toBeDisabled();
  await answer(page);
  await page.evaluate(() => {
    const save = JSON.parse(localStorage.getItem("ti4.local.3.8.1") ?? "");
    localStorage.setItem("ti4.local.3.8.1", JSON.stringify({ ...save, engine: "another" }));
  });
  await page.reload();
  await expect(page.getByText("was played by another build of the engine")).toBeVisible();
  await page.getByRole("button", { name: "Replay anyway" }).click();
  await expect(send(page)).toBeDisabled();
  await expect.poll(() => savedLine(page)).toContain("1 answers saved");
  // The next answer is saved with this engine.
  await answer(page);
  await expect.poll(() => savedLine(page)).toContain("2 answers saved");

  // An answer that the game does not offer stops the replay and names the answer.
  await page.evaluate(() => {
    const save = JSON.parse(localStorage.getItem("ti4.local.3.8.1") ?? "");
    localStorage.setItem("ti4.local.3.8.1", JSON.stringify({ ...save, answers: ["no-such"] }));
  });
  await page.reload();
  await expect(page.getByRole("status")).toContainText(
    'The saved game does not fit this engine: answer 1 of 1 was "no-such"',
  );
});

test("a draft of a tactical action is private, is kept, and is applied as one request", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("console", (message) => message.type() === "error" && errors.push(message.text()));
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/?local=3&players=8&humans=1");
  const workspace = (name: string) =>
    page.getByRole("navigation", { name: "Workspace" }).getByRole("button", { name });
  const row = (name: string) => panel(page).getByRole("button", { name });
  const system = (id: string) => page.locator("g.system").filter({ hasText: `#${id}` });
  const apply = page.getByRole("button", { name: "Apply to Live" });
  // The draft opens in the strategy phase too. It is sent at the turn menu only.
  await workspace("Draft").click();
  await expect(heading(page)).toHaveText("Activation");
  await expect(apply).toBeDisabled();
  await workspace("Live").click();
  await row("8. Imperial").click();
  await send(page).click();
  await row("Decline").click();
  await send(page).click();
  await expect(row("Take a tactical action")).toBeVisible();

  // The draft opens at the choice of a system. The game is not asked anything.
  await workspace("Draft").click();
  await expect(heading(page)).toHaveText("Activation");
  await expect(page.getByText("Private draft").first()).toBeVisible();
  await expect(apply).toBeDisabled();
  await expect(page.locator("g.system.target:not(.plain)")).toHaveCount(13);
  await system("23").click();
  await page.keyboard.press("Enter");

  // The movement of a draft has no main button: every change is recorded.
  await expect(heading(page)).toHaveText("Movement");
  await expect(send(page)).toHaveCount(0);
  await expect(panel(page).getByText("Private to you until you apply.")).toBeVisible();
  await system("01").click();
  const sheet = page.getByRole("group", { name: /^Move from Jord/ });
  await sheet.getByRole("button", { name: "Carrier 1 of 2" }).click();
  await sheet.getByRole("button", { name: "Destroyer 1 of 1" }).click();
  await expect(sheet.getByText(/· 2 ships$/)).toBeVisible();
  await expect(system("23")).toHaveAttribute("aria-label", /2 Sol ships/);
  await page.screenshot({ path: "shots/local/draft-movement.png", animations: "disabled" });
  // Undo and redo are of the draft.
  await undo(page).click();
  await expect(sheet.getByText(/· 1 ship$/)).toBeVisible();
  await page.getByRole("button", { name: "Redo" }).click();
  await expect(sheet.getByText(/· 2 ships$/)).toBeVisible();
  await expect.poll(() => savedLine(page)).toContain("2 answers saved");

  // Live is as it was, and the draft is still there after a reload.
  await page.reload();
  await expect(row("Take a tactical action")).toBeVisible();
  await expect(system("23")).not.toHaveAttribute("aria-label", /Sol ship/);
  await workspace("Draft").click();
  await expect(heading(page)).toHaveText("Movement");
  await expect(system("23")).toHaveAttribute("aria-label", /2 Sol ships/);

  // "Apply to Live" says what it sends, and sends it.
  await apply.click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText(/^Activate .+ · #23$/)).toBeVisible();
  await expect(dialog.getByText(/^2 ships from .+ · #01$/)).toBeVisible();
  await page.screenshot({ path: "shots/local/draft-apply.png", animations: "disabled" });
  await dialog.getByRole("button", { name: "Apply to Live" }).click();
  await expect(workspace("Live")).toHaveAttribute("aria-pressed", "true");
  await expect.poll(() => savedLine(page)).toContain("8 answers saved");
  await expect(system("23")).toHaveAttribute("aria-label", /2 Sol ships/);
  await expect(heading(page)).not.toHaveText("Movement");
  expect(errors).toEqual([]);
});

test("the other seats wait for a step when the Dev sheet says so", async ({ page }) => {
  await page.goto("/?local=3&players=8&humans=1");
  await expect(send(page)).toBeDisabled();
  const seats = settings(page).getByRole("group", { name: "Other seats" });
  await openSettings(page);
  await expect(seats.getByRole("button", { name: "Run" })).toHaveAttribute("aria-pressed", "true");
  await seats.getByRole("button", { name: "Step" }).click();
  await page.keyboard.press("Escape");
  await panel(page).getByRole("button", { name: "8. Imperial" }).click();
  await send(page).click();
  // The next seat has not chosen: the game waits, and the main button lets it decide.
  await expect(send(page)).toHaveText(/^Step/);
  await expect(panel(page).getByText(/decides next\.$/)).toBeVisible();
  const waiting = await page.locator("header strong").textContent();
  await page.keyboard.press("Enter");
  await expect(page.locator("header strong")).not.toHaveText(waiting ?? "");
  await expect(send(page)).toHaveText(/^Step/);
  // The setting is of the page: a reload keeps it. With Run the seats play on to the next choice.
  await page.reload();
  await expect(send(page)).toHaveText(/^Step/);
  await openSettings(page);
  await expect(seats.getByRole("button", { name: "Step" })).toHaveAttribute("aria-pressed", "true");
  await seats.getByRole("button", { name: "Run" }).click();
  await page.keyboard.press("Escape");
  await expect(send(page)).toHaveText(/^Send/);
});

test("a scenario is chosen in the Dev sheet and starts at the turn menu of its board", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("console", (message) => message.type() === "error" && errors.push(message.text()));
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/?local=3&players=8&humans=1");
  await expect(send(page)).toBeDisabled();
  await openSettings(page);
  const start = settings(page).getByRole("region", { name: "Start from" });
  await expect(start.getByRole("button", { name: /^Round 1/ })).toHaveAttribute(
    "aria-current",
    "true",
  );
  await start.getByRole("button", { name: /^Late game$/ }).click();
  await expect(page).toHaveURL(/\?scenario=late-game&humans=1$/);

  // Round 4, and seat A has the turn: no strategy phase is played first.
  const row = (name: string) => panel(page).getByRole("button", { name });
  await expect(row("Take a tactical action")).toBeVisible();
  await openSettings(page);
  await expect(
    settings(page).getByText(
      "Local game · scenario late-game · seed 42 · 8 seats · 0 answers saved",
    ),
  ).toBeVisible();
  await expect(start.getByRole("button", { name: /^Late game ✓ Selected$/ })).toHaveAttribute(
    "aria-current",
    "true",
  );
  await page.screenshot({ path: "shots/local/scenario-dev.png", animations: "disabled" });
  await page.keyboard.press("Escape");

  // The draft of the turn: many systems are in reach, and the fleet in the gravity rift rolls.
  const system = (id: string) => page.locator("g.system").filter({ hasText: `#${id}` });
  await page
    .getByRole("navigation", { name: "Workspace" })
    .getByRole("button", { name: "Draft" })
    .click();
  await expect(heading(page)).toHaveText("Activation");
  expect(await page.locator("g.system.target:not(.plain)").count()).toBeGreaterThan(30);
  await expect(system("70")).not.toHaveClass(/target/);
  await page.screenshot({ path: "shots/local/scenario-activation.png", animations: "disabled" });
  await system("42").click();
  await page.keyboard.press("Enter");
  await expect(heading(page)).toHaveText("Movement");
  await system("41").click();
  const sheet = page.getByRole("group", { name: /^Move from / });
  await expect(sheet).toBeVisible();
  await sheet.getByRole("button", { name: "Carrier 1 of 1" }).click();
  await expect(sheet.getByText("Rift roll")).toBeVisible();
  await page.screenshot({ path: "shots/local/scenario-movement.png", animations: "disabled" });

  // A reload goes on in the scenario, with the draft.
  await page.reload();
  await expect(row("Take a tactical action")).toBeVisible();
  await page
    .getByRole("navigation", { name: "Workspace" })
    .getByRole("button", { name: "Draft" })
    .click();
  await expect(heading(page)).toHaveText("Movement");
  expect(errors).toEqual([]);
});
