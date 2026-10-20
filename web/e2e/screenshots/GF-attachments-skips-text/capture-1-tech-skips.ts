import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { showTitleTooltip } from "../_shared/tooltip";
import { openResearch, prefix, SIZES, type SizeName } from "./scene";

// The technology picker's Tech Skips row. Jord has a Warfare Research Facility (a red skip),
// Tarmann (exhausted) a Cybernetic one, Wellon prints a yellow specialty and carries the +1/+1 side
// of a Warfare facility (no second skip). The viewer owns Sarween Tools (yellow); PDS II needs R + Y.
test.describe.configure({ retries: 2, mode: "serial" });

for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`skips row ${size}`, async ({ page }, testInfo) => {
    await openResearch(page, size, { engine: ["nm", "ps", "gd", "gls", "pds2"] });
    const row = page.getByTestId("tech-skips-list");
    await row.scrollIntoViewIfNeeded();
    if (prefix === "before") {
      await shot(page, testInfo, `${prefix}-skips-${size}`);
      return;
    }
    const jord = page.getByTestId("tech-skip-jord");
    await expect(jord).toContainText("Warfare Research Facility");
    await expect(page.getByTestId("tech-skip-badge-jord-warfare")).toBeVisible();
    await expect(page.getByTestId("tech-skip-wellon")).not.toContainText("Warfare Research Facility");
    // The exhausted facility planet cannot be used (no Psychoarchaeology).
    await expect(page.getByTestId("tech-skip-tarmann")).toBeDisabled();
    await expect(page.getByTestId("tech-skip-tarmann")).toContainText("Exhausted");
    await shot(page, testInfo, `${prefix}-skips-${size}`);
    // Toggling the facility planet is what covers PDS II's red prerequisite.
    await page.getByTestId("tech-card-pds2").click();
    await jord.click();
    await expect(jord).toContainText("Active (+1)");
    await expect(page.getByTestId("tech-card-pds2")).toHaveAttribute("data-selected", "true");
    await shot(page, testInfo, `${prefix}-skips-toggled-${size}`);
    if (size === "desktop") {
      await jord.evaluate((el) => el.removeAttribute("title"));
      await showTitleTooltip(page, page.getByTestId("tech-skip-badge-jord-warfare"));
      await shot(page, testInfo, `${prefix}-skips-badge-hover-${size}`);
    }
  });
}
