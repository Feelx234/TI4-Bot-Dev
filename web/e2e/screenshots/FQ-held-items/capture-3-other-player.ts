import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { noHorizontalScroll, openHeldItems, openScene, richBoard, richState, rich, rival, SIZES, type SizeName } from "./scene";

// Another seat's public view: read-only, no notes in hand, no faction markers, nothing usable.
for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`other player ${size}`, async ({ page }, testInfo) => {
    await openScene(page, size, { players: [rich, rival], state: richState, board: richBoard });
    await openHeldItems(page, size, 1);
    const panel = page.getByTestId("held-items-panel");
    await expect(panel).toHaveAttribute("data-own", "false");
    await expect(page.getByTestId("held-items-mode")).toContainText("public information only");
    await expect(panel).not.toContainText("in hand");
    await expect(panel).not.toContainText("usable now");
    await expect(page.getByTestId("held-item-note-ps:hacan")).toHaveCount(0);
    expect(await noHorizontalScroll(page)).toBeLessThanOrEqual(0);
    await shot(page, testInfo, `1-other-${size}`);
    // The switcher goes back to the viewer's own items.
    await page.getByTestId("held-items-seat-0").click();
    await expect(panel).toHaveAttribute("data-own", "true");
  });
}
