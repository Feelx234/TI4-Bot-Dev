import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { mendosaOffer, openScene } from "./scene";

// After an undo that lands on the offer it is asked again, even with the ability switched off.
test("Asked again after an undo", async ({ page }, testInfo) => {
  const { advance, submissions } = await openScene(page, { paused: ["saar_agent"] });
  advance(3, null);
  advance(2, mendosaOffer("mendosa-4"), 1);
  await page.waitForTimeout(1200);
  expect(submissions).toHaveLength(0);
  await expect(page.getByTestId("saar_agent-switch")).toBeVisible();
  await expect(page.getByTestId("saar_agent-switch-input")).toBeChecked();
  await shot(page, testInfo, "13-after-undo");
});

// A page load shows the open offer at once: asked, never answered by the client.
test("Asked after a page load", async ({ page }) => {
  const { advance, submissions } = await openScene(page, { paused: ["saar_agent"] });
  advance(2, mendosaOffer("mendosa-5"));
  await page.waitForTimeout(1200);
  expect(submissions).toHaveLength(0);
  await expect(page.getByTestId("saar_agent-switch")).toBeVisible();
});
