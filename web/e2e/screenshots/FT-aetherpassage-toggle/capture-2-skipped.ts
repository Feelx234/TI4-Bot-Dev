import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { aetherOffer, openScene } from "./scene";

// Switch it off in the player sheet; the next activation's offer is declined by the client.
test("Aetherpassage skipped after switching off", async ({ page }, testInfo) => {
  const { advance, submissions } = await openScene(page);
  // The Empyrean seat has several switchable abilities: they sit behind the sheet's Prompts button.
  const sheetTop = page.getByTestId("player-sheet-panel").locator("h2").locator("..");
  await expect(page.getByTestId("prompts-btn")).toContainText("Prompts (7)");
  await shot(page, testInfo, "2-sheet-ask", { of: sheetTop, pad: 40 });
  await page.getByTestId("prompts-btn").click();
  const toggle = page.getByTestId("prompt-toggle-aetherpassage");
  await expect(toggle).toHaveText("Ask");
  await toggle.click();
  await expect(toggle).toHaveText("Skipped");
  await shot(page, testInfo, "3-sheet-skipped");
  await page.getByTestId("prompts-panel-close").click();
  advance(1, null);
  await page.waitForTimeout(300);
  advance(2, aetherOffer("aether-2"));
  await expect.poll(() => submissions.length).toBe(1);
  expect(JSON.stringify(submissions[0])).toContain("decline");
  await expect(page.getByTestId("corner-toast")).toContainText("Aetherpassage skipped");
  await shot(page, testInfo, "4-declined-toast");
});

// Ticking the switch inside the offer itself turns it off for next time.
test("Aetherpassage switch inside the offer", async ({ page }, testInfo) => {
  const { advance, submissions } = await openScene(page);
  advance(1, null);
  advance(2, aetherOffer("aether-3"));
  await page.getByTestId("aetherpassage-switch-input").check();
  await expect(page.getByTestId("aetherpassage-switch-input")).toBeChecked();
  await shot(page, testInfo, "5-switch-ticked");
  expect(submissions).toHaveLength(0);
});

// After an undo that lands on the offer, it is asked again even with the switch off.
test("Aetherpassage asked again after an undo", async ({ page }, testInfo) => {
  const { advance, submissions } = await openScene(page);
  await page.getByTestId("prompts-btn").click();
  await page.getByTestId("prompt-toggle-aetherpassage").click();
  await page.getByTestId("prompts-panel-close").click();
  advance(3, null);
  advance(2, aetherOffer("aether-4"), 1);
  await page.waitForTimeout(1200);
  expect(submissions).toHaveLength(0);
  await expect(page.getByTestId("aetherpassage-switch")).toBeVisible();
  await shot(page, testInfo, "6-after-undo");
});
