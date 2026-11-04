import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { actor } from "../_shared/players";
import {
  BEFORE,
  noHorizontalPageScroll,
  openObjectives,
  openScene,
  prefixed,
  SCENES,
  SIZES,
  type SceneName,
  type SizeName,
} from "./scene";

// Diversify Research and Master the Sciences in the Objectives dialog: for every seat, how many
// technologies of each colour it owns against the 2 the card needs (alpha feedback fb_1793103326).
const MINE: Record<SceneName, string[]> = {
  "one-colour": ["Blue 2/2 ✓", "Green 1/2", "Yellow 0/2", "Red 0/2"],
  "two-colours": ["Blue 2/2 ✓", "Green 2/2 ✓", "Yellow 0/2", "Red 0/2"],
};

for (const name of Object.keys(SCENES) as SceneName[]) {
  for (const size of Object.keys(SIZES) as SizeName[]) {
    test(`diversify ${name} ${size}`, async ({ page }, testInfo) => {
      await openScene(page, size, name);
      await openObjectives(page, size);
      const modal = page.getByTestId("objectives-modal");
      if (!BEFORE) {
        // The viewer's column on desktop, the card-column copy on a phone.
        const mine =
          size === "desktop"
            ? modal.getByTestId(`tech-colour-progress-diversify-${actor}`)
            : modal.getByTestId("tech-colour-progress-diversify-card");
        await expect(mine).toBeVisible();
        for (const text of MINE[name]) await expect(mine).toContainText(text);
        await expect(mine).toContainText(
          `${SCENES[name].diversify.me} of 2 colors with 2+ · need 2`,
        );
        const chips = mine.locator("li");
        await expect(chips).toHaveCount(4);
        // Every chip also says its colour in words, never by the swatch alone.
        await expect(chips.first()).toHaveAttribute("title", /Propulsion \(blue\): 2 of 2/);
        // Master the Sciences reads 4 colours.
        const master =
          size === "desktop"
            ? modal.getByTestId(`tech-colour-progress-master_science-${actor}`)
            : modal.getByTestId("tech-colour-progress-master_science-card");
        await expect(master).toContainText(`${SCENES[name].master.me} of 4 colors with 2+ · need 4`);
        if (size === "desktop") {
          // Every seat's column carries its own counts.
          await expect(modal.getByTestId("tech-colour-progress-diversify-other_seat")).toContainText("Yellow 2/2 ✓");
          await expect(modal.getByTestId("tech-colour-progress-diversify-other_seat")).toContainText("Red 2/2 ✓");
          await expect(modal.getByTestId("tech-colour-progress-diversify-seat_c")).toContainText("Blue 1/2");
        }
      }
      if (size === "390x844") expect(await noHorizontalPageScroll(page)).toBeLessThanOrEqual(0);
      await page.getByTestId("objective-row-diversify").scrollIntoViewIfNeeded();
      await shot(page, testInfo, prefixed(`1-diversify-${name}-${size}`));
    });
  }
}
