import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openResearch, prefix, SIZES, VARIANTS, type SizeName } from "./scene";

// The Select Research picker with AI Development Algorithm and Psychoarchaeology. `before` runs
// (TI4_SHOT_PREFIX=before, on the commit without the change) only take pictures.
test.describe.configure({ retries: 2, mode: "serial" });

for (const size of Object.keys(SIZES) as SizeName[]) {
  for (const v of VARIANTS) {
    test(`${v.key} ${size}`, async ({ page }, testInfo) => {
      await openResearch(page, size, v);
      await shot(page, testInfo, `${prefix}-${v.key}-${size}-panel`);

      const dn2 = page.getByTestId("tech-card-dn2");
      const gd = page.getByTestId("tech-card-gd");
      const target = v.key === "pa-exhausted-planet" ? gd : dn2;
      await target.scrollIntoViewIfNeeded();
      await shot(page, testInfo, `${prefix}-${v.key}-${size}-cards`);

      if (prefix === "before") return;
      const researchable = (card: typeof dn2) => card.getAttribute("data-researchable");
      if (v.key === "aida-unlocks") {
        expect(await researchable(dn2)).toBe("true");
        expect(await researchable(page.getByTestId("tech-card-cr2"))).toBe("true");
        await expect(page.getByTestId("tech-aida-note-dn2")).toContainText("Unlocked by AI Development Algorithm");
        await expect(page.getByTestId("tech-aida-note")).toContainText("ignores 1 prerequisite (exhausts only if needed)");
      } else if (v.key === "aida-exhausted") {
        expect(await researchable(dn2)).toBe("false");
        await expect(page.getByTestId("tech-aida-note")).toHaveCount(0);
      } else if (v.key === "pa-exhausted-planet") {
        expect(await researchable(gd)).toBe("true");
        const pill = page.getByTestId("tech-skip-thibah");
        await expect(pill).toBeEnabled();
        await expect(pill).toContainText("free with Psychoarchaeology");
        await pill.click();
        await expect(pill).toContainText("free, not exhausted by Psychoarchaeology");
        await page.getByTestId("tech-skips-list").scrollIntoViewIfNeeded();
        await shot(page, testInfo, `${prefix}-${v.key}-${size}-toggled`);
      } else {
        expect(await researchable(dn2)).toBe("false");
        await expect(page.getByTestId("tech-aida-note")).toHaveCount(0);
      }
    });
  }
}
