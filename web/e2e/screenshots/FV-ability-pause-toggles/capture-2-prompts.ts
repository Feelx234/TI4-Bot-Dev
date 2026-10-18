import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openScene, openSheet, SIZES, type SizeName } from "./scene";

// An Empyrean seat has seven switchable abilities: the sheet shows one Prompts button, not seven.
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`Prompts control on the sheet ${size}`, async ({ page }, testInfo) => {
    await openScene(page, { size, faction: "empyrean" });
    await openSheet(page, size);
    const button = page.getByTestId("prompts-btn");
    await expect(button).toBeVisible();
    await expect(button).toContainText(/^Prompts \(7\)$/);
    await expect(page.getByTestId("aetherpassage-ask-btn")).toHaveCount(0);
    await shot(page, testInfo, `3-sheet-prompts-${size}`);

    await button.click();
    const panel = page.getByTestId("prompts-panel");
    await expect(panel).toBeVisible();
    await expect(page.getByTestId("prompt-toggle-aetherpassage")).toHaveText("Ask");
    await shot(page, testInfo, `4-panel-all-ask-${size}`);

    await page.getByTestId("prompt-toggle-aetherpassage").click();
    await page.getByTestId("prompt-toggle-dynamo").click();
    await page.getByTestId("prompt-toggle-watcher").click();
    await expect(page.getByTestId("prompt-toggle-dynamo")).toHaveText("Skipped");
    expect(await page.evaluate(() => localStorage.getItem("player_ask_aetherpassage"))).toBe("false");
    expect(await page.evaluate(() => localStorage.getItem("player_ask_dynamo"))).toBe("false");
    await shot(page, testInfo, `5-panel-some-skipped-${size}`);

    await page.getByTestId("prompts-panel-close").click();
    await expect(panel).toBeHidden();
    await expect(page.getByTestId("prompts-btn")).toContainText("Prompts (3 skipped)");
    await expect(page.getByTestId("paused-prompts-chip")).toContainText("3 prompts skipped");
    await shot(page, testInfo, `6-sheet-after-${size}`);
  });
}

// A seat with few abilities keeps one button each (the I48S and Aetherpassage keys are unchanged).
test("A seat with few abilities keeps the buttons", async ({ page }, testInfo) => {
  await openScene(page, { faction: "l1z1x" });
  const button = page.getByTestId("i48s-ask-btn");
  await expect(button).toContainText("I48S ask");
  await expect(page.getByTestId("prompts-btn")).toHaveCount(0);
  await button.click();
  await expect(button).toContainText("I48S skipped");
  expect(await page.evaluate(() => localStorage.getItem("player_ask_i48s"))).toBe("false");
  await shot(page, testInfo, "7-few-abilities", { of: page.getByTestId("player-sheet-panel").locator("h2").locator(".."), pad: 40 });
});
