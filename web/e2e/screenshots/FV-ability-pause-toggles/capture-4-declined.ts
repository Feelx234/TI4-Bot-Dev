import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { mendosaOffer, openScene, selfAssemblyOffer, SIZES, type SizeName } from "./scene";

// Switched off, the next offer is declined by the client with a toast; the decline is an ordinary submit_choice.
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`Declined with a toast ${size}`, async ({ page }, testInfo) => {
    const { advance, submissions } = await openScene(page, { size, paused: ["saar_agent"] });
    advance(1, null);
    await page.waitForTimeout(300);
    advance(2, mendosaOffer("mendosa-2"));
    await expect.poll(() => submissions.length).toBe(1);
    expect(JSON.stringify(submissions[0])).toContain("decline");
    await expect(page.getByTestId("corner-toast")).toContainText("Captain Mendosa skipped (switched off)");
    await shot(page, testInfo, `11-declined-toast-${size}`);
  });
}

test("An ability that asks by itself is declined too", async ({ page }) => {
  const { advance, submissions } = await openScene(page, { faction: "sol", technologies: ["sar"], paused: ["self_assembly_routines"] });
  advance(1, null);
  await page.waitForTimeout(300);
  advance(2, selfAssemblyOffer("sar-2"));
  await expect.poll(() => submissions.length).toBe(1);
  expect(JSON.stringify(submissions[0])).toContain("decline");
  await expect(page.getByTestId("corner-toast")).toContainText("Self-Assembly Routines skipped (switched off)");
});

// Another ability in the same window keeps the question open for the player.
test("A window with another answer stays open", async ({ page }, testInfo) => {
  const { advance, submissions } = await openScene(page, { paused: ["saar_agent"] });
  advance(1, null);
  await page.waitForTimeout(300);
  const offer = mendosaOffer("mendosa-3");
  offer.options.splice(1, 0, { id: "reaction:saar:SYSTEM_ACTIVATED:after", label: "Choose an action card…", kind: "ability" });
  advance(2, offer);
  await page.waitForTimeout(1200);
  expect(submissions).toHaveLength(0);
  await expect(page.getByTestId("saar_agent-switch-input")).toBeChecked();
  await shot(page, testInfo, "12-stays-open-with-card");
});
