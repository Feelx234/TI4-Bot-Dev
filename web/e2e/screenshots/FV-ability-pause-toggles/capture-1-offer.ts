import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { mendosaOffer, openScene, selfAssemblyOffer, SIZES, type SizeName } from "./scene";

// A reaction-window ability that was not toggleable before (the Saar agent): the offer carries the checkbox.
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`Window offer with the checkbox ${size}`, async ({ page }, testInfo) => {
    const { advance, submissions } = await openScene(page, { size });
    advance(1, null);
    advance(2, mendosaOffer("mendosa-1"));
    await expect(page.getByTestId("saar_agent-switch")).toBeVisible();
    await expect(page.getByTestId("saar_agent-switch-input")).not.toBeChecked();
    await page.waitForTimeout(1000);
    expect(submissions).toHaveLength(0);
    await shot(page, testInfo, `1-offer-${size}`);
  });

  // An ability that asks its own question (no reaction dialog): the switch sits under the question.
  test(`Own question with the checkbox ${size}`, async ({ page }, testInfo) => {
    const { advance, submissions } = await openScene(page, { size, faction: "sol", technologies: ["sar"] });
    advance(1, null);
    advance(2, selfAssemblyOffer("sar-1"));
    await expect(page.getByTestId("self_assembly_routines-switch")).toBeVisible();
    await expect(page.getByTestId("self_assembly_routines-switch-input")).not.toBeChecked();
    await page.waitForTimeout(1000);
    expect(submissions).toHaveLength(0);
    await shot(page, testInfo, `2-own-question-${size}`);
  });
}
