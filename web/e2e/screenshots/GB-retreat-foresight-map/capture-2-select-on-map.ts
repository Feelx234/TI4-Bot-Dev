import { expect, test } from "@playwright/test";
import { open, scenes, take, viewports } from "./scene";

// A click on a highlighted system picks the destination; Confirm sends exactly that option.
const cases = [
  { name: "foresight", hex: "26", option: "system|26", confirm: "Confirm Foresight" },
  { name: "retreat-multi", hex: "19", option: "19", confirm: "Confirm" },
] as const;

for (const c of cases) {
  for (const v of viewports) {
    test(`${c.name}-selected-${v.id}`, async ({ page }, testInfo) => {
      const sent: Record<string, unknown>[] = [];
      const [board, choice] = scenes[c.name]();
      await open(page, v, board, choice, (message) => sent.push(message));
      // The heading names the move; the from system is marked and the destinations are targets.
      await expect(page.getByTestId("retreat-pick-title")).toContainText("from Mecatol Rex (System 18)");
      await expect(page.getByTestId("system-hex-18")).toHaveAttribute("data-context-subject", "true");
      await expect(page.getByTestId("system-hex-18").getByTestId("move-origin-marker")).toHaveCount(1);
      await expect(page.getByTestId(`system-hex-${c.hex}`)).toHaveAttribute("data-target-candidate", "true");
      await expect(page.getByTestId("confirm-retreat-btn")).toBeDisabled();
      await page.getByTestId(`system-hex-${c.hex}`).dispatchEvent("click");
      await expect(page.getByTestId("retreat-pick-selected")).toContainText("System " + c.hex);
      await expect(page.getByTestId("confirm-retreat-btn")).toBeEnabled();
      await page.waitForTimeout(300);
      await take(page, testInfo, `${c.name}-selected`, v);
      await page.getByTestId("confirm-retreat-btn").click();
      await expect.poll(() => JSON.stringify(sent)).toContain(c.option);
    });
  }
}

// The list is the same choice: a chip selects the destination too (phone fallback).
test("chip picks the destination (phone)", async ({ page }) => {
  const [board, choice] = scenes["retreat-small"]();
  await open(page, viewports[1], board, choice);
  await page.getByTestId("retreat-pick-22").click();
  await expect(page.getByTestId("retreat-pick-selected")).toContainText("System 22");
});
