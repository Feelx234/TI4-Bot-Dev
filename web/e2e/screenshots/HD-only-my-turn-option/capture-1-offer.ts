import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { bioStimsOffer, mendosaOffer, openScene, SIZES, type SizeName } from "./scene";

// An ability that comes up in your own turn and in other players' turns gets a second checkbox.
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`Offer with the Only in my turn checkbox ${size}`, async ({ page }, testInfo) => {
    const { advance, submissions } = await openScene(page, { size, activePlayer: "me" });
    advance(1, null);
    advance(2, mendosaOffer("mendosa-1"));
    const own = page.getByTestId("saar_agent-ownturn-input");
    await expect(own).toBeVisible();
    await expect(page.getByTestId("saar_agent-switch-input")).not.toBeChecked();
    await shot(page, testInfo, `1-offer-${size}`);

    await own.check();
    expect(await page.evaluate(() => localStorage.getItem("player_ask_saar_agent"))).toBe("ownturn");
    await expect(own).toBeChecked();
    await expect(page.getByTestId("saar_agent-switch-input")).not.toBeChecked();
    await shot(page, testInfo, `2-offer-ticked-${size}`);
    expect(submissions).toHaveLength(0);
  });
}

// An ability that only ever asks in the owner's own turn keeps the single checkbox.
test("Offer of a one-context ability keeps one checkbox", async ({ page }, testInfo) => {
  const { advance } = await openScene(page, { faction: "saar", technologies: ["bs"] });
  advance(1, null);
  advance(2, bioStimsOffer("bs-1"));
  await expect(page.getByTestId("bio_stims-switch-input")).toBeVisible();
  await expect(page.getByTestId("bio_stims-ownturn-input")).toHaveCount(0);
  await shot(page, testInfo, "3-offer-one-context");
});
