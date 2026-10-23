import { expect, test, type Page } from "@playwright/test";
import { openMockedGame } from "../_shared/mockGame";
import { playerWithHand, opponent } from "../_shared/players";
import { systemActivationOptions } from "../_shared/fixtures";
import { shot } from "../_shared/shot";
import { push } from "../AH-secondary-prep-real-ui/prep";
import { prefix, topmost } from "./scene";

// o1-211-0549 (phone, 2026-10-22): Warfare's free tactical action asked for a system while a system
// inspector was still open, and the inspector's close button sat under the "Prompt skipped" chip.
test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

const open = async (page: Page) => {
  await page.addInitScript(() => localStorage.setItem("player_ask_winnu_agent", "false"));
  return openMockedGame(page, { players: [playerWithHand({ faction: "winnu" }), opponent] });
};

test("inspector header clear of the paused-prompts chip", async ({ page }, testInfo) => {
  await open(page);
  await page.getByTestId("paused-prompts-chip").waitFor();
  await page.getByTestId("system-hex-26").click();
  await page.getByTestId("system-inspector").waitFor();
  await page.waitForTimeout(300);
  const cover = await topmost(page, "close-inspector-button");
  console.log(`inspector ${prefix}: topmost at the close button = ${cover}`);
  await shot(page, testInfo, `${prefix}-1-inspector-close`);
  if (prefix === "after") expect(cover).toBe("itself");
});

test("free tactical action closes the inspector", async ({ page }, testInfo) => {
  const game = await open(page);
  await page.getByTestId("system-hex-26").click();
  await page.getByTestId("system-inspector").waitFor();
  push(game, {
    version: 42,
    history: { cursor: 41, redo_count: 0, generation: 0 },
    choice: {
      prompt: "Warfare: a tactical action without a command token",
      subtype: "warfare_free_tactical",
      options: systemActivationOptions,
    },
  });
  await page.getByText("a tactical action without a command token").first().waitFor();
  await page.waitForTimeout(500);
  const count = await page.getByTestId("system-inspector").count();
  console.log(`warfare free tactical ${prefix}: inspector open = ${count}`);
  await shot(page, testInfo, `${prefix}-2-free-tactical`);
  if (prefix === "after") expect(count).toBe(0);
});
