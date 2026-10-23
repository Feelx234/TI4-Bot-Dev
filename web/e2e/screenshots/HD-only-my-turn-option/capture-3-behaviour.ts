import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { mendosaOffer, openScene } from "./scene";

// With "Only in my turn" the client declines in another player's turn (toast "not your turn") ...
test("Declined in another player's turn", async ({ page }, testInfo) => {
  const { advance, submissions } = await openScene(page, { stored: { saar_agent: "ownturn" }, activePlayer: "other" });
  advance(1, null);
  await page.waitForTimeout(300);
  advance(2, mendosaOffer("mendosa-2"));
  await expect.poll(() => submissions.length).toBe(1);
  expect(JSON.stringify(submissions[0])).toContain("decline");
  await expect(page.getByTestId("corner-toast")).toContainText("Captain Mendosa skipped (not your turn)");
  await shot(page, testInfo, "10-declined-others-turn");
});

// ... and asks as usual in the viewer's own turn.
test("Asked in the viewer's own turn", async ({ page }, testInfo) => {
  const { advance, submissions } = await openScene(page, { stored: { saar_agent: "ownturn" }, activePlayer: "me" });
  advance(1, null);
  await page.waitForTimeout(300);
  advance(2, mendosaOffer("mendosa-3"));
  await page.waitForTimeout(1200);
  expect(submissions).toHaveLength(0);
  await expect(page.getByTestId("saar_agent-ownturn-input")).toBeChecked();
  await shot(page, testInfo, "11-asked-own-turn");
});

// A plain Skip still declines in the viewer's own turn.
test("Skip declines in every turn", async ({ page }, testInfo) => {
  const { advance, submissions } = await openScene(page, { stored: { saar_agent: "false" }, activePlayer: "me" });
  advance(1, null);
  await page.waitForTimeout(300);
  advance(2, mendosaOffer("mendosa-4"));
  await expect.poll(() => submissions.length).toBe(1);
  await expect(page.getByTestId("corner-toast")).toContainText("Captain Mendosa skipped (switched off)");
  await shot(page, testInfo, "12-skip-own-turn");
});
