import { expect, test } from "@playwright/test";
import { shot } from "../_shared/shot";
import { showTitleTooltip } from "../_shared/tooltip";
import { openBoard, prefix, SIZES, type SizeName } from "./scene";

// Hovering (or focusing, or tapping) an attachment's name shows what it does. After the change the
// system inspector's attachments are popover triggers; the board's attachment marker keeps a
// native tooltip, now with the effect text.
test.describe.configure({ retries: 2, mode: "serial" });

for (const size of Object.keys(SIZES) as SizeName[]) {
  test(`inspector attachment hover ${size}`, async ({ page }, testInfo) => {
    await openBoard(page, size);
    await page.getByTestId("system-hex-18").click();
    await page.getByTestId("system-inspector").waitFor();
    if (prefix === "before") {
      await shot(page, testInfo, `${prefix}-inspector-${size}`);
      return;
    }
    const trigger = page.getByTestId("attachment-info-warfare").getByRole("button");
    await trigger.scrollIntoViewIfNeeded();
    if (size === "desktop") await trigger.hover();
    else await trigger.click();
    const card = page.getByTestId("attachment-card-warfare");
    await expect(card).toContainText("Warfare Research Facility");
    await expect(card).toContainText("red technology specialty");
    await page.waitForTimeout(300);
    await shot(page, testInfo, `${prefix}-inspector-${size}`);
  });
}

test("board attachment marker title", async ({ page }, testInfo) => {
  await openBoard(page, "desktop");
  const mark = page.getByTestId("exploration-mark-attachments-18");
  await mark.waitFor();
  const title = (await mark.locator("title").textContent()) ?? "";
  if (prefix === "after") expect(title).toContain("red technology specialty");
  await showTitleTooltip(page, mark.locator("circle"), title);
  await shot(page, testInfo, `${prefix}-marker-title-desktop`);
});
