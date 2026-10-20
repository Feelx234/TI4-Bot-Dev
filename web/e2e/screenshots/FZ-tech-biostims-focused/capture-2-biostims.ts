import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openBioStims, SIZES, type SizeName } from "./scene";

test.describe.configure({ retries: 2 });

const states = [
  { key: "usable", opts: { exhaustedBs: false, exhaustedTech: true, exhaustedPlanet: true }, state: "usable" },
  { key: "nothing", opts: { exhaustedBs: false, exhaustedTech: false, exhaustedPlanet: false }, state: "ready" },
  { key: "exhausted", opts: { exhaustedBs: true, exhaustedTech: true, exhaustedPlanet: true }, state: "exhausted" },
] as const;

for (const size of Object.keys(SIZES) as SizeName[]) {
  for (const s of states) {
    test(`bio-stims ${s.key} ${size}`, async ({ page }, testInfo) => {
      await openBioStims(page, size, s.opts);
      const strip = page.getByTestId("tech-bio-stims");
      await expect(strip).toHaveAttribute("data-state", s.state);
      if (s.key === "usable") {
        await expect(strip).toContainText("Wellon");
        await expect(strip).toContainText("Sarween Tools");
      }
      await shot(page, testInfo, `after-biostims-${s.key}-${size}`);
    });
  }
}
