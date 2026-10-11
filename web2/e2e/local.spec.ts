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
      .getByText(/^Local game · /)
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

test("the Dev sheet plays another seat, several times, and undo stays with the seat", async ({
  page,
}) => {
  await page.goto("/?local=3&players=8&humans=1");
  await expect(send(page)).toBeDisabled();
  const seats = settings(page).getByRole("region", { name: "Play as" });
  const seat = (letter: string) => seats.getByRole("button", { name: `Seat ${letter}` });
  /** The seat that the page is asked as: in the strategy phase it is the active player. */
  const asked = page
    .getByRole("region", { name: "Players" })
    .locator('tbody tr:has([title="Active player"]) th');
  const playAs = async (letter: string) => {
    await openSettings(page);
    await seat(letter).click();
    await expect(settings(page)).toBeHidden();
  };
  /** The rows of the seats that are played here. */
  const playing = async () => {
    await openSettings(page);
    const rows = await seats.locator('button[aria-current="true"]').allTextContents();
    await page.keyboard.press("Escape");
    await expect(settings(page)).toBeHidden();
    return rows.join();
  };

  await openSettings(page);
  await expect(seats.getByRole("button", { name: /^Seat / })).toHaveCount(8);
  await expect(seat("A")).toBeDisabled();
  await expect(seat("A")).toContainText("✓ Playing");
  await expect(seat("C")).toContainText("Random");
  await page.screenshot({ path: "shots/local/dev-play-as.png", animations: "disabled" });
  await page.keyboard.press("Escape");

  // Seat A chooses its strategy card at random, and seat C is asked for its own.
  await playAs("C");
  await expect(asked.first()).toContainText("Seat C");
  await expect(send(page)).toBeDisabled();
  expect(await playing()).toMatch(/^Seat C.*✓ Playing$/);
  expect(await savedLine(page)).toContain("0 answers saved");
  // Nothing of seat C can be taken back yet, and the choice of seat A stays.
  await expect(undo(page)).toHaveCount(0);
  const first = await heading(page).textContent();
  await answer(page);
  await undo(page).click();
  await expect.poll(() => savedLine(page)).toContain("0 answers saved");
  await expect(heading(page)).toHaveText(first ?? "");
  await expect(undo(page)).toHaveCount(0);
  expect(await playing()).toMatch(/^Seat C/);
  await answer(page);

  // On to seat E, and a reload plays the game again with both changes.
  await playAs("E");
  await expect.poll(playing).toMatch(/^Seat E/);
  await expect(send(page)).toBeVisible();
  const open = await heading(page).textContent();
  await page.reload();
  await expect(send(page)).toBeVisible();
  await expect(heading(page)).toHaveText(open ?? "");
  expect(await playing()).toMatch(/^Seat E/);
  expect(await savedLine(page)).toContain("1 answers saved");
  await expect(undo(page)).toHaveCount(0);

  // Back to the seat of the start, which was a random seat in between.
  await playAs("A");
  await expect.poll(playing).toMatch(/^Seat A/);
  await answer(page);
  await expect(undo(page)).toBeVisible();
});

test("an invasion: the landings are staged on the map and sent as one plan", async ({ page }) => {
  const errors: string[] = [];
  page.on("console", (message) => message.type() === "error" && errors.push(message.text()));
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/?scenario=late-game&humans=1");
  const row = (name: string) => panel(page).getByRole("button", { name });
  const system = (id: string) => page.locator("g.system").filter({ hasText: `#${id}` });
  const board = page.locator("section[aria-label='Galaxy board']");
  const decks = page.getByRole("complementary", { name: "Exploration" });
  // The inspector says what the exploration left on a planet: its type and what is attached.
  await system("74").click();
  const mapView = (name: string) =>
    page.getByRole("group", { name: "Map view" }).getByRole("button", { name });
  const depot = board
    .locator("> div")
    .filter({ has: page.getByRole("button", { name: "Close system details" }) });
  await expect(depot.getByText("Dyson Sphere", { exact: true })).toBeVisible();
  await expect(depot.getByText("+2 / +1")).toBeVisible();
  await expect(depot.getByText("Unexplored", { exact: true })).toHaveCount(0);
  await expect(depot.getByRole("button", { name: /^Cultural planet Vega Major/ })).toBeVisible();
  await page.screenshot({ path: "shots/local/inspector-attachment.png", animations: "disabled" });
  await page.getByRole("button", { name: "Close system details" }).click();
  await row("Take a tactical action").click();
  await send(page).click();
  // The carrier of the north takes two infantry and a mech to a system with two free planets.
  await expect(heading(page)).toHaveText("Activation");
  await system("38").click();
  await page.keyboard.press("Enter");
  await expect(heading(page)).toHaveText("Movement");
  await system("63").click();
  const fleet = page.getByRole("group", { name: /^Move from / });
  await fleet.getByRole("button", { name: "Carrier 1 of 1" }).click();
  await fleet.getByRole("button", { name: /^Load Infantry, Planet Vorhal, 2 left/ }).click();
  await fleet.getByRole("button", { name: "Load Infantry on Carrier 1 of 1" }).first().click();
  await fleet.getByRole("button", { name: "Load Infantry on Carrier 1 of 1" }).first().click();
  await fleet.getByRole("button", { name: /^Load Mech, Planet Vorhal, 1 left/ }).click();
  await fleet.getByRole("button", { name: "Load Mech on Carrier 1 of 1" }).first().click();
  await expect(fleet.getByText("1 ship · 3 cargo")).toBeVisible();
  await send(page).click();

  // Invasion: the landings open at the active system, and the panel has no controls.
  await expect(heading(page)).toHaveText("Invasion");
  await expect(send(page)).toHaveText(/Land nothing/);
  const sheet = page.getByRole("group", { name: /^Land in / });
  await expect(sheet).toBeVisible();
  await expect(
    sheet.getByRole("button", { name: "Land Infantry, Space area, 2 left" }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(sheet.getByRole("button", { name: "Land Mech, Space area, 1 left" })).toBeVisible();
  await expect(panel(page).getByText("2 Infantry, 1 Mech · not landed")).toBeVisible();
  await expect(panel(page).getByRole("spinbutton")).toHaveCount(0);
  const planet = (name: string) => page.locator("g.planet").filter({ hasText: name });
  await expect(page.locator("g.planet.payable")).toHaveCount(2);
  await page.screenshot({ path: "shots/local/invasion-open.png", animations: "disabled" });
  const saved = await savedLine(page);

  // Nobody has these planets: a landing explores them. The type of a planet opens its deck.
  await expect(sheet.getByText("Unexplored", { exact: true })).toHaveCount(2);
  await sheet.getByRole("button", { name: /^Hazardous planet Abyz/ }).click();
  await expect(decks).toBeVisible();
  const hazardous = decks.getByRole("region", { name: "Hazardous deck" });
  await expect(hazardous).toBeInViewport();
  await expect(hazardous.getByText("19 of 20 cards left")).toBeVisible();
  // A card has its printed text, how many are left, and who drew one.
  await expect(hazardous.getByText("Mining World")).toBeVisible();
  await expect(
    hazardous.getByText("This planet's resource value is increased by 2."),
  ).toBeVisible();
  await expect(hazardous.getByText("Drawn · Round 2 · Sol · Mehar Xull")).toBeVisible();
  await expect(hazardous.getByText(/^Relic fragments/)).toContainText("Sol 1");
  await page.screenshot({ path: "shots/local/exploration-deck.png", animations: "disabled" });
  await page.getByRole("button", { name: "Close Exploration" }).click();
  await expect(sheet).toBeVisible();
  // The map view of the exploration: the type of each planet, "?" while it is still explored.
  await mapView("Exploration").click();
  // (A planet of the open choice keeps its values in every map view.)
  await expect(planet("Siig")).toContainText("H?");
  await expect(page.locator("g.planet").filter({ hasText: "Vega Major" })).toContainText("C+");
  await page.screenshot({ path: "shots/local/exploration-map.png", animations: "disabled" });
  await mapView("Standard").click();

  // A slot of the sheet lands the chosen kind; a tap on a planet of the map lands one more.
  await sheet.getByRole("button", { name: "Land Infantry on Abyz" }).click();
  await planet("Abyz").click();
  await sheet.getByRole("button", { name: "Land Mech, Space area, 1 left" }).click();
  await sheet.getByRole("button", { name: "Land Mech on Fria" }).click();
  await expect(sheet.getByText("3 units land")).toBeVisible();
  await expect(panel(page).getByRole("button", { name: /^Abyz 2 Infantry$/ })).toBeVisible();
  await expect(panel(page).getByRole("button", { name: /^Fria 1 Mech$/ })).toBeVisible();
  await expect(panel(page).getByText("Every ground force lands")).toBeVisible();
  await expect(page.locator("g.planet.chosen")).toHaveCount(2);
  await expect(send(page)).toHaveText(/Commit ground forces/);
  await page.screenshot({ path: "shots/local/invasion.png", animations: "disabled" });
  // A token takes its unit back, and Reset everything.
  await sheet.getByRole("button", { name: "Take back Infantry from Abyz" }).first().click();
  await expect(
    sheet.getByRole("button", { name: "Land Infantry, Space area, 1 left" }),
  ).toBeVisible();
  await sheet.getByRole("button", { name: "Land Infantry, Space area, 1 left" }).click();
  await sheet.getByRole("button", { name: "Land Infantry on Fria" }).click();
  await expect(
    panel(page).getByRole("button", { name: /^Fria 1 Infantry, 1 Mech$/ }),
  ).toBeVisible();
  // Nothing was sent yet, and a reload keeps what is staged.
  expect(await savedLine(page)).toBe(saved);
  await page.reload();
  await expect(send(page)).toHaveText(/Commit ground forces/);
  await expect(sheet.getByText("3 units land")).toBeVisible();

  // The main button sends every landing, and the game goes on: the planets are Sol's.
  await send(page).click();
  await expect(heading(page)).not.toHaveText("Invasion");
  await expect(sheet).toBeHidden();
  await system("38").click();
  const inspector = page
    .locator("section[aria-label='Galaxy board'] > div")
    .filter({ hasText: "Abyz" });
  await expect(inspector.getByText("1 Infantry").first()).toBeVisible();
  await expect(inspector.getByText("1 Mech · 1 Infantry")).toBeVisible();
  // Abyz was explored, and its card asks: the decision has the card on top, as printed. Fria
  // is explored after it.
  await expect(panel(page).getByText("Hazardous exploration · Abyz")).toBeVisible();
  await expect(panel(page).getByText(/If you have at least 1 mech on this planet/)).toBeVisible();
  await expect(inspector.getByRole("button", { name: /^Hazardous planet Abyz/ })).toBeVisible();
  await expect(inspector.getByText("Unexplored", { exact: true })).toHaveCount(1);
  await page.screenshot({ path: "shots/local/after-invasion.png", animations: "disabled" });
  await page.locator("header").getByRole("button", { name: "Exploration" }).click();
  await expect(decks.getByText("Drawn · Round 4 · Sol · Abyz")).toBeVisible();
  await expect(decks.getByText("18 of 20 cards left").first()).toBeVisible();
  await page.getByRole("button", { name: "Close Exploration" }).click();

  // One undo takes the landings back whole, and they are staged again.
  await undo(page).click();
  await expect(heading(page)).toHaveText("Invasion");
  await expect(send(page)).toHaveText(/Commit ground forces/);
  expect(await savedLine(page)).toBe(saved);
  await expect(
    panel(page).getByRole("button", { name: /^Fria 1 Infantry, 1 Mech$/ }),
  ).toBeVisible();
  // The check mark closes the sheet; the active system opens it again.
  await system("38").click();
  await expect(sheet).toBeVisible();
  await sheet.getByRole("button", { name: "Reset" }).click();
  await expect(send(page)).toHaveText(/Land nothing/);
  await sheet.getByRole("button", { name: "Done with the landings" }).click();
  await expect(sheet).toBeHidden();
  await panel(page)
    .getByRole("button", { name: /^Abyz Nothing lands$/ })
    .click();
  await expect(sheet).toBeVisible();
  expect(errors).toEqual([]);
});

test("a production: the build is staged on the map, then paid on the map, and sent as one plan", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("console", (message) => message.type() === "error" && errors.push(message.text()));
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/?scenario=late-game&humans=1");
  const row = (name: string | RegExp) => panel(page).getByRole("button", { name });
  const system = (id: string) => page.locator("g.system").filter({ hasText: `#${id}` });
  const planet = (name: string) => page.locator("g.planet").filter({ hasText: name });
  await row("Take a tactical action").click();
  await send(page).click();
  // The system with two space docks. The destroyer of the nebula moves there.
  await expect(heading(page)).toHaveText("Activation");
  await system("72").click();
  await expect(panel(page).getByText("Production 13")).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(heading(page)).toHaveText("Movement");
  await system("42").click();
  await page
    .getByRole("group", { name: /^Move from / })
    .getByRole("button", { name: "Destroyer 1 of 1" })
    .click();
  await send(page).click();
  // AI Development Algorithm asks before the production: a decision in the list.
  await row(/^Decline/i).click();
  await send(page).click();

  // Production: the build opens at the system that produces, and the panel has no controls.
  await expect(heading(page)).toHaveText("Production");
  await expect(send(page)).toHaveText(/Produce nothing/);
  const sheet = page.getByRole("group", { name: /^Produce in / });
  await expect(sheet).toBeVisible();
  await expect(sheet.getByRole("status").first()).toHaveText("Nothing is produced");
  // Every kind of unit has its row, as printed. One that is not produced here says why.
  const ships = sheet.getByRole("group", { name: "Ships · Space area" });
  const forces = sheet.getByRole("group", { name: "Ground forces" });
  for (const name of ["Carrier", "Cruiser", "Destroyer", "Dreadnought", "Flagship", "War sun"]) {
    await expect(ships.getByText(name, { exact: true })).toBeVisible();
  }
  await expect(forces.getByText("Mech", { exact: true })).toBeVisible();
  await expect(sheet.getByText("None left")).toHaveCount(3);
  await expect(ships.getByRole("button", { name: "Add Carrier" })).toHaveCount(0);
  await expect(ships.getByRole("button", { name: "Add War sun" })).toBeEnabled();
  // The limits, with the resources that the seat has.
  for (const limit of [
    "Production capacity",
    "Resources",
    "Fleet supply after",
    "Fighters and cargo in space",
  ]) {
    await expect(sheet.getByText(limit, { exact: true })).toBeVisible();
  }
  const gauge = (label: string) =>
    sheet.getByText(label, { exact: true }).locator("xpath=following-sibling::strong");
  await expect(gauge("Resources")).toHaveText("0 / 26");
  const cargo = Number((await gauge("Fighters and cargo in space").innerText()).split("/")[1]);
  expect(await sheet.evaluate((node) => node.scrollHeight <= node.clientHeight)).toBe(true);
  await expect(panel(page).getByRole("spinbutton")).toHaveCount(0);
  // No planet pays while the build is chosen, and there is no payment bar.
  await expect(page.locator("g.planet.payable")).toHaveCount(0);
  await expect(page.getByRole("group", { name: "Payment" })).toHaveCount(0);
  await page.screenshot({ path: "shots/local/production-open.png", animations: "disabled" });
  const saved = await savedLine(page);

  // A counter buys in its place: two fighters, a cruiser, infantry on each planet.
  await ships.getByRole("button", { name: "Add Fighter" }).click();
  await expect(ships.getByLabel("Fighter quantity")).toHaveText("2");
  await ships.getByRole("button", { name: "Add Cruiser" }).click();
  await forces.getByRole("button", { name: "Add Infantry, Planet Velnor" }).click();
  await forces.getByRole("button", { name: "Add Infantry, Planet Lisis" }).click();
  await expect(sheet.getByRole("status").first()).toHaveText("7 units · 4 resources · discount 1");
  await expect(gauge("Resources")).toHaveText("4 / 26");
  await expect(row(/^Space area 1 Cruiser, 2 Fighters$/)).toBeVisible();
  await expect(row(/^Planet Velnor 2 Infantry$/)).toBeVisible();
  // A staged ship adds its capacity to the limit of the cargo, and takes it back with it.
  await expect(gauge("Fighters and cargo in space")).toHaveText(new RegExp(`/ ${cargo + 1}$`));
  await ships.getByRole("button", { name: "Add War sun" }).click();
  await expect(gauge("Fighters and cargo in space")).toHaveText(new RegExp(`/ ${cargo + 7}$`));
  await expect(ships.getByRole("button", { name: "Add War sun" })).toBeDisabled();
  await ships.getByRole("button", { name: "Remove War sun" }).click();
  await expect(gauge("Fighters and cargo in space")).toHaveText(new RegExp(`/ ${cargo + 1}$`));
  // The "−" of a counter takes a purchase back.
  await forces.getByRole("button", { name: "Remove Infantry, Planet Lisis" }).click();
  await expect(sheet.getByRole("status").first()).toHaveText("5 units · 3 resources · discount 1");
  await expect(row(/^Planet Lisis Nothing is produced$/)).toBeVisible();
  await expect(send(page)).toHaveText(/Pay 3 resources/);
  await page.screenshot({ path: "shots/local/production-build.png", animations: "disabled" });

  // Then the payment, on the map: the sheet closes, the bar opens, the ready planets pay.
  await send(page).click();
  await expect(sheet).toBeHidden();
  const bar = page.getByRole("group", { name: "Payment" });
  await expect(bar.getByRole("status").first()).toHaveText(/Paying\s*0 \/ 3\s*resources\s*3 more/);
  await expect(send(page)).toHaveText(/Produce units/);
  await expect(send(page)).toBeDisabled();
  await expect(page.locator("g.planet.payable")).toHaveCount(8);
  await planet("Lisis").click();
  await expect(bar.getByRole("status").first()).toHaveText(/2 \/ 3\s*resources\s*1 more/);
  await bar.getByRole("button", { name: "Pay rest" }).click();
  await expect(bar.getByRole("status").first()).toHaveText(/3 \/ 3\s*resources\s*✓ Paid/);
  await expect(page.locator("footer").getByText("Lisis 2 + 1 trade good")).toBeVisible();
  await expect(send(page)).toBeEnabled();
  await page.screenshot({ path: "shots/local/production-pay.png", animations: "disabled" });
  // Auto-pay finds the payment with the least waste: one planet of 3.
  await bar.getByRole("button", { name: "Clear payment" }).click();
  await expect(send(page)).toBeDisabled();
  await bar.getByRole("button", { name: "Auto-pay" }).click();
  await expect(bar.getByRole("status").first()).toHaveText(/3 \/ 3\s*resources\s*✓ Paid/);
  await expect(page.locator("g.planet.chosen")).toHaveCount(1);
  // Esc goes back to the build, with the payment still staged; the main button pays again.
  await page.keyboard.press("Escape");
  await expect(sheet).toBeVisible();
  await expect(bar).toHaveCount(0);
  await send(page).click();
  await bar.getByRole("button", { name: "Clear payment" }).click();
  await planet("Lisis").click();
  await bar.getByRole("button", { name: "Pay rest" }).click();
  // Nothing was sent yet, and a reload keeps what is staged.
  expect(await savedLine(page)).toBe(saved);
  await page.reload();
  await expect(sheet.getByRole("status").first()).toHaveText("5 units · 3 resources · discount 1");
  await send(page).click();
  await expect(bar.getByRole("status").first()).toHaveText(/3 \/ 3\s*resources\s*✓ Paid/);

  // The main button sends the build and its payment, and the game goes on.
  await send(page).click();
  await expect(heading(page)).not.toHaveText("Production");
  await expect(bar).toHaveCount(0);
  await system("72").click();
  const inspector = page
    .locator("section[aria-label='Galaxy board'] > div")
    .filter({ hasText: "Velnor" });
  await expect(inspector.getByText(/2 Cruisers/)).toBeVisible();
  await expect(inspector.getByText(/4 Fighters/)).toBeVisible();
  await expect(inspector.getByText("Exhausted")).toHaveCount(1);
  await page.screenshot({ path: "shots/local/after-production.png", animations: "disabled" });

  // One undo takes the production back whole, and it is staged again.
  await undo(page).click();
  await expect(heading(page)).toHaveText("Production");
  expect(await savedLine(page)).toBe(saved);
  await expect(sheet.getByRole("status").first()).toHaveText("5 units · 3 resources · discount 1");
  await expect(send(page)).toHaveText(/Pay 3 resources/);
  // Reset takes the build back; the check mark closes the sheet, and the system opens it again.
  await sheet.getByRole("button", { name: "Reset" }).click();
  await expect(sheet.getByRole("status").first()).toHaveText("Nothing is produced");
  await sheet.getByRole("button", { name: "Done with the build" }).click();
  await expect(sheet).toBeHidden();
  await expect(send(page)).toHaveText(/Produce nothing/);
  await system("72").click();
  await expect(sheet).toBeVisible();
  expect(errors).toEqual([]);
});

test("the sheet of a player has its abilities, leaders and used cards as printed", async ({
  page,
}) => {
  await page.goto("/?scenario=late-game&humans=1");
  await page.getByRole("button", { name: /^Seat A, Sol.*Open player sheet/ }).click();
  const sheet = page.getByRole("complementary", { name: "Seat A · Sol" });
  const cards = (section: string) => sheet.getByRole("region", { name: section });
  const card = (name: string) => sheet.getByRole("article", { name });
  await expect(cards("Faction abilities").getByRole("article")).toHaveCount(2);
  await expect(card("Orbital Drop")).toContainText("ACTION: Spend 1 token from your strategy pool");
  // A leader says its state in a word, and a locked one how it is unlocked.
  await expect(cards("Leaders").getByRole("article")).toHaveCount(3);
  await expect(card("Evelyn DeLouis")).toContainText("Exhausted");
  await expect(card("Evelyn DeLouis")).toContainText("1 additional die");
  await expect(card("Claire Gibson")).toContainText("Ready");
  await expect(card("Jace X, 4th Air Legion")).toContainText("Locked");
  await expect(card("Jace X, 4th Air Legion")).toContainText("Unlock: Have 3 scored objectives.");
  // Only the cards that are used: Gravity Drive always holds, so it stays a line of the list.
  await expect(cards("Action components").getByRole("article")).toHaveCount(4);
  await expect(card("AI Development Algorithm")).toContainText("Ready");
  await expect(card("Bio-Stims")).toContainText("Exhausted");
  await expect(card("Dynamis Core")).toContainText("Ready");
  await expect(card("The Prophet's Tears")).toContainText("Exhausted");
  await expect(sheet.getByRole("article", { name: "Gravity Drive" })).toHaveCount(0);
  await expect(sheet.getByText("GD", { exact: true })).toBeVisible();
  await page.screenshot({ path: "shots/local/player-sheet.png", animations: "disabled" });
  // Another seat: its cards are public too, and nothing of its hand is here.
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: /^Seat B, .*Open player sheet/ }).click();
  const other = page.getByRole("complementary", { name: /^Seat B · / });
  await expect(other.getByRole("region", { name: "Leaders" }).getByRole("article")).toHaveCount(3);
});
