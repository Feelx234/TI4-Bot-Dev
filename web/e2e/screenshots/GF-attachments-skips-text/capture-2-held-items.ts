import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { openBoard, openHeldItems, prefix, SIZES, type SizeName } from "./scene";

// The Held items panel lists the attachments on the viewer's planets. Demilitarized Zone and the
// Cybernetic Research Facility had no text (only +resources/+influence were ever read).
test.describe.configure({ retries: 2, mode: "serial" });

for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`held items attachments ${size}`, async ({ page }, testInfo) => {
    await openBoard(page, size);
    await openHeldItems(page, size);
    const section = page.getByTestId("held-section-attachments");
    await section.scrollIntoViewIfNeeded();
    if (prefix === "after") {
      await expect(page.getByTestId("held-item-attach-centauri-dmz")).toContainText("cannot be committed to, produced on");
      await expect(page.getByTestId("held-item-attach-tarmann-cybernetic")).toContainText("yellow technology specialty");
      await expect(page.getByTestId("held-item-attach-jord-warfare")).toContainText("red technology specialty");
    }
    await shot(page, testInfo, `${prefix}-held-${size}`);
  });
}
