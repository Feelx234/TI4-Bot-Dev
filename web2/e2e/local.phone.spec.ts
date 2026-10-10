import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

// Local play on a Pixel 9 in portrait. Needs the engine: `scripts/build-wasm.sh`.
const WASM = fileURLToPath(new URL("../src/session/ti4.wasm", import.meta.url));
test.skip(!existsSync(WASM), "the engine is not built: run web2/scripts/build-wasm.sh");
test.setTimeout(120_000);

test("phone: a game of eight is played through the decision list", async ({ page }) => {
  await page.goto("/?local=3&players=8&humans=1");
  // The main button of the footer: "Send", "Activate system", "Move nothing".
  const send = page.locator('footer button[aria-keyshortcuts="Enter"]');
  const pane = (name: string) =>
    page.getByRole("navigation", { name: "Panes" }).getByRole("button", { name });
  // On a phone, Dev is an item of "Reference and more", not a button of the header.
  const savedLine = async () => {
    await page.getByRole("button", { name: "Reference and more" }).click();
    await page.getByRole("menuitem", { name: "Dev" }).click();
    const line = page.getByRole("complementary", { name: "Dev" }).getByText(/^Local game · /);
    const text = (await line.textContent()) ?? "";
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "Dev" })).toBeHidden();
    return text;
  };
  await expect(send).toBeDisabled();
  await page.screenshot({ path: "shots/phone/local/first-choice.png", animations: "disabled" });
  // A later movement moves nothing: the main button already says so.
  let moved = false;
  for (let count = 0; count < 12; count++) {
    const before = await savedLine();
    const system = page.locator("g.system.target");
    const moving = /Move/.test((await send.textContent()) ?? "");
    if (!moving && (await system.count())) {
      await pane("Map").click();
      // The system next to the home of this seat, so that the movement has ships in range.
      const near = system.filter({ hasText: "#23" });
      await ((await near.count()) ? near : system).first().click();
      await page.screenshot({
        path: "shots/phone/local/system-choice.png",
        animations: "disabled",
      });
    } else if (!moving && (await send.isDisabled())) {
      await pane("Action").click();
      const rows = page.locator("#step-panel").getByRole("button", { pressed: false });
      // A tactical action when there is one, so that the run has a choice on the map.
      const tactical = rows.and(page.getByRole("button", { name: "Take a tactical action" }));
      await ((await tactical.count()) ? tactical : rows).first().click();
    } else if (moving && !moved) {
      // The first movement: the map stays open, and the fleet of a system opens in a sheet over
      // it. The sheet has the room of the footer.
      moved = true;
      await expect(pane("Map")).toHaveAttribute("aria-pressed", "true");
      await page.locator("g.system").filter({ hasText: "#01" }).click();
      const sheet = page.getByRole("group", { name: /^Move from / });
      await sheet.getByRole("button", { name: /^Carrier 1 of / }).click();
      await expect(send).toBeHidden();
      await page.screenshot({ path: "shots/phone/local/movement.png", animations: "disabled" });
      // The check of the sheet closes it: the main button is back.
      await sheet.getByRole("button", { name: "Done with this system" }).click();
      await expect(send).toHaveText(/Move fleet/);
      await expect(page.locator(".galaxy .t-staged")).toHaveCount(1);
      // An undo of the movement stages it again, and the map is open again.
      await send.click();
      await expect(send).not.toHaveText(/Move/);
      await pane("Action").click();
      await page.getByRole("button", { name: "Undo" }).click();
      await expect(send).toHaveText(/Move/);
      await expect(pane("Map")).toHaveAttribute("aria-pressed", "true");
      await expect(send).toHaveText(/Move fleet/);
      await expect(page.locator(".galaxy .t-staged")).toHaveCount(1);
      await page.screenshot({ path: "shots/phone/local/undo.png", animations: "disabled" });
      // An undo opens the map also when the choice before it was on the map: the activation.
      await pane("Action").click();
      await page.getByRole("button", { name: "Undo" }).click();
      await expect(send).toHaveText(/Activate system/);
      await expect(pane("Map")).toHaveAttribute("aria-pressed", "true");
      await system.filter({ hasText: "#23" }).first().click();
      await send.click();
      await expect(send).toHaveText(/Move nothing/);
    }
    // The main button is under every pane, in view.
    await expect(send).toBeInViewport({ ratio: 1 });
    await send.click();
    await expect.poll(savedLine).not.toBe(before);
    await expect(send).toBeVisible();
    // Nothing is cut at the side.
    const width = page.viewportSize()!.width;
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width);
  }
  expect(existsSync("shots/phone/local/system-choice.png"), "a system was chosen on the map").toBe(
    true,
  );
  expect(existsSync("shots/phone/local/movement.png"), "a ship was moved from the sheet").toBe(
    true,
  );
  await pane("Players").click();
  await page.screenshot({ path: "shots/phone/local/players.png", animations: "disabled" });
});

test("phone: a draft is staged on the map and applied from the toolbar", async ({ page }) => {
  await page.goto("/?local=3&players=8&humans=1");
  const send = page.locator('footer button[aria-keyshortcuts="Enter"]');
  const pane = (name: string) =>
    page.getByRole("navigation", { name: "Panes" }).getByRole("button", { name });
  const row = (name: string) => page.locator("#step-panel").getByRole("button", { name });
  const system = (id: string) => page.locator("g.system").filter({ hasText: `#${id}` });
  const width = page.viewportSize()!.width;
  const fits = async () =>
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width);
  await expect(send).toBeDisabled();
  for (const name of ["8. Imperial", "Decline"]) {
    await pane("Action").click();
    await row(name).click();
    await send.click();
  }
  await expect(row("Take a tactical action")).toBeVisible();

  await page
    .getByRole("navigation", { name: "Workspace" })
    .getByRole("button", { name: "Draft" })
    .click();
  // The system is chosen on the map, and the main button records it.
  await expect(pane("Map")).toHaveAttribute("aria-pressed", "true");
  await system("23").click();
  await expect(send).toHaveText(/Activate system/);
  await expect(send).toBeInViewport({ ratio: 1 });
  await send.click();
  // The movement of a draft has no main button; the controls of the draft are in the toolbar.
  await system("01").click();
  const sheet = page.getByRole("group", { name: /^Move from / });
  await sheet.getByRole("button", { name: /^Carrier 1 of / }).click();
  await sheet.getByRole("button", { name: "Done with this system" }).click();
  await expect(send).toHaveCount(0);
  await expect(system("23")).toHaveAttribute("aria-label", /1 Sol ship/);
  await fits();
  await page.screenshot({ path: "shots/phone/local/draft.png", animations: "disabled" });
  const apply = page.getByRole("button", { name: "Apply to Live" });
  await expect(apply).toBeInViewport({ ratio: 1 });
  await apply.click();
  await fits();
  await page.screenshot({ path: "shots/phone/local/draft-apply.png", animations: "disabled" });
  await page.getByRole("dialog").getByRole("button", { name: "Apply to Live" }).click();
  await expect(apply).toHaveCount(0);
  await expect(system("23")).toHaveAttribute("aria-label", /1 Sol ship/);
  await fits();
});

test("phone: the Dev sheet plays another seat", async ({ page }) => {
  await page.goto("/?local=3&players=8&humans=1");
  const send = page.locator('footer button[aria-keyshortcuts="Enter"]');
  await expect(send).toBeDisabled();
  const sheet = page.getByRole("complementary", { name: "Dev" });
  const seats = sheet.getByRole("region", { name: "Play as" });
  const open = async () => {
    await page.getByRole("button", { name: "Reference and more" }).click();
    await page.getByRole("menuitem", { name: "Dev" }).click();
    await expect(seats).toBeVisible();
  };
  await open();
  await expect(seats.getByRole("button", { name: "Seat A" })).toContainText("✓ Playing");
  // Nothing of the sheet is cut at the side.
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(412);
  await page.screenshot({ path: "shots/phone/local/dev-play-as.png", animations: "disabled" });
  await seats.getByRole("button", { name: "Seat C" }).click();
  await expect(sheet).toBeHidden();
  await open();
  await expect(seats.getByRole("button", { name: "Seat C" })).toContainText("✓ Playing");
  await expect(seats.getByRole("button", { name: "Seat A" })).toContainText("Random");
});

test("phone: the landings are staged in a sheet over the map", async ({ page }) => {
  await page.goto("/?scenario=late-game&humans=1");
  const send = page.locator('footer button[aria-keyshortcuts="Enter"]');
  const pane = (name: string) =>
    page.getByRole("navigation", { name: "Panes" }).getByRole("button", { name });
  const system = (id: string) => page.locator("g.system").filter({ hasText: `#${id}` });
  const width = page.viewportSize()!.width;
  const fits = async () =>
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width);
  await page.locator("#step-panel").getByRole("button", { name: "Take a tactical action" }).click();
  await send.click();
  await system("38").click();
  await send.click();
  await system("63").click();
  const fleet = page.getByRole("group", { name: /^Move from / });
  await fleet.getByRole("button", { name: "Carrier 1 of 1" }).click();
  await fleet.getByRole("button", { name: /^Load Infantry, Planet Vorhal/ }).click();
  await fleet.getByRole("button", { name: "Load Infantry on Carrier 1 of 1" }).first().click();
  await fleet.getByRole("button", { name: "Load Infantry on Carrier 1 of 1" }).first().click();
  await fleet.getByRole("button", { name: "Done with this system" }).click();
  await send.click();

  // The map stays open, and the landings are a sheet over it, with the room of the footer.
  const sheet = page.getByRole("group", { name: /^Land in / });
  await expect(sheet).toBeVisible();
  await expect(pane("Map")).toHaveAttribute("aria-pressed", "true");
  await expect(send).toBeHidden();
  await sheet.getByRole("button", { name: "Land Infantry on Abyz" }).click();
  await sheet.getByRole("button", { name: "Land Infantry on Fria" }).click();
  await expect(sheet.getByText("2 units land")).toBeVisible();
  await fits();
  await page.screenshot({ path: "shots/phone/local/invasion.png", animations: "disabled" });
  // The type of a planet opens its exploration deck, as a sheet over the pane.
  await expect(sheet.getByText("Unexplored", { exact: true })).toHaveCount(2);
  await sheet.getByRole("button", { name: /^Hazardous planet Fria/ }).click();
  const decks = page.getByRole("complementary", { name: "Exploration" });
  await expect(decks.getByRole("region", { name: "Hazardous deck" })).toBeInViewport();
  await expect(
    decks.getByRole("region", { name: "Hazardous deck" }).getByText("19 of 20 cards left"),
  ).toBeVisible();
  await fits();
  await page.screenshot({ path: "shots/phone/local/exploration-deck.png", animations: "disabled" });
  await page.getByRole("button", { name: "Close Exploration" }).click();
  await expect(sheet).toBeVisible();
  // The check of the sheet closes it: the main button is back, in view.
  await sheet.getByRole("button", { name: "Done with the landings" }).click();
  await expect(send).toHaveText(/Commit ground forces/);
  await expect(send).toBeInViewport({ ratio: 1 });
  // The panel says what lands, and a row of it opens the sheet again, on the map.
  await pane("Action").click();
  await page.screenshot({ path: "shots/phone/local/invasion-panel.png", animations: "disabled" });
  await fits();
  await page
    .locator("#step-panel")
    .getByRole("button", { name: /^Abyz 1 Infantry$/ })
    .click();
  await pane("Map").click();
  await expect(sheet).toBeVisible();
  await sheet.getByRole("button", { name: "Done with the landings" }).click();
  await send.click();
  await expect(send).not.toHaveText(/Commit/);
  await fits();
});
