import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openResearch, prefix, SIZES, VARIANTS, type SizeName } from "./scene";

// The Select Research picker with AI Development Algorithm and Psychoarchaeology. `before` runs
// (TI4_SHOT_PREFIX=before, on the commit without the change) only take pictures of the variants
// that existed then.
test.describe.configure({ retries: 2, mode: "serial" });

const NEW_ONLY = new Set(["aida-toggled", "aida-nonupgrade", "first-aida-second-cr2"]);

for (const size of Object.keys(SIZES) as SizeName[]) {
  for (const v of VARIANTS) {
    if (prefix === "before" && NEW_ONLY.has(v.key)) continue;
    test(`${v.key} ${size}`, async ({ page }, testInfo) => {
      await openResearch(page, size, v);
      const chip = page.getByTestId("tech-skip-aida");
      const card = (id: string) => page.getByTestId(`tech-card-${id}`);
      const researchable = (id: string) => card(id).getAttribute("data-researchable");
      await shot(page, testInfo, `${prefix}-${v.key}-${size}-panel`);

      const target = v.key === "pa-exhausted-planet" ? "gd" : "dn2";
      await card(target).scrollIntoViewIfNeeded();
      await shot(page, testInfo, `${prefix}-${v.key}-${size}-cards`);

      if (prefix === "before") return;
      if (v.key === "aida-unlocks") {
        expect(await researchable("dn2")).toBe("true");
        expect(await researchable("cr2")).toBe("true");
        await expect(page.getByTestId("tech-aida-note-dn2")).toContainText("Unlocked by AI Development Algorithm");
        await expect(chip).toBeEnabled();
        await expect(chip).toContainText("Ready (+1)");
      } else if (v.key === "aida-toggled") {
        await card("dn2").click();
        await chip.click();
        await expect(chip).toContainText("Active (+1), exhausts only if needed");
        await chip.scrollIntoViewIfNeeded();
        await shot(page, testInfo, `${prefix}-${v.key}-${size}-toggled`);
      } else if (v.key === "aida-nonupgrade") {
        await card("gd").click();
        await expect(chip).toBeDisabled();
        await expect(chip).toContainText("Unit upgrades only");
        await chip.scrollIntoViewIfNeeded();
        await shot(page, testInfo, `${prefix}-${v.key}-${size}-disabled`);
      } else if (v.key === "aida-exhausted") {
        expect(await researchable("dn2")).toBe("false");
        await expect(chip).toBeDisabled();
        await expect(chip).toContainText("Exhausted");
      } else if (v.key === "first-aida-second-cr2") {
        await card("aida").click();
        await expect(card("cr2")).toHaveAttribute("data-selectable", "true");
        await expect(page.getByTestId("tech-aida-note-cr2")).toContainText("Unlocked by AI Development Algorithm");
        await card("cr2").click();
        await expect(chip).toContainText("Active (+1)");
        await chip.scrollIntoViewIfNeeded();
        await shot(page, testInfo, `${prefix}-${v.key}-${size}-second`);
        await card("cr2").scrollIntoViewIfNeeded();
        await shot(page, testInfo, `${prefix}-${v.key}-${size}-second-card`);
      } else if (v.key === "pa-exhausted-planet") {
        expect(await researchable("gd")).toBe("true");
        const pill = page.getByTestId("tech-skip-thibah");
        await expect(pill).toBeEnabled();
        await expect(pill).toContainText("free with Psychoarchaeology");
        await pill.click();
        await expect(pill).toContainText("free, not exhausted by Psychoarchaeology");
        await page.getByTestId("tech-skips-list").scrollIntoViewIfNeeded();
        await shot(page, testInfo, `${prefix}-${v.key}-${size}-toggled`);
      } else {
        expect(await researchable("dn2")).toBe("false");
        await expect(chip).toHaveCount(0);
      }
    });
  }
}
