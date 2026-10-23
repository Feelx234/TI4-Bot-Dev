import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openScene, openSheet, SIZES, type SizeName } from "./scene";

// The Prompts list: Ask / My turn / Skip where the ability also comes up in other players' turns,
// the old Ask / Skipped switch where it does not (Bio-Stims, Nullification-like own-only cases).
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`Prompts list with the three-way choice ${size}`, async ({ page }, testInfo) => {
    await openScene(page, { size, faction: "empyrean", technologies: ["bs"] });
    await openSheet(page, size);
    await page.getByTestId("prompts-btn").click();
    await expect(page.getByTestId("prompts-panel")).toBeVisible();
    await expect(page.getByTestId("prompt-mode-dynamo-ask")).toHaveAttribute("aria-pressed", "true");
    // Bio-Stims is only asked at the end of your own turn, Xuange only for other players' moves:
    // they keep the two-state switch.
    await expect(page.getByTestId("prompt-toggle-bio_stims")).toHaveText("Ask");
    await expect(page.getByTestId("prompt-mode-bio_stims-ownturn")).toHaveCount(0);
    await expect(page.getByTestId("prompt-toggle-empyrean_commander")).toHaveText("Ask");
    await shot(page, testInfo, `4-panel-all-ask-${size}`);

    await page.getByTestId("prompt-mode-dynamo-ownturn").click();
    await page.getByTestId("prompt-mode-watcher-skip").click();
    await page.getByTestId("prompt-mode-aetherstream-ownturn").click();
    await expect(page.getByTestId("prompt-mode-dynamo-ownturn")).toHaveAttribute("aria-pressed", "true");
    expect(await page.evaluate(() => localStorage.getItem("player_ask_dynamo"))).toBe("ownturn");
    expect(await page.evaluate(() => localStorage.getItem("player_ask_watcher"))).toBe("false");
    await shot(page, testInfo, `5-panel-mixed-${size}`);

    await page.getByTestId("prompts-panel-close").click();
    await expect(page.getByTestId("paused-prompts-chip")).toContainText("3 prompts skipped");
    await shot(page, testInfo, `6-chip-${size}`);
  });
}

// Stored settings of the first version are kept: "false" is Skip, nothing is Ask.
test("Old stored values still read as Skip and Ask", async ({ page }, testInfo) => {
  await openScene(page, { faction: "empyrean", stored: { dynamo: "false", watcher: "true" } });
  await page.getByTestId("prompts-btn").click();
  await expect(page.getByTestId("prompt-mode-dynamo-skip")).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("prompt-mode-watcher-ask")).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("prompt-mode-aetherstream-ask")).toHaveAttribute("aria-pressed", "true");
  await shot(page, testInfo, "7-old-values");
});

// A seat with few abilities: the sheet button cycles Ask, Skipped, My turn only.
test("Sheet button cycles through the three choices", async ({ page }, testInfo) => {
  await openScene(page, { faction: "l1z1x" });
  const button = page.getByTestId("i48s-ask-btn");
  const top = page.getByTestId("player-sheet-panel").locator("h2").locator("..");
  await expect(button).toContainText("I48S ask");
  await button.click();
  await expect(button).toContainText("I48S skipped");
  await button.click();
  await expect(button).toContainText("I48S my turn only");
  expect(await page.evaluate(() => localStorage.getItem("player_ask_i48s"))).toBe("ownturn");
  await shot(page, testInfo, "8-sheet-my-turn-only", { of: top, pad: 40 });
  await expect(page.getByTestId("paused-prompts-chip")).toContainText("Prompt skipped in others' turns: I48S");
  await shot(page, testInfo, "9-chip-others-turns");
});
