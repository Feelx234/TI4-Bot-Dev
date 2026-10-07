import { test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openWaiting, push, secondaryQuestion } from "./prep";

// Auto mode: the window opens and the prepared answer is announced, with Cancel, before it is sent.
test("secondary prep: auto mode toast", async ({ page }, testInfo) => {
  const game = await openWaiting(page, { name: "Technology", card: "pok7technology" });
  await page.getByTestId("secondary-prep-mode-auto").click();
  await page.getByTestId("secondary-prep-chip").click();
  await page.getByTestId("prep-follow").click();
  await page.getByTestId("prep-tech-amd").click();
  push(game, {
    version: 42,
    history: { cursor: 41, redo_count: 0, generation: 0 },
    choice: secondaryQuestion("pok7technology", "spend a strategy token and 4 resources to research", "spend"),
  });
  await page.getByTestId("secondary-autoplay-toast").waitFor();
  await shot(page, testInfo, "9-auto-toast");
});
