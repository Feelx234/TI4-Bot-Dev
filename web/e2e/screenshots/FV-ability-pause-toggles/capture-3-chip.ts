import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openScene, SIZES, type SizeName } from "./scene";

// While anything is switched off a chip under the top bar says so, and opens the list.
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`Paused prompts chip ${size}`, async ({ page }, testInfo) => {
    await openScene(page, { size, faction: "nomad", paused: ["thunders_paradox", "nomad_mercer"] });
    const chip = page.getByTestId("paused-prompts-chip");
    await expect(chip).toBeVisible();
    await expect(chip).toContainText("2 prompts skipped");
    await shot(page, testInfo, `8-chip-${size}`);
    await chip.click();
    await expect(page.getByTestId("prompts-panel")).toBeVisible();
    await expect(page.getByTestId("prompt-row-thunders_paradox")).toHaveAttribute("data-paused", "true");
    await shot(page, testInfo, `9-chip-opens-list-${size}`);
    // Switching the last two back on removes the chip.
    await page.getByTestId("prompt-toggle-thunders_paradox").click();
    await page.getByTestId("prompt-toggle-nomad_mercer").click();
    await page.getByTestId("prompts-panel-close").click();
    await expect(chip).toHaveCount(0);
  });
}

// A skipped ability of another faction stays visible (and switchable) wherever the player sits.
test("A skipped foreign ability is listed", async ({ page }, testInfo) => {
  await openScene(page, { faction: "sol", paused: ["aetherpassage"] });
  await page.getByTestId("paused-prompts-chip").click();
  await expect(page.getByTestId("prompt-row-aetherpassage")).toContainText("not yours in this game");
  await shot(page, testInfo, "10-foreign-listed");
});
