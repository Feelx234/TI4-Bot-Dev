import { expect, test } from "@playwright/test";
import { openMapLobby } from "../_shared/mapLobby";
import { shot } from "../_shared/shot";
import { SIZES } from "./expeditionGame";

// Where the content is switched on: the offline setup form and the table form of an online game,
// and the lobby of a table that has it on. Off is what a new table plays unless the box is ticked.
for (const [size, viewport] of Object.entries(SIZES)) {
  test(`offline setup ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    await page.route(/\/(api|ws|health)(\/|$)/, (route) => route.abort());
    await page.goto("/offline");
    const toggle = page.getByTestId("offline-thunders-edge");
    await expect(toggle).toBeVisible();
    await expect(toggle.getByRole("checkbox")).not.toBeChecked();
    await shot(page, testInfo, `1-offline-setup-off-${size}`);
    await toggle.getByRole("checkbox").check();
    await shot(page, testInfo, `2-offline-setup-on-${size}`);
  });

  test(`online table form ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    await page.route(/\/api\//, (route) => route.fulfill({ status: 200, contentType: "application/json", body: "[]" }));
    await page.goto("/");
    const toggle = page.getByTestId("thunders-edge-toggle");
    await expect(toggle).toBeVisible();
    await toggle.getByRole("checkbox").check();
    await shot(page, testInfo, `3-online-table-form-${size}`);
  });

  test(`online lobby ${size}`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    await openMapLobby(page, { players: 4, strategyCardSet: "te", thundersEdge: true });
    await expect(page.getByTestId("lobby-thunders-edge-row")).toBeVisible();
    await shot(page, testInfo, `4-online-lobby-${size}`);
  });
}
